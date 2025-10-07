use git2::Status;
use log::{info, warn};

use crate::git::{HeadRef, RemoteRef};

#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    ResolveConflict,
    CreateBranch,
    RenameBranch,
    StageChanges,
    Commit,
    Rebase {
        head_ref: HeadRef,
        upstream_ref: RemoteRef,
    },
    Push {
        head_ref: HeadRef,
        upstream_ref: Option<RemoteRef>,
    },
}

/// Group of methods to collect repository state,
/// for deciding the next action.
pub trait Collector {
    type Error;

    /// name of default branch
    fn default_branch(&self) -> Result<Option<String>, Self::Error>;

    /// check if the HEAD is remote's HEAD
    fn is_remote_head(&self) -> Result<bool, Self::Error>;

    /// check if the HEAD is protected
    fn is_head_protected(&self) -> Result<bool, Self::Error>;

    /// HEAD refname
    fn head_ref(&self) -> Result<HeadRef, Self::Error>;
    /// Refname of the remote tracking branch for HEAD if exists.
    ///
    /// It should be like `refs/remotes/origin/branch-name`
    fn upstream_ref(&self) -> Result<Option<RemoteRef>, Self::Error>;
    /// Check if the latest commit on HEAD and its remote tracking branch are same.
    ///
    /// For HEAD without remote tracking branch, should return `Ok(false)`.
    fn is_synchronized(&self) -> Result<bool, Self::Error>;
    /// Check if commits on the branch pointed by head_ref are at the top of
    /// the remote tracking branch (upstream_ref). i.e., HEAD is already
    /// rebased onto upstream_ref.
    ///
    /// For HEAD without remote tracking branch, should return `Ok(false)`.
    fn is_based_on_remote(&self) -> Result<bool, Self::Error>;
    /// Merged status of current index and work tree.
    fn status(&self) -> Result<Status, Self::Error>;
}

impl Action {
    pub fn new<T>(collector: &T) -> Result<Self, T::Error>
    where
        T: Collector,
    {
        let default_branch = collector.default_branch()?;
        let head_ref = collector.head_ref()?;
        let upstream_ref = collector.upstream_ref()?;
        let status = collector.status()?;

        let has_wt_change = status.is_wt_new()
            || status.is_wt_modified()
            || status.is_wt_deleted()
            || status.is_wt_renamed()
            || status.is_wt_typechange();
        let has_index_change = status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_index_renamed()
            || status.is_index_typechange();

        if status.is_conflicted() {
            return Ok(Self::ResolveConflict);
        }
        if has_wt_change {
            return Ok(Self::StageChanges);
        }
        if has_index_change {
            return Ok(Self::Commit);
        }

        if let Some(head_branch) = head_ref.branch() {
            if collector.is_synchronized()? {
                return Ok(Self::None);
            }
            if let Some(true) = default_branch.map(|b| head_branch == b) {
                info!("found local commits on default branch");
                return Ok(Self::RenameBranch);
            }
            if collector.is_remote_head()? {
                info!("found local commits on remote's default branch");
                return Ok(Self::RenameBranch);
            }
            if collector.is_head_protected()? {
                info!("found local commits on default or protected branch");
                return Ok(Self::RenameBranch);
            }

            if let Some(upstream_ref) = upstream_ref {
                if collector.is_based_on_remote()? {
                    return Ok(Self::Push {
                        head_ref,
                        upstream_ref: Some(upstream_ref),
                    });
                }
                return Ok(Self::Rebase {
                    head_ref,
                    upstream_ref,
                });
            } else {
                return Ok(Self::Push {
                    head_ref,
                    upstream_ref: None,
                });
            }
        }

        // detached HEAD
        Ok(Self::CreateBranch)
    }
}

pub trait Dispatcher {
    type Error;

    fn status(&self) -> Result<(), Self::Error>;
    fn create_branch_and_switch(&self) -> Result<(), Self::Error>;
    fn rename_branch_and_switch(&self) -> Result<(), Self::Error>;
    fn stage_changes(&self) -> Result<(), Self::Error>;
    fn commit(&self) -> Result<(), Self::Error>;
    fn pull_with_rebase(&self, upstream_ref: &str) -> Result<(), Self::Error>;
    fn push(&self, head_ref: &str, upstream_ref: Option<&str>) -> Result<(), Self::Error>;
}

pub enum StepResult {
    Stop,
    Continue,
}

