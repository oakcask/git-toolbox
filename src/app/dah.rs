mod statemachine;

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
pub enum RepositoryCollectorError {
    #[error("commit inspection terminated")]
    CommitInspectionTerminated,
    #[error("{0}")]
    InternalError(#[from] git2::Error),
}

pub struct RepositoryCollector<'a> {
    repo: &'a Repository,
    walk_limit: usize,
}

impl RepositoryCollector<'_> {
    pub fn new(repo: &Repository) -> RepositoryCollector<'_> {
        RepositoryCollector {
            repo,
            walk_limit: 100usize,
        }
    }

    pub fn with_walk_limit(self, num_commits: usize) -> Self {
        Self {
            walk_limit: num_commits,
            ..self
        }
    }
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

impl<'a> Collector for RepositoryCollector<'a> {
    type Error = RepositoryCollectorError;

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
        if let Some(upstream) = get_upstream_branch(head)? {
            let upstream = upstream.into_reference();
            let upstream_head = upstream.peel_to_commit()?.id();
            let mut walk = self.repo.revwalk()?;

            walk.push(self.repo.head()?.peel_to_commit()?.id())?;
            walk.hide(upstream_head)?;
            walk.set_sorting(Sort::TOPOLOGICAL)?;

            info!(
                "searching {}({}) from history of HEAD...",
                upstream.name().unwrap_or_default(),
                upstream_head
            );

            let mut count = self.walk_limit;
            for oid in walk {
                if count == 0 {
                    return Err(RepositoryCollectorError::CommitInspectionTerminated);
                }
                let commit = self.repo.find_commit(oid?)?;
                let time: GitTime = commit.time().into();
                let time: DateTime<FixedOffset> = time.into();

                info!(
                    " * {} author={} time={}",
                    commit.id(),
                    commit.author(),
                    time
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
        let statuses = self.repo.statuses(Some(
            StatusOptions::default().show(StatusShow::IndexAndWorkdir),
        ))?;
        // merge all statuses
        Ok(statuses
            .iter()
            .map(|st| st.status())
            .fold(Status::CURRENT, |a, b| a | b))
    }
}

pub struct Application {
    pub repo: Repository,
    pub step: bool,
    pub limit: usize,
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
    pub fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        env_logger::init();

        loop {
            let collector = RepositoryCollector::new(&self.repo).with_walk_limit(self.limit);
            let action = Action::new(collector)?;
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
        let mesg = commit.message().and_then(|m| m.lines().next());
        let random = Ulid::new().to_string().to_ascii_lowercase();
        if let Some(mesg) = mesg {
            let mesg = Regex::new(r#"\s+"#).unwrap().replace_all(mesg, "-");
            let mesg = Regex::new(r#"[^-\w]"#).unwrap().replace_all(&mesg, "_");

            let mut mesg = mesg.to_lowercase();
            mesg.push_str("-dah");
            mesg.push_str(&random);
            Ok(mesg)
        } else {
            let mut mesg = String::from("dah");
            mesg.push_str(&random);
            Ok(mesg)
        }
    }

    fn run_command(&self, command: &mut std::process::Command) -> Result<(), ApplicationError> {
        let cmdline = get_command_line(command);
        info!("invoking {:?}", cmdline);

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
                std::process::Command::new("git")
                    .arg("push")
                    .arg("--force-with-lease")
                    .arg("--force-if-includes")
                    .arg("-u")
                    .arg(upstream_ref.remote())
                    .arg(head_ref.branch().unwrap()),
            )
        } else {
            self.run_command(
                std::process::Command::new("git")
                    .arg("push")
                    .arg("--force-with-lease")
                    .arg("--force-if-includes")
                    .arg("-u")
                    .arg("origin")
                    .arg(head_ref.branch().unwrap()),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use git2::{ConfigLevel, ObjectType, Repository, Signature};
    use tempfile::TempDir;
    use ulid::Ulid;

    use crate::{app::dah::Application, git::GitTime};

    use super::{fnmatch, statemachine::Collector, RepositoryCollector};

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
            let author = Signature::new("foo", "foo@example.com", GitTime::now().as_ref()).unwrap();
            let tree = repo.treebuilder(None).unwrap();
            let tree = tree.write().unwrap();
            let tree = repo.find_tree(tree).unwrap();
            repo.commit(Some("refs/heads/main"), &author, &author, "Initial commit", &tree, &[]).unwrap();
            repo.set_head("refs/heads/main").unwrap();
        }

        let app = Application {
            repo,
            step: true,
            limit: 1,
        };
        let got = app.generate_branch_name().unwrap();

        if let Some(ulid) = got.strip_prefix("initial-commit-dah") {
            assert!(Ulid::from_string(ulid).is_ok(), "expected {:?} to have ULID suffix", got);
        } else {
            unreachable!("expected {:?} to have {:?}", got, "initial-commit-dah");
        }
    }

    #[test]
    fn repository_collector_default_branch_returns_git_config_init_defaultbranch() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo = Repository::init_bare(tmpdir.path())?;

        repo.config()?.open_level(ConfigLevel::Local)?.set_str("init.defaultbranch", "foo")?;

        let got = RepositoryCollector::new(&repo).default_branch()?;
        let got = got.as_ref().map(|s| s.as_str());

        assert_eq!(Some("foo"), got);

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
    fn repository_collector_is_head_protected() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo = Repository::init_bare(tmpdir.path())?;
        repo.config()?.open_level(ConfigLevel::Local)?.set_str("dah.protectedbranch", "develop:release/*")?;

        let author = Signature::new("foo", "foo@example.com", GitTime::now().as_ref())?;
        let tree = repo.treebuilder(None)?;
        let tree = tree.write()?;
        let tree = repo.find_tree(tree)?;
        repo.commit(Some("refs/heads/develop"), &author, &author, "develop", &tree, &[])?;
        let oid = repo.commit(Some("refs/heads/release/v1"), &author, &author, "release v1", &tree, &[])?;
        repo.commit(Some("refs/heads/release/v2"), &author, &author, "release v2", &tree, &[])?;
        repo.commit(Some("refs/heads/release-latest"), &author, &author, "release latest", &tree, &[])?;
        repo.tag("v1", &repo.find_object(oid, Some(ObjectType::Commit))?, &author, "tag v1", true)?;

        // listed
        repo.set_head("refs/heads/develop")?;
        assert!(RepositoryCollector::new(&repo).is_head_protected()?);

        // listed
        repo.set_head("refs/heads/release/v1")?;
        assert!(RepositoryCollector::new(&repo).is_head_protected()?);

        // listed
        repo.set_head("refs/heads/release/v2")?;
        assert!(RepositoryCollector::new(&repo).is_head_protected()?);

        // not listed
        repo.set_head("refs/heads/release-latest")?;
        assert!(!RepositoryCollector::new(&repo).is_head_protected()?);

        // detached
        repo.set_head("refs/tags/v1")?;
        assert!(!RepositoryCollector::new(&repo).is_head_protected()?);

        Ok(())
    }
}
