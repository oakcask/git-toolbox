use git2::{IndexEntry, IndexTime, Oid, Repository, Signature};
use std::path::Path;

/// Bare repository with multiple committed files (paths use `/` as in Git).
pub fn bare_repo_with_committed_files(
    root: impl AsRef<Path>,
    files: &[(&str, &[u8])],
) -> Repository {
    let repo = Repository::init_bare(root).unwrap();
    let mut index = repo.index().unwrap();
    for (path, content) in files {
        let entry = IndexEntry {
            ctime: IndexTime::new(0, 0),
            mtime: IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode: 0o100644,
            uid: 0,
            gid: 0,
            file_size: 0,
            id: Oid::from_bytes(&[0; 20]).unwrap(),
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        };
        index.add_frombuffer(&entry, content).unwrap();
    }
    let tree_id = index.write_tree().unwrap();
    let sig = Signature::now("t", "t@example.com").unwrap();
    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
    }
    repo
}
