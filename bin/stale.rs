use chrono::{DateTime, Local};
use clap::Parser;
use git2::{Branch, BranchType, PushOptions, Repository};
use git_toolbox::{
    config::Configuration, config::ProtectedBranches, git::credentials::remote_callbacks,
    git::GitTime, git::RemoteRef, reltime::Reltime,
};
use log::{error, info, warn};
use std::{collections::HashMap, error::Error, io, process::exit};

#[derive(Parser)]
#[command(
    about = "List or delete stale branches",
    long_about = None)]
struct Cli {
    #[arg(
        long,
        help = "Select origin remote-tracking branches instead of local branches"
    )]
    remote: bool,
    #[arg(short, long, help = "Perform deletion of selected branches")]
    delete: bool,
    #[arg(
        long,
        help = "Combined with --delete, perform deletion on remote repository instead"
    )]
    push: bool,
    #[arg(long,
        help = "Select local branch with commit times older than the specified relative time",
        value_parser = parse_reltime)]
    since: Option<Reltime>,
    #[arg(help = "Select branches with specified prefixes, or select all if unset")]
    branches: Vec<String>,
}

fn parse_reltime(arg: &str) -> Result<Reltime, String> {
    Reltime::try_from(arg).map_err(|e| format!("while parsing {arg} got error: {e}"))
}

enum Command {
    DeleteUpstreamBranches {
        repo: Repository,
        visitor: LocalBranchVisitor,
    },
    DeleteLocalBranches {
        repo: Repository,
        visitor: LocalBranchVisitor,
    },
    ListLocalBranches {
        repo: Repository,
        visitor: LocalBranchVisitor,
    },
    DeleteRemoteBranches {
        repo: Repository,
        visitor: RemoteBranchVisitor,
    },
    ListRemoteBranches {
        repo: Repository,
        visitor: RemoteBranchVisitor,
    },
}

impl Command {
    fn run(self) -> Result<(), Box<dyn Error>> {
        match self {
            Self::DeleteUpstreamBranches { repo, visitor } => {
                let refspecs: HashMap<String, Vec<String>> = HashMap::new();
                let mut refspecs =
                    visitor.for_each_branches(&repo, refspecs, |mut refspecs, branch| {
                        let upstream = branch.upstream()?;
                        let upstream = upstream.get();
                        let upstream = upstream
                            .name()
                            .and_then(|u| u.strip_prefix("refs/remotes/"))
                            .and_then(|u| u.split('/').next());
                        let branch_name = branch.get().name();

                        if let (Some(remote_name), Some(branch_name)) = (upstream, branch_name) {
                            info!("branch '{branch_name}' will be deleted from {remote_name}");

                            // refspec has <src>:<dst> format, so leaving <src> empty will delete <dst>.
                            let refspec = format!(":{branch_name}");
                            if let Some(branches) = refspecs.get_mut(remote_name) {
                                branches.push(refspec)
                            } else {
                                refspecs.insert(remote_name.to_owned(), vec![refspec]);
                            }
                        }

                        Ok(refspecs)
                    })?;
                for (remote_name, refspecs) in refspecs.drain() {
                    let mut remote = repo.find_remote(&remote_name)?;
                    let config = repo.config()?;
                    let mut callbacks = remote_callbacks(config);
                    callbacks.push_update_reference(|refname, status| {
                        if let Some(error) = status {
                            warn!("push failed: {refname}, status = {error}");
                        } else {
                            info!("pushed: {refname}");
                        }
                        Ok(())
                    });
                    let mut push_options = PushOptions::new();
                    push_options.remote_callbacks(callbacks);
                    if let Err(e) = remote.push(refspecs.as_slice(), Some(&mut push_options)) {
                        warn!("failed to remove branches from {remote_name}: {e}")
                    }
                }
                Ok(())
            }
            Self::DeleteLocalBranches { repo, visitor } => {
                visitor.for_each_branches(&repo, (), |_, mut branch| {
                    if let Some(branch_name) = branch.get().name() {
                        let branch_name = branch_name.to_owned();
                        if let Err(e) = branch.delete() {
                            warn!("failed to remove branch '{branch_name}': {e}")
                        }
                    }
                    Ok(())
                })
            }
            Self::ListLocalBranches { repo, visitor } => {
                visitor.for_each_branches(&repo, (), |_, branch| {
                    println!("{}", branch.get().name().unwrap());
                    Ok(())
                })
            }
            Self::DeleteRemoteBranches { repo, visitor } => {
                let refspecs = visitor.for_each_branches(
                    &repo,
                    Vec::new(),
                    |mut refspecs, remote_ref, _| {
                        info!(
                            "branch '{}' will be deleted from {}",
                            remote_ref.branch(),
                            remote_ref.remote()
                        );
                        refspecs.push(format!(":refs/heads/{}", remote_ref.branch()));
                        Ok(refspecs)
                    },
                )?;
                let mut remote = repo.find_remote("origin")?;
                let config = repo.config()?;
                let mut callbacks = remote_callbacks(config);
                callbacks.push_update_reference(|refname, status| {
                    if let Some(error) = status {
                        warn!("push failed: {refname}, status = {error}");
                    } else {
                        info!("pushed: {refname}");
                    }
                    Ok(())
                });
                let mut push_options = PushOptions::new();
                push_options.remote_callbacks(callbacks);
                if let Err(e) = remote.push(refspecs.as_slice(), Some(&mut push_options)) {
                    warn!("failed to remove branches from origin: {e}")
                }
                Ok(())
            }
            Self::ListRemoteBranches { repo, visitor } => {
                visitor.for_each_branches(&repo, (), |_, remote_ref, _| {
                    println!("{}/{}", remote_ref.remote(), remote_ref.branch());
                    Ok(())
                })
            }
        }
    }
}

