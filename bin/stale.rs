use chrono::{DateTime, Local};
use clap::Parser;
use git2::{Branch, BranchType, PushOptions, RemoteCallbacks, Repository};
use git_toolbox::{
    config::Configuration, config::ProtectedBranches, git::GitTime, reltime::Reltime,
};
use log::{error, info, warn};
use std::{collections::HashMap, error::Error, process::exit};

#[derive(Parser)]
#[command(
    about = "List or delete stale branches",
    long_about = None)]
struct Cli {
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
                    let mut callbacks = RemoteCallbacks::new();
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
        }
    }
}

struct LocalBranchVisitor {
    since: Option<DateTime<Local>>,
    branches: Vec<String>,
    protected_branches: Option<ProtectedBranches>,
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

impl Cli {
    fn into_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let config = repo.config()?;
        let protected_branches = Configuration::new(&config).dah_protected_branches()?;
        let now = Local::now();
        let since = self.since.map(|s| now - s);
        let visitor = LocalBranchVisitor {
            since,
            branches: self.branches,
            protected_branches,
        };

        let command = if self.delete && self.push {
            Command::DeleteUpstreamBranches { repo, visitor }
        } else if self.delete {
            Command::DeleteLocalBranches { repo, visitor }
        } else {
            Command::ListLocalBranches { repo, visitor }
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
    use super::LocalBranchVisitor;
    use chrono::{Duration, Local};
    use git2::{BranchType, ConfigLevel, Repository, Signature};
    use tempfile::TempDir;

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
}
