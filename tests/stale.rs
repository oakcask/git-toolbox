#[path = "common/bin.rs"]
mod bin;
#[path = "common/git_worktree.rs"]
mod git_worktree;

use git2::{BranchType, Repository, Time};
use std::process::{Command, Output};
use tempfile::TempDir;
use url::Url;

use bin::git_stale_exe;
use git_worktree::{
    git_add_remote, git_checkout_branch, git_commit_at, git_init_with_initial_commit,
    git_set_config, local_branch_exists, ref_exists,
};

struct StaleFixture {
    _tmpdir: TempDir,
    worktree_root: std::path::PathBuf,
    origin_root: std::path::PathBuf,
}

impl StaleFixture {
    fn new() -> Self {
        let tmpdir = TempDir::new().unwrap();
        let worktree_root = tmpdir.path().join("worktree");
        let origin_root = tmpdir.path().join("origin.git");
        let upstream_root = tmpdir.path().join("upstream.git");

        let now = chrono::Local::now();
        let old = Time::new((now - chrono::Duration::days(150)).timestamp(), 0);
        let new = Time::new((now - chrono::Duration::days(10)).timestamp(), 0);

        let repo = git_init_with_initial_commit(&worktree_root, "main", old);
        git_set_config(&repo, "dah.protectedbranch", "develop:release/*");
        git_set_config(&repo, "init.defaultbranch", "main");

        commit_branch(&repo, "develop", old);
        commit_branch(&repo, "release/v1", old);
        commit_branch(&repo, "feature/old", old);
        commit_branch(&repo, "feature/new", new);
        commit_branch(&repo, "topic/local-only", old);
        git_checkout_branch(&repo, "main");

        let _origin = Repository::init_bare(&origin_root).unwrap();
        let origin_url = file_url(&origin_root);
        git_add_remote(&repo, "origin", &origin_url);

        for branch in [
            "main",
            "develop",
            "release/v1",
            "feature/old",
            "feature/new",
        ] {
            push_branch(&repo, "origin", branch);
        }
        fetch_remote_tracking_refs(&repo, "origin");
        for branch in [
            "main",
            "develop",
            "release/v1",
            "feature/old",
            "feature/new",
        ] {
            repo.find_branch(branch, BranchType::Local)
                .unwrap()
                .set_upstream(Some(&format!("origin/{branch}")))
                .unwrap();
        }
        set_origin_head(&repo, "main");

        let _upstream = Repository::init_bare(&upstream_root).unwrap();
        let upstream_url = file_url(&upstream_root);
        git_add_remote(&repo, "upstream", &upstream_url);
        let feature_old_target = branch_target(&repo, "feature/old");
        repo.reference(
            "refs/remotes/upstream/feature/other",
            feature_old_target,
            true,
            "create non-origin remote-tracking ref",
        )
        .unwrap();

        Self {
            _tmpdir: tmpdir,
            worktree_root,
            origin_root,
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(git_stale_exe())
            .current_dir(&self.worktree_root)
            .args(args)
            .output()
            .expect("spawn git-stale")
    }

    fn worktree_repo(&self) -> Repository {
        Repository::open(&self.worktree_root).unwrap()
    }

    fn origin_repo(&self) -> Repository {
        Repository::open_bare(&self.origin_root).unwrap()
    }
}

fn commit_branch(repo: &Repository, branch: &str, timestamp: Time) {
    git_checkout_branch(repo, "main");
    git_checkout_branch(repo, branch);
    let path = format!("branches/{}.txt", branch.replace('/', "__"));
    let content = format!("{branch}\n");
    git_commit_at(repo, &path, content.as_bytes(), branch, timestamp);
}

fn branch_target(repo: &Repository, branch: &str) -> git2::Oid {
    repo.find_branch(branch, BranchType::Local)
        .unwrap()
        .get()
        .target()
        .unwrap()
}

fn file_url(path: &std::path::Path) -> String {
    Url::from_directory_path(path).unwrap().to_string()
}

fn push_branch(repo: &Repository, remote_name: &str, branch: &str) {
    let mut remote = repo.find_remote(remote_name).unwrap();
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[refspec.as_str()], None).unwrap();
}

fn fetch_remote_tracking_refs(repo: &Repository, remote_name: &str) {
    let mut remote = repo.find_remote(remote_name).unwrap();
    remote
        .fetch(&["refs/heads/*:refs/remotes/origin/*"], None, None)
        .unwrap();
}

fn set_origin_head(repo: &Repository, branch: &str) {
    repo.reference_symbolic(
        "refs/remotes/origin/HEAD",
        &format!("refs/remotes/origin/{branch}"),
        true,
        "set origin head",
    )
    .unwrap();
}

fn sorted_stdout_lines(output: &Output) -> Vec<String> {
    let mut lines = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>();
    lines.sort();
    lines
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn sorted_ref_names(repo: &Repository, prefix: &str) -> Vec<String> {
    let mut refs = repo
        .references()
        .unwrap()
        .filter_map(|reference| {
            let reference = reference.unwrap();
            let name = reference.name()?.to_owned();
            name.starts_with(prefix).then_some(name)
        })
        .collect::<Vec<_>>();
    refs.sort();
    refs
}

#[test]
fn git_stale_without_since_lists_only_local_branches_without_upstream() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&[]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert_eq!(
        vec!["refs/heads/topic/local-only".to_owned()],
        sorted_stdout_lines(&output)
    );
}

