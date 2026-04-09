use git2::{IndexEntry, IndexTime, Oid, Repository, Signature};
use log::Log;
use once_cell::sync::Lazy;
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};

/// do `git init <path>`
pub fn git_init<P: AsRef<Path>>(path: P) -> Repository {
    Repository::init(path).unwrap()
}

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

type LogRecord = (log::Level, String, String);

pub struct CapturedLog {
    buf: Vec<LogRecord>,
}

impl CapturedLog {
    fn log(&mut self, record: &log::Record) {
        let mesg = if let Some(s) = record.args().as_str() {
            s.to_string()
        } else {
            record.args().to_string()
        };

        let r = (record.level(), record.target().to_owned(), mesg);
        self.buf.push(r);
    }

    fn take(&mut self) -> Vec<LogRecord> {
        self.buf.drain(..).collect()
    }
}

#[derive(Clone)]
pub struct TestLogger {
    captures: Arc<Mutex<CapturedLog>>,
}

impl Default for TestLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl TestLogger {
    pub fn new() -> TestLogger {
        TestLogger {
            captures: Arc::new(Mutex::new(CapturedLog { buf: Vec::new() })),
        }
    }

    pub fn take(&self) -> Vec<LogRecord> {
        let mut cap = self.captures.lock().unwrap();
        cap.take()
    }
}

impl Log for TestLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut cap = self.captures.lock().unwrap();
        cap.log(record);
    }

    fn flush(&self) {
        // do nothing
    }
}

static LOGGER: Lazy<TestLogger> = Lazy::new(|| {
    let logger = TestLogger::new();
    log::set_boxed_logger(Box::new(logger.clone())).unwrap();
    log::set_max_level(log::LevelFilter::Debug);
    logger
});

pub fn test_logger() -> TestLogger {
    LOGGER.clone()
}
