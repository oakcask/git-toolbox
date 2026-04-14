#![allow(dead_code)]

use git2::{
    BranchType, ConfigLevel, ObjectType, Repository, RepositoryInitOptions, Signature, Time,
};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

/// do `git init <path>`
pub fn git_init<P: AsRef<Path>>(path: P) -> Repository {
    Repository::init(path).unwrap()
}

/// Initialize a worktree repository with `head_name` checked out and an initial commit.
pub fn git_init_with_initial_commit<P: AsRef<Path>>(
    path: P,
    head_name: &str,
    timestamp: Time,
) -> Repository {
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head(head_name);
    let repo = Repository::init_opts(path, &opts).unwrap();
    git_commit_at(
        &repo,
        ".git-toolbox-init",
        b"initial commit\n",
        "initial commit",
        timestamp,
    );
    repo
}

/// do `git add <path>`
pub fn git_add<P: AsRef<Path>>(repo: &Repository, path: P) {
    let mut index = repo.index().unwrap();
    index.add_path(path.as_ref()).unwrap();
    index.write().unwrap();
}

/// Create a commit that updates `path` with explicit author and committer timestamps.
pub fn git_commit_at<P: AsRef<Path>>(
    repo: &Repository,
    path: P,
    buf: &[u8],
    message: &str,
    timestamp: Time,
) {
    let workdir = repo.workdir().unwrap();
    let path = path.as_ref();
    let full_path = workdir.join(path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&full_path, buf).unwrap();

    let mut index = repo.index().unwrap();
    index.add_path(path).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::new("t", "t@example.com", &timestamp).unwrap();
    let parents = repo
        .head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| vec![repo.find_commit(oid).unwrap()])
        .unwrap_or_default();
    let parent_refs: Vec<_> = parents.iter().collect();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parent_refs,
    )
    .unwrap();
}

/// Create `branch_name` from the current `HEAD` if needed, then check it out.
pub fn git_checkout_branch(repo: &Repository, branch_name: &str) {
    if repo.find_branch(branch_name, BranchType::Local).is_err() {
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch(branch_name, &head_commit, false).unwrap();
    }

    repo.set_head(&format!("refs/heads/{branch_name}")).unwrap();
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.checkout_head(Some(&mut checkout)).unwrap();
}

/// Set a local Git config key.
pub fn git_set_config(repo: &Repository, key: &str, value: &str) {
    repo.config()
        .unwrap()
        .open_level(ConfigLevel::Local)
        .unwrap()
        .set_str(key, value)
        .unwrap();
}

/// Add a named remote to the repository.
pub fn git_add_remote(repo: &Repository, name: &str, url: &str) {
    repo.remote(name, url).unwrap();
}

/// Return true when the local branch exists.
pub fn local_branch_exists(repo: &Repository, branch_name: &str) -> bool {
    repo.find_branch(branch_name, BranchType::Local).is_ok()
}

/// Return true when the ref exists.
pub fn ref_exists(repo: &Repository, ref_name: &str) -> bool {
    repo.revparse_single(ref_name)
        .map(|object| matches!(object.kind(), Some(ObjectType::Commit)))
        .unwrap_or(false)
        || repo.find_reference(ref_name).is_ok()
}

/// do `mkdir -p <path>`
pub fn mkdir_p<P: AsRef<Path>>(path: P) {
    fs::create_dir_all(path).unwrap();
}

/// write buffer data to path
pub fn write<P: AsRef<Path>>(path: P, buf: &[u8]) {
    let mut file = File::create_new(path).unwrap();
    file.write_all(buf).unwrap();
}
