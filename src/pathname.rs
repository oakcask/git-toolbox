use std::{
    env,
    path::{Component, Path, PathBuf, MAIN_SEPARATOR_STR},
};

use git2::Repository;

#[derive(PartialEq, Debug, thiserror::Error)]
pub enum NormalizePathError {
    #[error("path {0} points to the out side of repository")]
    OutSideOfRepo(PathBuf),
    #[error("{0}")]
    RuntimeError(&'static str),
    #[error("{0}")]
    IOError(String),
}

/// Canonicalize path, but without symlink resolve.
fn canonicalize(path: PathBuf) -> PathBuf {
    let mut buf = PathBuf::with_capacity(path.capacity());

    for part in path.components() {
        match part {
            Component::Prefix(pfx) => {
                buf.push(pfx.as_os_str());
            }
            Component::RootDir => buf.push(MAIN_SEPARATOR_STR),
            Component::CurDir => {
                // do nothing
            }
            Component::ParentDir => {
                buf.pop();
            }
            Component::Normal(p) => {
                buf.push(p);
            }
        }
    }

    buf
}

pub fn normalize_paths(
    repo: &Repository,
    paths: Vec<String>,
) -> Result<Vec<String>, NormalizePathError> {
    let repo_root = repo.path().parent().unwrap();
    let mut workdir_paths = Vec::new();
    for path in paths {
        let path = Path::new(&path);
        let abs_path = normalize_path(
            &env::current_dir().map_err(|e| NormalizePathError::IOError(e.to_string()))?,
            repo_root,
            path,
        )?;
        workdir_paths.push(abs_path)
    }
    Ok(workdir_paths)
}

fn normalize_path(cwd: &Path, repo_root: &Path, path: &Path) -> Result<String, NormalizePathError> {
    let mut components = path.components();
    match components.next() {
        Some(Component::CurDir)
        | Some(Component::ParentDir)
        | Some(Component::RootDir)
        | Some(Component::Normal(_)) => {
            let abs = canonicalize(cwd.join(path));
            let normalized = abs
                .strip_prefix(repo_root)
                .map_err(|_| NormalizePathError::OutSideOfRepo(path.to_owned()))?;

            match normalized.as_os_str().to_str() {
                Some(s) => Ok(s.to_owned()),
                None => Err(NormalizePathError::RuntimeError(
                    "cannot convert path to UTF-8 string",
                )),
            }
        }
        Some(Component::Prefix(_)) => Err(NormalizePathError::RuntimeError(
            "cannot handle path with prefix",
        )),
        None => Err(NormalizePathError::RuntimeError("cannot handle empty path")),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        str::FromStr,
    };

    use tempfile::TempDir;

    use crate::pathname::normalize_path;

    #[test]
    #[cfg(unix)]
    fn test_canonicalize() {
        let cases = [
            ("a", "a"),
            ("a/b", "a/b"),
            ("a/./b", "a/b"),
            ("a//b", "a/b"),
            ("/a/b", "/a/b"),
            ("/a/b/..", "/a"),
        ];

        for (idx, (path, want)) in cases.into_iter().enumerate() {
            let got = super::canonicalize(PathBuf::from_str(path).unwrap());
            assert_eq!(
                got.to_str(),
                Some(want),
                "#{} wants {} but got {:?}",
                idx,
                want,
                got
            );
        }
    }

    #[test]
    fn test_normalize_path() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo_root = tmpdir.path();
        fs::create_dir(tmpdir.path().join("foo"))?;
        fs::create_dir(tmpdir.path().join("foo").join("bar"))?;

        let cases = [
            (
                tmpdir.path().join("foo"),
                Path::new("bar").to_path_buf(),
                "foo/bar",
            ),
            (
                tmpdir.path().join("foo"),
                Path::new("../a").to_path_buf(),
                "a",
            ),
            (
                tmpdir.path().join("foo"),
                Path::new("./b").to_path_buf(),
                "foo/b",
            ),
            (
                tmpdir.path().join("foo"),
                tmpdir.path().join("foo").join("bar"),
                "foo/bar",
            ),
        ];

        for (idx, (cwd, path, normalized_path)) in cases.into_iter().enumerate() {
            let got = normalize_path(cwd.as_path(), repo_root, path.as_path());
            assert_eq!(
                got,
                Ok(normalized_path.to_owned()),
                "#{}: wanted Ok({:?}) for repo={:?}, cwd={:?} and path={:?}, but got {:?}",
                idx,
                normalized_path,
                repo_root,
                cwd,
                path,
                got
            );
        }

        Ok(())
    }
}
