use git2::{Oid, Repository, Signature};
use std::path::Path;

fn tree_from_path_parts(repo: &Repository, parts: &[&str], blob: Oid) -> Result<Oid, git2::Error> {
    match parts.split_first() {
        None => unreachable!("empty git path"),
        Some((name, [])) => {
            let mut tb = repo.treebuilder(None)?;
            tb.insert(*name, blob, 0o100644)?;
            tb.write()
        }
        Some((dir, rest)) => {
            let inner = tree_from_path_parts(repo, rest, blob)?;
            let mut tb = repo.treebuilder(None)?;
            tb.insert(*dir, inner, 0o040000)?;
            tb.write()
        }
    }
}

/// Bare repository with a single committed file at `git_path` (e.g. `.github/CODEOWNERS`).
pub fn bare_repo_with_committed_file(
    root: impl AsRef<Path>,
    git_path: &str,
    content: &[u8],
) -> Repository {
    let repo = Repository::init_bare(root).unwrap();
    let blob_id = repo.blob(content).unwrap();
    let parts: Vec<&str> = git_path.split('/').filter(|p| !p.is_empty()).collect();
    let tree_id = tree_from_path_parts(&repo, &parts, blob_id).unwrap();
    let sig = Signature::now("t", "t@example.com").unwrap();
    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
    }
    repo
}