struct LocalBranchVisitor {
    since: Option<DateTime<Local>>,
    branches: Vec<String>,
    protected_branches: Option<ProtectedBranches>,
}

struct RemoteBranchVisitor {
    since: DateTime<Local>,
    branches: Vec<String>,
    protected_branches: Option<ProtectedBranches>,
    default_branch: Option<String>,
}

impl LocalBranchVisitor {
    fn for_each_branches<S, F: Fn(S, Branch<'_>) -> Result<S, Box<dyn Error>>>(
        &self,
        repo: &Repository,
        init: S,
        f: F,
    ) -> Result<S, Box<dyn Error>> {
        let mut st = init;
        for branch in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            if !self.match_branch(&branch)? {
                continue;
            }

            let commit = branch.get().peel_to_commit()?;
            let commit_time: GitTime = commit.time().into();

            if let Some(s) = self.since {
                if s > commit_time.into() {
                    st = f(st, branch)?;
                }
            } else if branch.upstream().is_err() {
                st = f(st, branch)?;
            }
        }

        Ok(st)
    }

    fn match_branch(&self, branch: &Branch) -> Result<bool, Box<dyn Error>> {
        match branch.name()? {
            None => Ok(false),
            Some(branch_name) => {
                if branch.is_head() {
                    info!("branch '{branch_name}' ignored. NOTE: HEAD branch is always ignored.");
                    Ok(false)
                } else if self
                    .protected_branches
                    .as_ref()
                    .is_some_and(|protected| protected.is_match(branch_name))
                {
                    info!(
                        "branch '{branch_name}' ignored. NOTE: protected branches from dah.protectedbranch are always ignored."
                    );
                    Ok(false)
                } else if self.branches.is_empty() {
                    Ok(true)
                } else {
                    match self
                        .branches
                        .iter()
                        .find(|&prefix| branch_name.starts_with(prefix))
                    {
                        Some(_) => Ok(true),
                        None => Ok(false),
                    }
                }
            }
        }
    }
}

