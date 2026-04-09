use git2::Repository;
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

/// do `git init <path>`
pub fn git_init<P: AsRef<Path>>(path: P) -> Repository {
    Repository::init(path).unwrap()
}

/// do `git add <path>`
pub fn git_add<P: AsRef<Path>>(repo: &Repository, path: P) {
    let mut index = repo.index().unwrap();
    index.add_path(path.as_ref()).unwrap();
    index.write().unwrap();
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
