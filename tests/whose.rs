#[path = "common/bare_multi.rs"]
mod bare_multi;
#[path = "common/bin.rs"]
mod bin;
#[path = "common/git_worktree.rs"]
mod git_worktree;

use std::process::Command;

use bare_multi::bare_repo_with_committed_files;
use bin::git_whose_exe;
use git_worktree::{git_add, git_init, mkdir_p, write};
use tempfile::TempDir;

#[test]
fn git_whose_prints_owners_for_indexed_paths() {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    let co_path = root.join(".github/CODEOWNERS");
    mkdir_p(co_path.parent().unwrap());
    write(co_path, b"*.rs @rust-team\n");

    let src = root.join("src/lib.rs");
    mkdir_p(src.parent().unwrap());
    write(&src, b"fn f() {}\n");

    git_add(&repo, ".github/CODEOWNERS");
    git_add(&repo, "src/lib.rs");

    let exe = git_whose_exe();
    let out = Command::new(exe)
        .current_dir(root)
        .args(["src/lib.rs"])
        .output()
        .expect("spawn git-whose");

    assert!(
        out.status.success(),
        "git-whose failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("src/lib.rs") && stdout.contains("@rust-team"),
        "unexpected stdout: {stdout:?}"
    );
}

#[test]
fn git_whose_prints_owners_for_bare_repo_head_tree() {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();
    let git_dir = root.join("bare.git");

    bare_repo_with_committed_files(
        &git_dir,
        &[
            (".github/CODEOWNERS", b"*.rs @rust-team\n"),
            ("src/lib.rs", b"fn f() {}\n"),
        ],
    );

    let exe = git_whose_exe();
    let out = Command::new(exe)
        .env("GIT_DIR", git_dir.canonicalize().unwrap())
        .current_dir(root)
        .args(["src/lib.rs"])
        .output()
        .expect("spawn git-whose");

    assert!(
        out.status.success(),
        "git-whose failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("src/lib.rs") && stdout.contains("@rust-team"),
        "unexpected stdout: {stdout:?}"
    );
}