impl RemoteBranchVisitor {
    fn for_each_branches<S, F: Fn(S, RemoteRef, Branch<'_>) -> Result<S, Box<dyn Error>>>(
        &self,
        repo: &Repository,
        init: S,
        f: F,
    ) -> Result<S, Box<dyn Error>> {
        let mut st = init;
        for branch in repo.branches(Some(BranchType::Remote))? {
            let (branch, _) = branch?;
            let Some(remote_ref_name) = branch.get().name() else {
                continue;
            };
            let remote_ref = RemoteRef::new(remote_ref_name.to_owned())?;

            if !self.match_branch(&remote_ref)? {
                continue;
            }

            let commit = branch.get().peel_to_commit()?;
            let commit_time: GitTime = commit.time().into();

            if self.since > commit_time.into() {
                st = f(st, remote_ref, branch)?;
            }
        }

        Ok(st)
    }

    fn match_branch(&self, branch: &RemoteRef) -> Result<bool, Box<dyn Error>> {
        if branch.remote() != "origin" {
            return Ok(false);
        }

        let branch_name = branch.branch();
        if branch_name == "HEAD" {
            info!("branch 'origin/HEAD' ignored. NOTE: remote HEAD is always ignored.");
            Ok(false)
        } else if self
            .default_branch
            .as_ref()
            .is_some_and(|default_branch| default_branch == branch_name)
        {
            info!("branch 'origin/{branch_name}' ignored. NOTE: default branch is always ignored.");
            Ok(false)
        } else if self
            .protected_branches
            .as_ref()
            .is_some_and(|protected| protected.is_match(branch_name))
        {
            info!(
                "branch 'origin/{branch_name}' ignored. NOTE: protected branches from dah.protectedbranch are always ignored."
            );
            Ok(false)
        } else if self.branches.is_empty() {
            Ok(true)
        } else {
            Ok(self
                .branches
                .iter()
                .any(|prefix| branch_name.starts_with(prefix)))
        }
    }
}

fn usage_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(io::Error::other(message.into()))
}

impl Cli {
    fn into_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let config = repo.config()?;
        let configuration = Configuration::new(&config);
        let protected_branches = configuration.dah_protected_branches()?;
        let now = Local::now();
        let since = self.since.map(|s| now - s);
        let command = if self.remote {
            let since = since.ok_or_else(|| usage_error("--remote requires --since"))?;
            if self.delete && !self.push {
                return Err(usage_error("--remote --delete requires --push"));
            }
            repo.find_remote("origin")
                .map_err(|_| usage_error("origin remote does not exist"))?;

            let visitor = RemoteBranchVisitor {
                since,
                branches: self.branches,
                protected_branches,
                default_branch: configuration.init_default_branch()?,
            };

            if self.delete {
                Command::DeleteRemoteBranches { repo, visitor }
            } else {
                Command::ListRemoteBranches { repo, visitor }
            }
        } else {
            let visitor = LocalBranchVisitor {
                since,
                branches: self.branches,
                protected_branches,
            };

            if self.delete && self.push {
                Command::DeleteUpstreamBranches { repo, visitor }
            } else if self.delete {
                Command::DeleteLocalBranches { repo, visitor }
            } else {
                Command::ListLocalBranches { repo, visitor }
            }
        };

        Ok(command)
    }
}