#[test]
fn git_stale_since_lists_only_stale_unprotected_local_branches() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--since", "3mo"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert_eq!(
        vec![
            "refs/heads/feature/old".to_owned(),
            "refs/heads/topic/local-only".to_owned()
        ],
        sorted_stdout_lines(&output)
    );
}

#[test]
fn git_stale_since_prefix_filters_local_branch_names() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--since", "3mo", "feature/"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert_eq!(
        vec!["refs/heads/feature/old".to_owned()],
        sorted_stdout_lines(&output)
    );
}

#[test]
fn git_stale_remote_requires_since() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--remote"]);

    assert!(!output.status.success(), "git-stale unexpectedly succeeded");
    assert!(stderr_text(&output).contains("--remote requires --since"));
}

#[test]
fn git_stale_remote_delete_requires_push() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--remote", "--since", "3mo", "--delete"]);

    assert!(!output.status.success(), "git-stale unexpectedly succeeded");
    assert!(stderr_text(&output).contains("--remote --delete requires --push"));
}

#[test]
fn git_stale_remote_requires_origin() {
    let tmpdir = TempDir::new().unwrap();
    let worktree_root = tmpdir.path().join("worktree");
    let old = Time::new(
        (chrono::Local::now() - chrono::Duration::days(150)).timestamp(),
        0,
    );
    let _repo = git_init_with_initial_commit(&worktree_root, "main", old);

    let output = Command::new(git_stale_exe())
        .current_dir(&worktree_root)
        .args(["--remote", "--since", "3mo"])
        .output()
        .expect("spawn git-stale");

    assert!(!output.status.success(), "git-stale unexpectedly succeeded");
    assert!(stderr_text(&output).contains("origin remote does not exist"));
}

#[test]
fn git_stale_delete_removes_selected_local_branches_only() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--since", "3mo", "--delete"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert!(sorted_stdout_lines(&output).is_empty());

    let repo = fixture.worktree_repo();
    assert!(local_branch_exists(&repo, "main"));
    assert!(local_branch_exists(&repo, "develop"));
    assert!(local_branch_exists(&repo, "release/v1"));
    assert!(local_branch_exists(&repo, "feature/new"));
    assert!(!local_branch_exists(&repo, "feature/old"));
    assert!(!local_branch_exists(&repo, "topic/local-only"));
}

#[test]
fn git_stale_delete_push_removes_selected_upstream_branches_only() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--since", "3mo", "--delete", "--push", "feature/"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert!(sorted_stdout_lines(&output).is_empty());

    let worktree_repo = fixture.worktree_repo();
    assert!(local_branch_exists(&worktree_repo, "feature/old"));
    assert!(local_branch_exists(&worktree_repo, "topic/local-only"));

    let origin_repo = fixture.origin_repo();
    assert!(ref_exists(&origin_repo, "refs/heads/main"));
    assert!(ref_exists(&origin_repo, "refs/heads/develop"));
    assert!(ref_exists(&origin_repo, "refs/heads/release/v1"));
    assert!(ref_exists(&origin_repo, "refs/heads/feature/new"));
    assert!(!ref_exists(&origin_repo, "refs/heads/feature/old"));
}

#[test]
fn git_stale_remote_since_lists_matching_origin_tracking_refs() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--remote", "--since", "3mo"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert_eq!(
        vec!["origin/feature/old".to_owned()],
        sorted_stdout_lines(&output)
    );
}

#[test]
fn git_stale_remote_prefix_filters_use_short_branch_names() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--remote", "--since", "3mo", "feature/"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert_eq!(
        vec!["origin/feature/old".to_owned()],
        sorted_stdout_lines(&output)
    );

    let output = fixture.run(&["--remote", "--since", "3mo", "origin/feature/"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert!(sorted_stdout_lines(&output).is_empty());
}

#[test]
fn git_stale_remote_delete_push_removes_selected_origin_branches() {
    let fixture = StaleFixture::new();

    let output = fixture.run(&["--remote", "--since", "3mo", "--delete", "--push"]);

    assert!(
        output.status.success(),
        "git-stale failed: stderr={}",
        stderr_text(&output)
    );
    assert!(sorted_stdout_lines(&output).is_empty());

    let origin_repo = fixture.origin_repo();
    let origin_refs = sorted_ref_names(&origin_repo, "refs/heads/");
    assert!(ref_exists(&origin_repo, "refs/heads/main"));
    assert!(ref_exists(&origin_repo, "refs/heads/develop"));
    assert!(ref_exists(&origin_repo, "refs/heads/release/v1"));
    assert!(ref_exists(&origin_repo, "refs/heads/feature/new"));
    assert!(
        !ref_exists(&origin_repo, "refs/heads/feature/old"),
        "remote delete did not remove origin/feature/old; stderr={}; refs={origin_refs:?}",
        stderr_text(&output)
    );
}
