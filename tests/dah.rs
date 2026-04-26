#[path = "common/bin.rs"]
mod bin;
#[path = "common/git_worktree.rs"]
mod git_worktree;

use git2::{
    build::{CloneLocal, RepoBuilder},
    BranchType, Repository, Status,
};
use regex::Regex;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use tempfile::TempDir;
use url::Url;

use bin::git_dah_exe;
use git_worktree::{
    git_add_remote, git_checkout_branch, git_command, git_commit_at, git_init_with_initial_commit,
    git_set_config, local_branch_exists, ref_exists,
};

struct DahFixture {
    _tmpdir: TempDir,
    worktree_root: PathBuf,
    origin_root: PathBuf,
    editor_path: PathBuf,
}

impl DahFixture {
    fn new() -> Self {
        let tmpdir = TempDir::new().unwrap();
        let seed_root = tmpdir.path().join("seed");
        let worktree_root = tmpdir.path().join("worktree");
        let origin_root = tmpdir.path().join("origin.git");

        let timestamp = git2::Time::new(1_700_000_000, 0);
        let seed_repo = git_init_with_initial_commit(&seed_root, "main", timestamp);
        git_set_config(&seed_repo, "user.name", "t");
        git_set_config(&seed_repo, "user.email", "t@example.com");
        git_set_config(&seed_repo, "init.defaultbranch", "main");
        git_commit_at(&seed_repo, "tracked.txt", b"base\n", "base", timestamp);
        git_commit_at(&seed_repo, "other.txt", b"base\n", "base", timestamp);

        let origin_repo = Repository::init_bare(&origin_root).unwrap();
        git_add_remote(&seed_repo, "origin", &file_url(&origin_root));
        push_branch(&seed_repo, "origin", "main");
        origin_repo.set_head("refs/heads/main").unwrap();

        RepoBuilder::new()
            .clone_local(CloneLocal::Auto)
            .clone(&file_url(&origin_root), &worktree_root)
            .unwrap();

        let worktree_repo = Repository::open(&worktree_root).unwrap();
        git_set_config(&worktree_repo, "user.name", "t");
        git_set_config(&worktree_repo, "user.email", "t@example.com");
        git_set_config(&worktree_repo, "init.defaultbranch", "main");

        let editor_path = tmpdir.path().join("git-editor.sh");
        fs::write(
            &editor_path,
            "#!/bin/sh\nprintf '%s\\n' \"$GIT_DAH_TEST_MESSAGE\" > \"$1\"\n",
        )
        .unwrap();
        let mut perms = fs::metadata(&editor_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&editor_path, perms).unwrap();

        Self {
            _tmpdir: tmpdir,
            worktree_root,
            origin_root,
            editor_path,
        }
    }

    fn worktree_repo(&self) -> Repository {
        Repository::open(&self.worktree_root).unwrap()
    }

    fn origin_repo(&self) -> Repository {
        Repository::open_bare(&self.origin_root).unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        self.run_with_message(args, "Ship it")
    }

    fn run_with_message(&self, args: &[&str], message: &str) -> Output {
        Command::new(git_dah_exe())
            .current_dir(&self.worktree_root)
            .env("GIT_EDITOR", &self.editor_path)
            .env("GIT_DAH_TEST_MESSAGE", message)
            .args(args)
            .output()
            .expect("spawn git-dah")
    }
}

fn file_url(path: &Path) -> String {
    Url::from_directory_path(path).unwrap().to_string()
}

fn push_branch(repo: &Repository, remote_name: &str, branch: &str) {
    let mut remote = repo.find_remote(remote_name).unwrap();
    let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
    remote.push(&[refspec.as_str()], None).unwrap();
}

fn current_branch(repo: &Repository) -> String {
    repo.head()
        .unwrap()
        .shorthand()
        .unwrap()
        .to_owned()
}

fn head_message(repo: &Repository) -> String {
    repo.head()
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .message()
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_owned()
}

fn head_blob_text(repo: &Repository, path: &str) -> String {
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let entry = head.tree().unwrap().get_path(Path::new(path)).unwrap();
    let blob = entry.to_object(repo).unwrap().peel_to_blob().unwrap();
    String::from_utf8(blob.content().to_vec()).unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "git-dah failed: stdout={}, stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_branch_matches(branch: &str, pattern: &str) {
    let pattern = Regex::new(pattern).unwrap();
    assert!(
        pattern.is_match(branch),
        "expected branch {branch:?} to match {pattern:?}"
    );
}

fn ahead_behind(repo: &Repository, local_ref: &str, upstream_ref: &str) -> (usize, usize) {
    let local_oid = repo.revparse_single(local_ref).unwrap().id();
    let upstream_oid = repo.revparse_single(upstream_ref).unwrap().id();
    repo.graph_ahead_behind(local_oid, upstream_oid).unwrap()
}