fn main() -> ! {
    env_logger::init();
    match Cli::parse().into_command().and_then(|cmd| cmd.run()) {
        Err(e) => {
            error!("{e}");
            exit(1)
        }
        Ok(_) => exit(0),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, LocalBranchVisitor, RemoteBranchVisitor};
    use chrono::{Duration, Local};
    use clap::Parser;
    use git2::{BranchType, ConfigLevel, Oid, Repository, Signature};
    use git_toolbox::git::RemoteRef;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static CWD_LOCK: Mutex<()> = Mutex::new(());

    fn create_repo() -> Result<(TempDir, Repository), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo = Repository::init_bare(tmpdir.path())?;
        let author = Signature::now("foo", "foo@example.com")?;
        {
            let tree_oid = {
                let tree = repo.treebuilder(None)?;
                tree.write()?
            };
            let tree = repo.find_tree(tree_oid)?;

            repo.commit(
                Some("refs/heads/main"),
                &author,
                &author,
                "main",
                &tree,
                &[],
            )?;
            repo.commit(
                Some("refs/heads/develop"),
                &author,
                &author,
                "develop",
                &tree,
                &[],
            )?;
            repo.commit(
                Some("refs/heads/release/v1"),
                &author,
                &author,
                "release v1",
                &tree,
                &[],
            )?;
            repo.commit(
                Some("refs/heads/feature/old"),
                &author,
                &author,
                "feature old",
                &tree,
                &[],
            )?;
        }
        repo.set_head("refs/heads/main")?;

        Ok((tmpdir, repo))
    }

    fn create_local_branch_visitor(
        repo: &Repository,
    ) -> Result<LocalBranchVisitor, Box<dyn std::error::Error>> {
        repo.config()?
            .open_level(ConfigLevel::Local)?
            .set_str("dah.protectedbranch", "develop:release/*")?;
        let config = repo.config()?;
        let protected_branches =
            git_toolbox::config::Configuration::new(&config).dah_protected_branches()?;
        Ok(LocalBranchVisitor {
            since: None,
            branches: Vec::new(),
            protected_branches,
        })
    }

    fn create_remote_branch_visitor(
        repo: &Repository,
    ) -> Result<RemoteBranchVisitor, Box<dyn std::error::Error>> {
        repo.config()?
            .open_level(ConfigLevel::Local)?
            .set_str("dah.protectedbranch", "develop:release/*")?;
        let config = repo.config()?;
        let config = git_toolbox::config::Configuration::new(&config);
        let protected_branches = config.dah_protected_branches()?;
        Ok(RemoteBranchVisitor {
            since: Local::now() + Duration::days(1),
            branches: Vec::new(),
            protected_branches,
            default_branch: config.init_default_branch()?,
        })
    }

    fn track_branch(
        repo: &Repository,
        branch_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let remote_ref = format!("refs/remotes/origin/{branch_name}");
        let local_branch = repo.find_branch(branch_name, BranchType::Local)?;
        let branch_target = local_branch
            .get()
            .target()
            .expect("local branch should point to a commit");

        if repo.find_remote("origin").is_err() {
            repo.remote("origin", "file:///tmp/origin.git")?;
        }
        repo.reference(
            &remote_ref,
            branch_target,
            true,
            "create remote-tracking ref",
        )?;

        let mut local_branch = repo.find_branch(branch_name, BranchType::Local)?;
        local_branch.set_upstream(Some(&format!("origin/{branch_name}")))?;

        Ok(())
    }

    fn create_remote_tracking_branch(
        repo: &Repository,
        branch_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let branch = repo.find_branch(branch_name, BranchType::Local)?;
        let target = branch
            .get()
            .target()
            .expect("local branch should point to a commit");
        create_remote_tracking_branch_from_target(repo, branch_name, target)
    }

    fn create_remote_tracking_branch_from_target(
        repo: &Repository,
        branch_name: &str,
        target: Oid,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if repo.find_remote("origin").is_err() {
            repo.remote("origin", "file:///tmp/origin.git")?;
        }
        repo.reference(
            &format!("refs/remotes/origin/{branch_name}"),
            target,
            true,
            "create remote-tracking ref",
        )?;
        Ok(())
    }

    fn with_cwd<T>(
        dir: &std::path::Path,
        f: impl FnOnce() -> Result<T, Box<dyn std::error::Error>>,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let _cwd_lock = CWD_LOCK.lock().expect("cwd lock should not be poisoned");
        let cwd = std::env::current_dir()?;
        std::env::set_current_dir(dir)?;
        let result = f();
        std::env::set_current_dir(cwd)?;
        result
    }

    #[test]
    fn head_branch_is_always_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let v = create_local_branch_visitor(&repo)?;
        let branch = repo.find_branch("main", BranchType::Local)?;

        assert!(!v.match_branch(&branch)?);

        Ok(())
    }

    #[test]
    fn protected_branches_are_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let v = create_local_branch_visitor(&repo)?;
        let branch = repo.find_branch("develop", BranchType::Local)?;

        assert!(!v.match_branch(&branch)?);

        let branch = repo.find_branch("release/v1", BranchType::Local)?;
        assert!(!v.match_branch(&branch)?);

        Ok(())
    }

    #[test]
    fn protected_branches_are_ignored_even_with_prefix_filters(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let mut v = create_local_branch_visitor(&repo)?;
        v.branches = vec!["release/".to_owned()];
        let branch = repo.find_branch("release/v1", BranchType::Local)?;

        assert!(!v.match_branch(&branch)?);

        Ok(())
    }

    #[test]
    fn unprotected_branch_can_match_prefix_filter() -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let mut v = create_local_branch_visitor(&repo)?;
        v.branches = vec!["feature/".to_owned()];
        let branch = repo.find_branch("feature/old", BranchType::Local)?;

        assert!(v.match_branch(&branch)?);

        Ok(())
    }

    #[test]
    fn unprotected_branch_matches_when_prefix_filters_are_unset(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let v = create_local_branch_visitor(&repo)?;
        let branch = repo.find_branch("feature/old", BranchType::Local)?;

        assert!(v.match_branch(&branch)?);

        Ok(())
    }

    #[test]
    fn unprotected_branch_is_ignored_when_prefix_filters_do_not_match(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let mut v = create_local_branch_visitor(&repo)?;
        v.branches = vec!["bugfix/".to_owned()];
        let branch = repo.find_branch("feature/old", BranchType::Local)?;

        assert!(!v.match_branch(&branch)?);

        Ok(())
    }

    #[test]
    fn for_each_branches_without_since_selects_only_branches_without_upstream(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        track_branch(&repo, "feature/old")?;
        let v = create_local_branch_visitor(&repo)?;

        let selected = v.for_each_branches(&repo, Vec::new(), |mut names, branch| {
            names.push(branch.name()?.unwrap().to_owned());
            Ok(names)
        })?;

        assert!(selected.is_empty());

        Ok(())
    }

    #[test]
    fn for_each_branches_with_since_can_select_branches_with_upstream(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        track_branch(&repo, "feature/old")?;
        let mut v = create_local_branch_visitor(&repo)?;
        v.since = Some(Local::now() + Duration::days(1));

        let selected = v.for_each_branches(&repo, Vec::new(), |mut names, branch| {
            names.push(branch.name()?.unwrap().to_owned());
            Ok(names)
        })?;

        assert_eq!(vec!["feature/old".to_owned()], selected);

        Ok(())
    }

    #[test]
    fn remote_head_is_always_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let main = repo.find_branch("main", BranchType::Local)?;
        let main = main
            .get()
            .target()
            .expect("local branch should point to a commit");
        create_remote_tracking_branch_from_target(&repo, "HEAD", main)?;
        let v = create_remote_branch_visitor(&repo)?;
        let remote_ref = RemoteRef::new("refs/remotes/origin/HEAD".to_owned())?;

        assert!(!v.match_branch(&remote_ref)?);

        Ok(())
    }

    #[test]
    fn remote_default_branch_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        repo.config()?
            .open_level(ConfigLevel::Local)?
            .set_str("init.defaultbranch", "main")?;
        create_remote_tracking_branch(&repo, "main")?;
        let v = create_remote_branch_visitor(&repo)?;
        let remote_ref = RemoteRef::new("refs/remotes/origin/main".to_owned())?;

        assert!(!v.match_branch(&remote_ref)?);

        Ok(())
    }

    #[test]
    fn remote_protected_branches_are_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        create_remote_tracking_branch(&repo, "develop")?;
        let v = create_remote_branch_visitor(&repo)?;
        let remote_ref = RemoteRef::new("refs/remotes/origin/develop".to_owned())?;

        assert!(!v.match_branch(&remote_ref)?);

        Ok(())
    }

    #[test]
    fn remote_branch_matches_prefix_filter_on_short_name() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_tmpdir, repo) = create_repo()?;
        create_remote_tracking_branch(&repo, "feature/old")?;
        let mut v = create_remote_branch_visitor(&repo)?;
        v.branches = vec!["feature/".to_owned()];
        let remote_ref = RemoteRef::new("refs/remotes/origin/feature/old".to_owned())?;

        assert!(v.match_branch(&remote_ref)?);

        Ok(())
    }

    #[test]
    fn remote_for_each_branches_lists_origin_tracking_refs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        create_remote_tracking_branch(&repo, "feature/old")?;
        let v = create_remote_branch_visitor(&repo)?;

        let selected = v.for_each_branches(&repo, Vec::new(), |mut names, remote_ref, _| {
            names.push(format!("{}/{}", remote_ref.remote(), remote_ref.branch()));
            Ok(names)
        })?;

        assert_eq!(vec!["origin/feature/old".to_owned()], selected);

        Ok(())
    }

    #[test]
    fn remote_for_each_branches_ignores_non_origin_tracking_refs(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (_tmpdir, repo) = create_repo()?;
        let feature = repo.find_branch("feature/old", BranchType::Local)?;
        let feature = feature
            .get()
            .target()
            .expect("local branch should point to a commit");
        repo.remote("upstream", "file:///tmp/upstream.git")?;
        repo.reference(
            "refs/remotes/upstream/feature/old",
            feature,
            true,
            "create remote-tracking ref",
        )?;
        let v = create_remote_branch_visitor(&repo)?;

        let selected = v.for_each_branches(&repo, Vec::new(), |mut names, remote_ref, _| {
            names.push(format!("{}/{}", remote_ref.remote(), remote_ref.branch()));
            Ok(names)
        })?;

        assert!(selected.is_empty());

        Ok(())
    }

    #[test]
    fn cli_rejects_remote_without_since() -> Result<(), Box<dyn std::error::Error>> {
        let (tmpdir, _repo) = create_repo()?;
        let result = with_cwd(tmpdir.path(), || {
            let cli = Cli::try_parse_from(["git-stale", "--remote"])?;
            cli.into_command()
        });
        let err = match result {
            Ok(_) => panic!("should reject missing --since"),
            Err(err) => err,
        };

        assert_eq!("--remote requires --since", err.to_string());

        Ok(())
    }

    #[test]
    fn cli_rejects_remote_delete_without_push() -> Result<(), Box<dyn std::error::Error>> {
        let (tmpdir, repo) = create_repo()?;
        repo.remote("origin", "file:///tmp/origin.git")?;
        let result = with_cwd(tmpdir.path(), || {
            let cli = Cli::try_parse_from(["git-stale", "--remote", "--since", "3mo", "--delete"])?;
            cli.into_command()
        });
        let err = match result {
            Ok(_) => panic!("should reject remote delete without push"),
            Err(err) => err,
        };

        assert_eq!("--remote --delete requires --push", err.to_string());

        Ok(())
    }

    #[test]
    fn cli_rejects_remote_mode_without_origin_remote() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let _repo = Repository::init(tmpdir.path())?;
        let result = with_cwd(tmpdir.path(), || {
            let cli = Cli::try_parse_from(["git-stale", "--remote", "--since", "3mo"])?;
            cli.into_command()
        });
        let err = match result {
            Ok(_) => panic!("should reject missing origin"),
            Err(err) => err,
        };

        assert_eq!("origin remote does not exist", err.to_string());

        Ok(())
    }

    #[test]
    fn local_mode_behavior_is_unchanged() -> Result<(), Box<dyn std::error::Error>> {
        let (tmpdir, _repo) = create_repo()?;
        let command = with_cwd(tmpdir.path(), || {
            let cli = Cli::try_parse_from(["git-stale", "--delete"])?;
            cli.into_command()
        })?;

        assert!(matches!(command, Command::DeleteLocalBranches { .. }));

        Ok(())
    }

    #[test]
    fn remote_delete_push_builds_origin_deletion_refspec() -> Result<(), Box<dyn std::error::Error>>
    {
        let (_tmpdir, repo) = create_repo()?;
        repo.remote("origin", "file:///tmp/origin.git")?;
        create_remote_tracking_branch(&repo, "feature/old")?;
        let mut visitor = create_remote_branch_visitor(&repo)?;
        visitor.branches = vec!["feature/".to_owned()];
        let refspecs =
            visitor.for_each_branches(&repo, Vec::new(), |mut refspecs, remote_ref, _| {
                refspecs.push(format!(":{}", remote_ref.branch()));
                Ok(refspecs)
            })?;

        assert_eq!(vec![":feature/old".to_owned()], refspecs);

        Ok(())
    }
}
