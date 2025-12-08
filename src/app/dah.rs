mod statemachine;

use crate::git::credentials::CredentialCallback;
use crate::git::{GitTime, HeadRef, RemoteRef};
use chrono::{DateTime, FixedOffset};
use fnmatch_sys::{self, FNM_NOESCAPE};
use git2::{Branch, ErrorCode, Repository, Sort, Status, StatusOptions, StatusShow};
use log::{error, info, warn};
use regex::Regex;
use statemachine::StepResult;
use statemachine::{Action, Collector, Dispatcher};
use std::{
    ffi::{CStr, CString, OsString},
    process::Stdio,
};
use ulid::Ulid;

#[derive(thiserror::Error, Debug)]
pub enum RepositoryStateError {
    #[error(
        "the number of commits to inspect was limited by {limit}. Increasing the limit with `--limit` may help."
    )]
    HistoryInspectionLimitExceeded { limit: usize },
    #[error("{0}")]
    InternalError(#[from] git2::Error),
}

fn get_upstream_branch(reference: git2::Reference<'_>) -> Result<Option<Branch<'_>>, git2::Error> {
    if reference.is_branch() {
        match Branch::wrap(reference).upstream() {
            Ok(upstream) => Ok(Some(upstream)),
            Err(e) => {
                if e.code() == git2::ErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    } else {
        Ok(None)
    }
}

fn fnmatch(pat: &CStr, s: &CStr) -> bool {
    let pat = pat.as_ptr();
    let s = s.as_ptr();

    unsafe { fnmatch_sys::fnmatch(pat, s, FNM_NOESCAPE) == 0 }
}

impl Collector for Application {
    type Error = RepositoryStateError;

    fn default_branch(&self) -> Result<Option<String>, Self::Error> {
        self.repo.config()?.get_string("init.defaultbranch")
            .map(Some)
            .or_else(|e| {
                if e.code() == ErrorCode::NotFound {
                    warn!("init.defaultbranch is unset; git-dah guesses the default branch name by this config");
                    Ok(None)
                } else {
                    Err(e.into())
                }
            })
    }

    fn is_remote_head(&self) -> Result<bool, Self::Error> {
        let head_ref = HeadRef::new(self.repo.head()?.name().unwrap().to_owned()).unwrap();

        if let Some(branch) = head_ref.branch() {
            for remote in self.repo.remotes()?.into_iter().flatten() {
                let mut remote = self.repo.find_remote(remote)?;
                let mut cb = git2::RemoteCallbacks::new();
                let config = self.repo.config()?;
                let mut cred_cb = CredentialCallback::new(config);
                cb.credentials(move |url, username, allowed_types| {
                    cred_cb.try_next(url, username, allowed_types)
                });
                remote.connect_auth(git2::Direction::Fetch, Some(cb), None)?;

                if let Ok(remote_default_branch) = remote.default_branch() {
                    if let Some(remote_default_branch) = remote_default_branch.as_str() {
                        let remote_default_branch = HeadRef::new(remote_default_branch).unwrap();
                        let remote_default_branch = remote_default_branch.branch().unwrap();
                        if branch == remote_default_branch {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    fn is_head_protected(&self) -> Result<bool, Self::Error> {
        let head_ref = HeadRef::new(self.repo.head()?.name().unwrap().to_owned()).unwrap();

        if let Some(branch) = head_ref.branch() {
            let config = self.repo.config()?;
            let config_protected = config.get_string("dah.protectedbranch")
                .map(Some)
                .or_else(|e| {
                    if e.code() == ErrorCode::NotFound {
                        warn!("dah.protectedbranch is unset; git-dah guesses the protected branch by this config");
                        Ok(None)
                    } else {
                        Err(e)
                    }
                })?;
            if let Some(config_protected) = config_protected {
                let branch_c_string = CString::new(branch).unwrap();
                let is_match = config_protected.split(':').any(|n| {
                    let pat = CString::new(n).unwrap();
                    fnmatch(pat.as_c_str(), branch_c_string.as_c_str())
                });
                if is_match {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn head_ref(&self) -> Result<HeadRef, Self::Error> {
        Ok(HeadRef::new(self.repo.head()?.name().unwrap().to_owned()).unwrap())
    }

    fn upstream_ref(&self) -> Result<Option<RemoteRef>, Self::Error> {
        let head = self.repo.head()?;
        if let Some(upstream) = get_upstream_branch(head)? {
            Ok(Some(
                RemoteRef::new(upstream.into_reference().name().unwrap().to_owned()).unwrap(),
            ))
        } else {
            Ok(None)
        }
    }

    fn is_synchronized(&self) -> Result<bool, Self::Error> {
        let head = self.repo.head()?;
        let head_oid = head.peel_to_commit()?.id();
        if let Some(upstream) = get_upstream_branch(head)? {
            Ok(head_oid == upstream.into_reference().peel_to_commit()?.id())
        } else {
            Ok(false)
        }
    }

    fn is_based_on_remote(&self) -> Result<bool, Self::Error> {
        let head = self.repo.head()?;
        let head_oid = head.peel_to_commit()?.id();
        if let Some(upstream) = get_upstream_branch(head)? {
            let upstream = upstream.into_reference();
            let upstream_commit = upstream.peel_to_commit()?;
            let upstream_head = upstream_commit.id();

            // check oid of head and the remote head first.
            if upstream_head == head_oid {
                info!("no commits on local branch.");
                return Ok(true);
            }

            // when force push is allowed,
            // search reflog to find out if the remote tracking branch's ref is included
            if self.allow_force_push {
                info!(
                    "searching {}({}) from reflog of HEAD...",
                    upstream.name().unwrap_or_default(),
                    upstream_head
                );
                let upstream_head_time = GitTime::from(upstream_commit.time());
                for ent in self
                    .repo
                    .reflog(self.head_ref()?.as_str())?
                    .iter()
                    .filter(|e| upstream_head_time <= e.committer().when().into())
                {
                    info!(
                        " * {} time={:?} message={:?}",
                        ent.id_new(),
                        DateTime::<FixedOffset>::from(GitTime::from(ent.committer().when())),
                        ent.message(),
                    );

                    if ent.id_old() == upstream_head {
                        info!("DONE");
                        return Ok(true);
                    }
                }
            }

            // as the plan B, search history
            let mut walk = self.repo.revwalk()?;
            walk.push(self.repo.head()?.peel_to_commit()?.id())?;
            walk.hide(upstream_head)?;
            walk.set_sorting(Sort::TOPOLOGICAL)?;

            info!(
                "searching {}({}) from history of HEAD...",
                upstream.name().unwrap_or_default(),
                upstream_head
            );

            let mut count = self.limit;
            for oid in walk {
                if count == 0 {
                    return Err(RepositoryStateError::HistoryInspectionLimitExceeded {
                        limit: self.limit,
                    });
                }
                let commit = self.repo.find_commit(oid?)?;

                info!(
                    " * {} author={} time={}",
                    commit.id(),
                    commit.author(),
                    DateTime::<FixedOffset>::from(GitTime::from(commit.time())),
                );
                if commit
                    .parents()
                    .map(|o| o.id())
                    .any(|id| id == upstream_head)
                {
                    info!("DONE");
                    return Ok(true);
                }

                count -= 1;
            }
        }

        Ok(false)
    }

    fn status(&self) -> Result<Status, Self::Error> {
        let mut option = StatusOptions::default();
        let option = &mut option;
        if self.allow_stage {
            option.show(StatusShow::IndexAndWorkdir);
        } else {
            // ignore the work directory status to touch staged changes only.
            option.show(StatusShow::Index);
        }
        let statuses = self.repo.statuses(Some(option))?;
        // merge all statuses
        Ok(statuses
            .iter()
            .map(|st| st.status())
            .fold(Status::CURRENT, |a, b| a | b))
    }
}

pub struct Application {
    repo: Repository,
    step: bool,
    limit: usize,
    allow_force_push: bool,
    allow_stage: bool,
    fetch_first: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum ApplicationError {
    #[error("{command:?} failed with exit code {code:?}")]
    ExitStatus {
        command: OsString,
        code: Option<i32>,
    },
    #[error("internal error: {0}")]
    IO(#[from] std::io::Error),
    #[error("internal error: {0}")]
    Git(#[from] git2::Error),
}

fn get_command_line(command: &std::process::Command) -> OsString {
    let mut cmd = command.get_program().to_owned();
    for arg in command.get_args() {
        cmd.push(" ");
        cmd.push(arg);
    }
    cmd
}

impl Application {
    pub fn new(repo: Repository) -> Self {
        Application {
            repo,
            step: false,
            limit: 100,
            allow_force_push: true,
            allow_stage: true,
            fetch_first: true,
        }
    }

    pub fn with_step(self, step: bool) -> Self {
        Self { step, ..self }
    }

    pub fn with_limit(self, limit: usize) -> Self {
        Self { limit, ..self }
    }

    pub fn with_allow_force_push(self, allow_force_push: bool) -> Self {
        Self {
            allow_force_push,
            ..self
        }
    }

    pub fn with_fetch_first(self, fetch_first: bool) -> Self {
        Self {
            fetch_first,
            ..self
        }
    }

    pub fn with_allow_stage(self, allow_stage: bool) -> Self {
        Self {
            allow_stage,
            ..self
        }
    }

    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        env_logger::init();

        if self.fetch_first {
            if let Err(e) = self.run_command(std::process::Command::new("git").arg("fetch")) {
                error!("fetch failed: {e:?}; but we'll continue.");
            }
        }

        loop {
            let action = Action::new(&self)?;
            match statemachine::step(action, &self)? {
                StepResult::Stop => break,
                StepResult::Continue => {
                    if self.step {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn generate_branch_name(&self) -> Result<String, ApplicationError> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        let prefix = self
            .repo
            .config()?
            .get_string("dah.branchprefix")
            .or_else(|e| {
                if e.code() == ErrorCode::NotFound {
                    Ok(String::new())
                } else {
                    Err(e)
                }
            })?;

        let mesg = commit.message().and_then(|m| m.lines().next());
        Ok(generate_branch_name_from_commit_message(prefix, mesg))
    }

    fn new_git_push_command_with_force_options(&self) -> std::process::Command {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("push");

        if self.allow_force_push {
            cmd.arg("--force-with-lease").arg("--force-if-includes");
        }

        cmd
    }

    fn run_command(&self, command: &mut std::process::Command) -> Result<(), ApplicationError> {
        let cmdline = get_command_line(command);
        info!("invoking {cmdline:?}");

        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            Err(ApplicationError::ExitStatus {
                command: cmdline,
                code: status.code(),
            })
        }
    }
}

impl Dispatcher for Application {
    type Error = ApplicationError;

    fn status(&self) -> Result<(), Self::Error> {
        self.run_command(std::process::Command::new("git").arg("status"))
    }

    fn create_branch_and_switch(&self) -> Result<(), Self::Error> {
        let branch_name = self.generate_branch_name()?;
        self.run_command(
            std::process::Command::new("git")
                .arg("switch")
                .arg("-c")
                .arg(branch_name),
        )
    }

    fn rename_branch_and_switch(&self) -> Result<(), Self::Error> {
        let branch_name = self.generate_branch_name()?;
        self.run_command(
            std::process::Command::new("git")
                .arg("branch")
                .arg("-m")
                .arg(branch_name),
        )
    }

    fn stage_changes(&self) -> Result<(), Self::Error> {
        self.run_command(std::process::Command::new("git").arg("add").arg("-u"))
    }

    fn commit(&self) -> Result<(), Self::Error> {
        self.run_command(std::process::Command::new("git").arg("commit"))
    }

    fn pull_with_rebase(&self, upstream_ref: &str) -> Result<(), Self::Error> {
        // TODO: receive RemoteRef
        let upstream_ref = RemoteRef::new(upstream_ref).unwrap();
        self.run_command(
            std::process::Command::new("git")
                .arg("pull")
                .arg("--rebase")
                .arg(upstream_ref.remote())
                .arg(upstream_ref.branch()),
        )
    }

    fn push(&self, head_ref: &str, upstream_ref: Option<&str>) -> Result<(), Self::Error> {
        let head_ref = HeadRef::new(head_ref).unwrap();
        if let Some(upstream_ref) = upstream_ref {
            let upstream_ref = RemoteRef::new(upstream_ref).unwrap();
            self.run_command(
                self.new_git_push_command_with_force_options()
                    .arg("-u")
                    .arg(upstream_ref.remote())
                    .arg(head_ref.branch().unwrap()),
            )
        } else {
            self.run_command(
                self.new_git_push_command_with_force_options()
                    .arg("-u")
                    .arg("origin")
                    .arg(head_ref.branch().unwrap()),
            )
        }
    }
}

fn generate_branch_name_from_commit_message(prefix: String, mesg: Option<&str>) -> String {
    let mut branch_name = prefix;

    if let Some(mesg) = mesg {
        let mesg = Regex::new(r#"\s+"#).unwrap().replace_all(mesg, "-");
        let mesg = Regex::new(r#"[^-_.0-9a-zA-Z]"#)
            .unwrap()
            .replace_all(&mesg, "_");
        let mesg = mesg.to_lowercase();
        branch_name.push_str(&mesg);
        branch_name.push_str("-dah");
    } else {
        branch_name.push_str("dah");
    }

    let mut random = Ulid::new().to_string();
    random.make_ascii_lowercase();
    branch_name.push_str(&random);

    branch_name
}

#[cfg(test)]
mod tests {

    use std::{
        fs,
        path::Path,
        process::Command,
    };

    use git2::{
        build::{CloneLocal, RepoBuilder},
        ConfigLevel, ObjectType, Repository, Signature,
    };

    use regex::Regex;
    use tempfile::TempDir;
    use ulid::Ulid;
    use url::Url;

    use crate::app::dah::Application;

    use super::{fnmatch, generate_branch_name_from_commit_message, statemachine::Collector};

    #[test]
    fn test_fnmatch() {
        let cases = [(c"foo/*", c"foo/bar/baz")];

        for (pat, s) in cases {
            assert!(fnmatch(pat, s))
        }
    }

    #[test]
    fn application_generate_branch_name() {
        let tmpdir = TempDir::new().unwrap();
        let repo = Repository::init_bare(tmpdir.path()).unwrap();
        {
            let author = Signature::now("foo", "foo@example.com").unwrap();
            let tree = repo.treebuilder(None).unwrap();
            let tree = tree.write().unwrap();
            let tree = repo.find_tree(tree).unwrap();
            repo.commit(
                Some("refs/heads/main"),
                &author,
                &author,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();
            repo.set_head("refs/heads/main").unwrap();
        }

        let app = Application::new(repo).with_step(true).with_limit(1);
        let got = app.generate_branch_name().unwrap();

        if let Some(ulid) = got.strip_prefix("initial-commit-dah") {
            assert!(
                Ulid::from_string(ulid).is_ok(),
                "expected {got:?} to have ULID suffix"
            );
        } else {
            unreachable!(
                "expected {:?} to have prefix {:?}",
                got, "initial-commit-dah"
            );
        }
    }

    #[test]
    fn application_generate_branch_name_prefixes_by_git_config_dah_branchprefix() {
        let tmpdir = TempDir::new().unwrap();
        let repo = Repository::init_bare(tmpdir.path()).unwrap();
        repo.config()
            .unwrap()
            .open_level(ConfigLevel::Local)
            .unwrap()
            .set_str("dah.branchprefix", "feature/")
            .unwrap();

        {
            let author = Signature::now("foo", "foo@example.com").unwrap();
            let tree = repo.treebuilder(None).unwrap();
            let tree = tree.write().unwrap();
            let tree = repo.find_tree(tree).unwrap();
            repo.commit(
                Some("refs/heads/main"),
                &author,
                &author,
                "add something",
                &tree,
                &[],
            )
            .unwrap();
            repo.set_head("refs/heads/main").unwrap();
        }

        let app = Application::new(repo).with_step(true).with_limit(1);
        let got = app.generate_branch_name().unwrap();

        if let Some(ulid) = got.strip_prefix("feature/add-something-dah") {
            assert!(
                Ulid::from_string(ulid).is_ok(),
                "expected {got:?} to have ULID suffix"
            );
        } else {
            unreachable!(
                "expected {:?} to have prefix {:?}",
                got, "feature/add-something-dah"
            );
        }
    }

    #[test]
    fn application_default_branch_returns_git_config_init_defaultbranch(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo = Repository::init_bare(tmpdir.path())?;

        repo.config()?
            .open_level(ConfigLevel::Local)?
            .set_str("init.defaultbranch", "foo")?;

        let got = Application::new(repo).default_branch()?;
        let got = got.as_deref();

        assert_eq!(Some("foo"), got);

        Ok(())
    }

    #[test]
    fn application_is_remote_head() -> Result<(), Box<dyn std::error::Error>> {
        let origin = TempDir::new()?;
        let origin_path = origin.path();
        let origin = Repository::init_bare(origin_path)?;
        let local = TempDir::new()?;
        let local = Repository::init_bare(local.path())?;

        let author = Signature::now("foo", "foo@example.com")?;
        let tree = origin.treebuilder(None)?;
        let tree = tree.write()?;
        let tree = origin.find_tree(tree)?;
        origin.commit(Some("refs/heads/main"), &author, &author, "c1", &tree, &[])?;
        origin.set_head("refs/heads/main")?;

        let origin_path = origin_path.to_str().unwrap();
        {
            // git remote add origin file://{origin_path}
            let mut remote = local.remote("origin", &format!("file://{origin_path}"))?;
            // git fetch origin main:refs/remotes/origin/main
            remote.fetch(&["main:refs/remotes/origin/main"], None, None)?;
            // git switch -c main --track origin/main
            let remote_branch = local.find_branch("origin/main", git2::BranchType::Remote)?;
            let remote_head = remote_branch.get().peel_to_commit()?;
            let _ = local.branch("main", &remote_head, true)?;
            local.set_head("refs/heads/main")?;
        }

        assert!(
            Application::new(local).is_remote_head()?,
            "expected local's HEAD is remote HEAD"
        );

        Ok(())
    }

    // given:
    //   - config: dah.protectedbranch=develop:release/*
    //   - branches:
    //     - develop
    //     - release/v1 (also tagged as v1)
    //     - release/v2
    //     - release-latest
    //
    // when HEAD is refs/heads/develop then HEAD is protected it matches
    // when HEAD is refs/heads/release/v1 then HEAD is protected beacause it matches
    // when HEAD is refs/heads/release/v2 then HEAD is protected beacause it matches
    // when HEAD is refs/heads/release-latest then HEAD is NOT protected because it doesn't match
    // when HEAD is refs/tags/v1 then HEAD is NOT protected because it is detached
    #[test]
    fn application_is_head_protected() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo = Repository::init_bare(tmpdir.path())?;
        repo.config()?
            .open_level(ConfigLevel::Local)?
            .set_str("dah.protectedbranch", "develop:release/*")?;

        let author = Signature::now("foo", "foo@example.com")?;
        let tree = repo.treebuilder(None)?;
        let tree = tree.write()?;
        let tree = repo.find_tree(tree)?;
        repo.commit(
            Some("refs/heads/develop"),
            &author,
            &author,
            "develop",
            &tree,
            &[],
        )?;
        let oid = repo.commit(
            Some("refs/heads/release/v1"),
            &author,
            &author,
            "release v1",
            &tree,
            &[],
        )?;
        repo.commit(
            Some("refs/heads/release/v2"),
            &author,
            &author,
            "release v2",
            &tree,
            &[],
        )?;
        repo.commit(
            Some("refs/heads/release-latest"),
            &author,
            &author,
            "release latest",
            &tree,
            &[],
        )?;
        repo.tag(
            "v1",
            &repo.find_object(oid, Some(ObjectType::Commit))?,
            &author,
            "tag v1",
            true,
        )?;

        let repo = Repository::open_bare(tmpdir.path())?;
        // listed
        repo.set_head("refs/heads/develop")?;
        assert!(Application::new(repo).is_head_protected()?);

        // listed
        let repo = Repository::open_bare(tmpdir.path())?;
        repo.set_head("refs/heads/release/v1")?;
        assert!(Application::new(repo).is_head_protected()?);

        // listed
        let repo = Repository::open_bare(tmpdir.path())?;
        repo.set_head("refs/heads/release/v2")?;
        assert!(Application::new(repo).is_head_protected()?);

        // not listed
        let repo = Repository::open_bare(tmpdir.path())?;
        repo.set_head("refs/heads/release-latest")?;
        assert!(!Application::new(repo).is_head_protected()?);

        // detached
        let repo = Repository::open_bare(tmpdir.path())?;
        repo.set_head("refs/tags/v1")?;
        assert!(!Application::new(repo).is_head_protected()?);

        Ok(())
    }

    #[test]
    fn application_is_based_on_remote() {
        let _ = env_logger::builder().is_test(true).try_init();

        let upstream_repo = TempDir::new().unwrap();
        let upstream_repo_path = upstream_repo.path();
        let upstream_repo = Repository::init_bare(upstream_repo_path).unwrap();
        {
            let author = Signature::now("foo", "foo@example.com").unwrap();
            let tree = upstream_repo.treebuilder(None).unwrap();
            let tree = tree.write().unwrap();
            let tree = upstream_repo.find_tree(tree).unwrap();
            let c1 = upstream_repo
                .commit(None, &author, &author, "1", &tree, &[])
                .unwrap();
            let c1 = upstream_repo.find_commit(c1).unwrap();
            let c2 = upstream_repo
                .commit(None, &author, &author, "2", &tree, &[&c1])
                .unwrap();
            let c2 = upstream_repo.find_commit(c2).unwrap();
            upstream_repo.branch("main", &c2, true).unwrap();
            upstream_repo.set_head("refs/heads/main").unwrap();
        }

        let mut upstream_repo_url = Url::parse("file:///").unwrap();
        upstream_repo_url.set_path(upstream_repo_path.canonicalize().unwrap().to_str().unwrap());
        let upstream_repo_url = upstream_repo_url.as_str();

        // just checking out remote branch, so head ref and remote ref is same.
        let repo = TempDir::new().unwrap();
        let repo = RepoBuilder::new()
            .bare(false)
            .clone_local(CloneLocal::Auto)
            .clone(upstream_repo_url, repo.path())
            .unwrap();
        {
            let repo = Repository::open(repo.path()).unwrap();
            assert!(Application::new(repo).is_based_on_remote().unwrap());
        }

        // then, adding local change to HEAD, still based on the remote tracking branch.
        {
            let author = Signature::now("foo", "foo@example.com").unwrap();
            let tree = repo.treebuilder(None).unwrap();
            let tree = tree.write().unwrap();
            let tree = repo.find_tree(tree).unwrap();
            let head = repo.head().unwrap().peel_to_commit().unwrap();
            repo.set_head("refs/heads/main").unwrap();
            repo.checkout_head(None).unwrap();
            repo.commit(
                Some("HEAD"),
                &author,
                &author,
                "local change",
                &tree,
                &[&head],
            )
            .unwrap();
        }
        assert!(Application::new(repo).is_based_on_remote().unwrap());

        // here, using new clone repository,
        // checkout the remote branch then reset to HEAD~ will cause diversion.
        let repo = TempDir::new().unwrap();
        let repo = RepoBuilder::new()
            .bare(false)
            .clone_local(CloneLocal::Auto)
            .clone(upstream_repo_url, repo.path())
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        repo.checkout_head(None).unwrap();
        repo.reset(
            repo.head()
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .parent(0)
                .unwrap()
                .as_object(),
            git2::ResetType::Hard,
            None,
        )
        .unwrap();
        assert!(!Application::new(repo)
            .with_allow_force_push(false)
            .is_based_on_remote()
            .unwrap());

        let repo = TempDir::new().unwrap();
        let repo = RepoBuilder::new()
            .bare(false)
            .clone_local(CloneLocal::Auto)
            .clone(upstream_repo_url, repo.path())
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();
        repo.checkout_head(None).unwrap();
        repo.reset(
            repo.head()
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .parent(0)
                .unwrap()
                .as_object(),
            git2::ResetType::Hard,
            None,
        )
        .unwrap();
        assert!(Application::new(repo)
            .with_allow_force_push(true)
            .is_based_on_remote()
            .unwrap());
    }

    #[test]
    fn application_status() {
        let repo = TempDir::new().unwrap();
        let repo_path = repo.path();

        // Arrange:
        // echo "initial" > staged
        // echo "initial" > unstaged
        // git add staged unstaged
        // git commit
        // echo "change" > staged
        // echo "change" > unstaged
        // git add staged
        {
            let repo = Repository::init(repo_path).unwrap();
            let odb = repo.odb().unwrap();
            let initial_oid = odb.write(ObjectType::Blob, "initial\n".as_bytes()).unwrap();
            let author = Signature::now("foo", "foo@example.com").unwrap();
            let mut tree = repo.treebuilder(None).unwrap();
            tree.insert("staged", initial_oid.clone(), 0o100644)
                .unwrap();
            tree.insert("unstaged", initial_oid.clone(), 0o100644)
                .unwrap();
            let tree = tree.write().unwrap();
            let tree = repo.find_tree(tree).unwrap();
            repo.commit(
                Some("refs/heads/main"),
                &author,
                &author,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();
            repo.set_head("refs/heads/main").unwrap();
            repo.checkout_head(None).unwrap();
            Command::new("cat")
                .arg(repo_path.join("staged").as_os_str())
                .spawn()
                .unwrap()
                .wait()
                .unwrap();

            fs::write(repo_path.join("staged"), "changed\n").unwrap();
            fs::write(repo_path.join("unstaged"), "changed\n").unwrap();
            let mut idx = repo.index().unwrap();
            idx.add_path(Path::new("staged")).unwrap();
            idx.write().unwrap();
        }
        // Act & assert
        // if allow_stage is false
        {
            let repo = Repository::open(repo_path).unwrap();
            let status = Application::new(repo)
                .with_allow_stage(false)
                .status()
                .unwrap();
            assert!(!status.is_wt_modified());
        }
        // Act & assert
        // if allow_stage is true
        {
            let repo = Repository::open(repo_path).unwrap();
            let status = Application::new(repo)
                .with_allow_stage(true)
                .status()
                .unwrap();
            assert!(status.is_wt_modified());
        }
    }

    #[test]
    fn test_generate_branch_name_from_commit_message() {
        let cases = [
            ("", Some("fix typos"), r#"\Afix-typos-dah[0-9a-z]{26}\z"#),
            ("", None, r#"\Adah[0-9a-z]{26}\z"#),
            (
                "hotfix/",
                Some("fix typos"),
                r#"\Ahotfix/fix-typos-dah[0-9a-z]{26}\z"#,
            ),
            ("hotfix/", None, r#"\Ahotfix/dah[0-9a-z]{26}\z"#),
            (
                "release/",
                Some("chore(main): v1.0"),
                r#"\Arelease/chore_main__-v1.0-dah[0-9a-z]{26}\z"#,
            ),
            ("release/", None, r#"\Arelease/dah[0-9a-z]{26}\z"#),
            // preserves underscore
            ("", Some("_"), r#"\A_-dah[0-9a-z]{26}\z"#),
            // create shell safe string
            (
                "",
                Some("あaいiうu\u{2764}\u{FE0F}\u{200D}\u{1F525}heart-on-fire"),
                r#"\A_a_i_u____heart-on-fire-dah[0-9a-z]{26}\z"#,
            ),
            (
                "",
                Some(r#"~`!@#$%^&*()+=[]{}\|;:'"<>,?/"#),
                r#"\A_____________________________-dah[0-9a-z]{26}\z"#,
            ),
        ];
        for (prefix, message, expected) in cases.into_iter() {
            let prefix = String::from(prefix);
            let actual = generate_branch_name_from_commit_message(prefix, message);
            let expected = Regex::new(expected).unwrap();
            assert!(
                expected.is_match(&actual),
                "expected to match {expected:?} but got {actual:?}"
            );
        }
    }
}