pub fn step<D>(action: Action, dispatcher: &D) -> Result<StepResult, D::Error>
where
    D: Dispatcher,
{
    match action {
        Action::None => {
            info!("it's alright. happy hacking!");
            Ok(StepResult::Stop)
        }
        Action::ResolveConflict => {
            warn!("resolve conflict first.");
            dispatcher.status()?;
            Ok(StepResult::Stop)
        }
        Action::CreateBranch => {
            dispatcher.create_branch_and_switch()?;
            Ok(StepResult::Continue)
        }
        Action::RenameBranch => {
            info!("cleaning local changes on default branch by renaming it");
            dispatcher.rename_branch_and_switch()?;
            Ok(StepResult::Continue)
        }
        Action::StageChanges => {
            info!("there are unstaged changes");
            dispatcher.stage_changes()?;
            Ok(StepResult::Continue)
        }
        Action::Commit => {
            info!("there are staged changes");
            dispatcher.commit()?;
            Ok(StepResult::Continue)
        }
        Action::Rebase { upstream_ref, .. } => {
            dispatcher.pull_with_rebase(upstream_ref.as_str())?;
            Ok(StepResult::Continue)
        }
        Action::Push {
            head_ref,
            upstream_ref,
        } => {
            let upstream_ref = upstream_ref.as_ref().map(|o| o.as_str());
            dispatcher.push(head_ref.as_str(), upstream_ref)?;
            Ok(StepResult::Stop)
        }
    }
}

#[cfg(test)]
mod tests {
    use git2::Status;

    use crate::git::{HeadRef, RemoteRef};

    use super::{Action, Collector};

    #[derive(Debug, Clone, Default)]
    struct MockState {
        default_branch: Option<Option<String>>,
        protected_branches: Vec<String>,
        head_ref: Option<HeadRef>,
        upstream: Option<Option<(RemoteRef, bool, bool, bool)>>,
        status: Option<Status>,
    }

    impl MockState {
        fn with_default_branch(self, branch: &str) -> Self {
            Self {
                default_branch: Some(Some(branch.to_owned())),
                ..self
            }
        }

        fn with_protected_branch(self, branch: &str) -> Self {
            let mut s = self;
            s.protected_branches.push(branch.to_owned());
            s
        }

        fn with_head_ref(self, head_ref: &str) -> Self {
            Self {
                head_ref: Some(HeadRef::new(head_ref).unwrap()),
                ..self
            }
        }

        fn with_detached_head(self) -> Self {
            Self {
                head_ref: Some(HeadRef::detached()),
                ..self
            }
        }

        fn with_upstream_ref(
            self,
            upstream_ref: &str,
            is_synchronized: bool,
            is_based_on_remote: bool,
            is_head: bool,
        ) -> Self {
            Self {
                upstream: Some(Some((
                    RemoteRef::new(upstream_ref).unwrap(),
                    is_synchronized,
                    is_based_on_remote,
                    is_head,
                ))),
                ..self
            }
        }

        fn with_no_upstream(self) -> Self {
            Self {
                upstream: Some(None),
                ..self
            }
        }

        fn with_status(self, status: Status) -> Self {
            Self {
                status: Some(status),
                ..self
            }
        }
    }

    impl Collector for MockState {
        type Error = &'static str;

        fn default_branch(&self) -> Result<Option<String>, Self::Error> {
            if let Some(o) = &self.default_branch {
                Ok(o.clone())
            } else {
                Err("default_branch unset")
            }
        }

        fn is_remote_head(&self) -> Result<bool, Self::Error> {
            if let Some(Some(branch)) = self.head_ref.as_ref().map(|o| o.branch()) {
                if let Some(upstream) = &self.upstream {
                    if let Some((o, _, _, is_head)) = upstream {
                        if branch == o.branch() && *is_head {
                            return Ok(true);
                        }
                    }
                }
            }

            Ok(false)
        }

        fn is_head_protected(&self) -> Result<bool, Self::Error> {
            if let Some(o) = &self.head_ref {
                if let Some(br) = o.branch() {
                    return Ok(self.protected_branches.iter().any(|pb| br == pb));
                }
            }

            Ok(false)
        }

        fn head_ref(&self) -> Result<HeadRef, Self::Error> {
            if let Some(o) = &self.head_ref {
                Ok(o.clone())
            } else {
                Err("head_ref unset")
            }
        }

        fn upstream_ref(&self) -> Result<Option<RemoteRef>, Self::Error> {
            if let Some(upstream) = &self.upstream {
                if let Some((o, _, _, _)) = upstream {
                    Ok(Some(o.clone()))
                } else {
                    Ok(None)
                }
            } else {
                Err("upstream_ref unset")
            }
        }