#[test]
fn git_dah_step_stages_only_one_action() {
    let fixture = DahFixture::new();
    fs::write(fixture.worktree_root.join("tracked.txt"), "step\n").unwrap();

    let output = fixture.run(&["--step", "--no-fetch"]);

    assert_success(&output);

    let repo = fixture.worktree_repo();
    assert_eq!("main", current_branch(&repo));
    assert_eq!("base", head_message(&repo));

    let status = repo.status_file(Path::new("tracked.txt")).unwrap();
    assert!(status.contains(Status::INDEX_MODIFIED));
    assert!(!status.contains(Status::WT_MODIFIED));
}

#[test]
fn git_dah_renames_default_branch_commits_and_pushes() {
    let fixture = DahFixture::new();
    fs::write(fixture.worktree_root.join("tracked.txt"), "release\n").unwrap();

    let output = fixture.run(&["--no-fetch"]);

    assert_success(&output);

    let repo = fixture.worktree_repo();
    let branch = current_branch(&repo);
    assert_branch_matches(&branch, r"\Aship-it-dah[0-9a-z]{26}\z");
    assert!(!local_branch_exists(&repo, "main"));
    assert_eq!("Ship it", head_message(&repo));
    assert!(repo.find_branch(&branch, BranchType::Local).unwrap().upstream().is_ok());

    let origin = fixture.origin_repo();
    assert!(ref_exists(&origin, &format!("refs/heads/{branch}")));
    assert!(ref_exists(&origin, "refs/heads/main"));
}

#[test]
fn git_dah_detached_head_creates_branch_and_pushes() {
    let fixture = DahFixture::new();
    git_command(&fixture.worktree_repo(), &["switch", "--detach", "HEAD"]);

    let output = fixture.run(&["--no-fetch"]);

    assert_success(&output);

    let repo = fixture.worktree_repo();
    let branch = current_branch(&repo);
    assert_branch_matches(&branch, r"\Abase-dah[0-9a-z]{26}\z");
    assert!(ref_exists(
        &fixture.origin_repo(),
        &format!("refs/heads/{branch}")
    ));
}

#[test]
fn git_dah_only_staged_leaves_unstaged_changes_in_worktree() {
    let fixture = DahFixture::new();
    let repo = fixture.worktree_repo();

    fs::write(fixture.worktree_root.join("tracked.txt"), "staged\n").unwrap();
    fs::write(fixture.worktree_root.join("other.txt"), "unstaged\n").unwrap();
    git_command(&repo, &["add", "tracked.txt"]);

    let output = fixture.run(&["--only-staged", "--no-fetch"]);

    assert_success(&output);

    let repo = fixture.worktree_repo();
    let branch = current_branch(&repo);
    assert_branch_matches(&branch, r"\Aship-it-dah[0-9a-z]{26}\z");

    let status = repo.status_file(Path::new("other.txt")).unwrap();
    assert_eq!(Status::WT_MODIFIED, status);
    assert_eq!("staged\n", head_blob_text(&repo, "tracked.txt"));
    assert_eq!("base\n", head_blob_text(&repo, "other.txt"));
}

#[test]
fn git_dah_cooperative_rebases_before_push() {
    let fixture = DahFixture::new();
    let repo = fixture.worktree_repo();

    git_checkout_branch(&repo, "feature/topic");
    git_command(&repo, &["push", "-u", "origin", "feature/topic"]);

    {
        let tmpdir = TempDir::new().unwrap();
        RepoBuilder::new()
            .clone_local(CloneLocal::Auto)
            .clone(&file_url(&fixture.origin_root), tmpdir.path())
            .unwrap();
        let remote_repo = Repository::open(tmpdir.path()).unwrap();
        git_set_config(&remote_repo, "user.name", "t");
        git_set_config(&remote_repo, "user.email", "t@example.com");
        git_checkout_branch(&remote_repo, "feature/topic");
        git_commit_at(
            &remote_repo,
            "tracked.txt",
            b"remote\n",
            "remote change",
            git2::Time::new(1_700_000_100, 0),
        );
        git_command(&remote_repo, &["push", "origin", "feature/topic"]);
    }

    git_command(&repo, &["fetch", "origin"]);
    git_commit_at(
        &repo,
        "other.txt",
        b"local\n",
        "local change",
        git2::Time::new(1_700_000_200, 0),
    );

    assert_eq!(
        (1, 1),
        ahead_behind(&repo, "HEAD", "refs/remotes/origin/feature/topic")
    );

    let output = fixture.run(&["--cooperative", "--step", "--no-fetch"]);

    assert_success(&output);

    let repo = fixture.worktree_repo();
    assert_eq!(
        (1, 0),
        ahead_behind(&repo, "HEAD", "refs/remotes/origin/feature/topic")
    );
    assert_eq!("local change", head_message(&repo));

    let output = fixture.run(&["--cooperative", "--no-fetch"]);
    assert_success(&output);

    let repo = fixture.worktree_repo();
    let origin = fixture.origin_repo();
    assert_eq!(
        repo.head().unwrap().target().unwrap(),
        origin
            .find_reference("refs/heads/feature/topic")
            .unwrap()
            .target()
            .unwrap()
    );
}