        fn is_synchronized(&self) -> Result<bool, Self::Error> {
            if let Some(Some((_, o, _, _))) = &self.upstream {
                Ok(*o)
            } else {
                Ok(false)
            }
        }

        fn is_based_on_remote(&self) -> Result<bool, Self::Error> {
            if let Some(Some((_, _, o, _))) = &self.upstream {
                Ok(*o)
            } else {
                Ok(false)
            }
        }

        fn status(&self) -> Result<Status, Self::Error> {
            if let Some(o) = self.status {
                Ok(o)
            } else {
                Err("status unset")
            }
        }
    }

    #[test]
    fn test_action_from() {
        let cases = [
            // index or wt has conflict -> should resolve conflict
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_upstream_ref("refs/remotes/origin/foo", true, true, false)
                    .with_status(Status::CONFLICTED),
                Action::ResolveConflict,
            ),
            // on default branch and synchronized -> nothing to do.
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/main")
                    .with_upstream_ref("refs/remotes/origin/main", true, true, false)
                    .with_status(Status::CURRENT),
                Action::None,
            ),
            // on default branch with local changes -> should rename the branch
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/main")
                    .with_upstream_ref("refs/remotes/origin/main", false, true, false)
                    .with_status(Status::CURRENT),
                Action::RenameBranch,
            ),
            // on protected branch and synchronized -> nothing to do.
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/develop")
                    .with_upstream_ref("refs/remotes/origin/develop", true, true, false)
                    .with_protected_branch("develop")
                    .with_status(Status::CURRENT),
                Action::None,
            ),
            // on a branch with remote's default branch -> should rename the branch
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/develop")
                    .with_upstream_ref("refs/remotes/origin/develop", false, true, true)
                    .with_protected_branch("main")
                    .with_status(Status::CURRENT),
                Action::RenameBranch,
            ),
            // on default branch with local changes -> should rename the branch
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/develop")
                    .with_upstream_ref("refs/remotes/origin/develop", false, true, false)
                    .with_protected_branch("develop")
                    .with_status(Status::CURRENT),
                Action::RenameBranch,
            ),
            // on detached head -> should create branch
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_detached_head()
                    .with_no_upstream()
                    .with_status(Status::CURRENT),
                Action::CreateBranch,
            ),
            // on topic branch and no remote tracking branch -> push
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_no_upstream()
                    .with_status(Status::CURRENT),
                Action::Push {
                    head_ref: HeadRef::new("refs/heads/foo").unwrap(),
                    upstream_ref: None,
                },
            ),
            // on topic branch and include remote commits -> push
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_upstream_ref("refs/remotes/origin/foo", false, true, false)
                    .with_status(Status::CURRENT),
                Action::Push {
                    head_ref: HeadRef::new("refs/heads/foo").unwrap(),
                    upstream_ref: Some(RemoteRef::new("refs/remotes/origin/foo").unwrap()),
                },
            ),
            // on topic branch, but it doesn't include remote commits -> rebase
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_upstream_ref("refs/remotes/origin/foo", false, false, false)
                    .with_status(Status::CURRENT),
                Action::Rebase {
                    head_ref: HeadRef::new("refs/heads/foo").unwrap(),
                    upstream_ref: RemoteRef::new("refs/remotes/origin/foo").unwrap(),
                },
            ),
            // on topic branch and dirty -> stage changes
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_upstream_ref("refs/remotes/origin/foo", true, true, false)
                    .with_status(Status::WT_MODIFIED),
                Action::StageChanges,
            ),
            // on topic branch and staged -> commit
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_upstream_ref("refs/remotes/origin/foo", true, true, false)
                    .with_status(Status::INDEX_NEW),
                Action::Commit,
            ),
            // on topic branch and synchronized -> nothing to do
            (
                MockState::default()
                    .with_default_branch("main")
                    .with_head_ref("refs/heads/foo")
                    .with_upstream_ref("refs/remotes/origin/foo", true, true, false)
                    .with_status(Status::CURRENT),
                Action::None,
            ),
        ];

        for (i, (given, expected)) in cases.into_iter().enumerate() {
            match Action::new(&given) {
                Ok(s) => assert_eq!(
                    expected, s,
                    "#{i}: from {given:?}, expected {expected:?} but got {s:?}"
                ),
                e => unreachable!("#{}: from {:?}, expected Ok but got {:?}", i, given, e),
            }
        }
    }
}
