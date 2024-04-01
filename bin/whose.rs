use std::{env, error::Error, path::{Component, Path}, process::exit};
use clap::Parser;
use git2::Repository;
use git_toolbox::{github::codeowners::CodeOwners, pathname};
use log::error;

#[derive(Parser)]
#[command(
    about = "find GitHub CODEOWNERS for path(s)",
    long_about = None)]
struct Cli {
    #[arg()]
    paths: Vec<String>
}

#[derive(thiserror::Error, Debug, PartialEq)]
enum CliError {
    #[error("{0}")]
    PathError(&'static str),
    #[error("path points to the out side of repository")]
    OutSideOfRepo
}

struct Command {
    _repo: Repository,
    codeowners: CodeOwners,
    paths: Vec<String>,
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        for path in self.paths.iter() {
            println!("{}: {:?}", path, self.codeowners.find_owners(path));
        }

        Ok(())
    }
}

impl Cli {
    fn to_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let paths = if repo.is_bare() {
            self.paths
        } else {
            Self::normalize_paths(&repo, self.paths)?
        };

        let codeowners = CodeOwners::new(&repo)?;

        Ok(Command {
            _repo: repo,
            codeowners,
            paths
        })
    }

    fn normalize_paths(repo: &Repository, paths: Vec<String>) -> Result<Vec<String>, Box<dyn Error>> {
        let repo_root = repo.path().parent().unwrap();
        let mut workdir_paths = Vec::new();
        for path in paths {
            let path = Path::new(&path);
            let abs_path = Self::normalize_path(&env::current_dir()?, repo_root, path)?;
            workdir_paths.push(abs_path)
        }
        Ok(workdir_paths)
    }

    fn normalize_path(cwd: &Path, repo_root: &Path, path: &Path) -> Result<String, CliError> {
        let mut components = path.components();
        match components.next() {
            Some(Component::CurDir)
                | Some(Component::ParentDir)
                | Some(Component::RootDir)
                | Some(Component::Normal(_)) => {
                let abs = pathname::canonicalize(cwd.join(path));
                let normalized = abs.strip_prefix(repo_root)
                    .map_err(|_| CliError::OutSideOfRepo)?;

                match normalized.as_os_str().to_str() {
                    Some(s) => Ok(s.to_owned()),
                    None => Err(CliError::PathError("cannot convert path to UTF-8 string"))
                }
            }
            Some(Component::Prefix(_)) => {
                Err(CliError::PathError("cannot handle path with prefix"))
            }
            None => {
                Err(CliError::PathError("cannot handle empty path"))
            }
        }
    }
}

fn main() -> ! {
    env_logger::init();
    match Cli::parse().to_command().and_then(|cmd| cmd.run()) {
        Err(e) => {
            error!("{}", e.to_string());
            exit(1)
        }
        Ok(_) => {
            exit(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, fs::{self}, path::Path};

    use tempfile::TempDir;

    use crate::Cli;

    #[test]
    fn test_normalize_path() -> Result<(), Box<dyn Error>> {
        let tmpdir = TempDir::new()?;
        let repo_root = tmpdir.path();
        fs::create_dir(tmpdir.path().join("foo"))?;
        fs::create_dir(tmpdir.path().join("foo").join("bar"))?;

        let cases = [
            (tmpdir.path().join("foo"), Path::new("bar").to_path_buf(), "foo/bar"),
            (tmpdir.path().join("foo"), Path::new("../a").to_path_buf(), "a"),
            (tmpdir.path().join("foo"), Path::new("./b").to_path_buf(), "foo/b"),
            (tmpdir.path().join("foo"), tmpdir.path().join("foo").join("bar"), "foo/bar"),
        ];

        for (idx, (cwd, path, normalized_path)) in cases.into_iter().enumerate() {
            let got = Cli::normalize_path(cwd.as_path(), repo_root, path.as_path());
            assert_eq!(
                got,
                Ok(normalized_path.to_owned()),
                "#{}: wanted Ok({:?}) for repo={:?}, cwd={:?} and path={:?}, but got {:?}",
                idx, normalized_path, repo_root, cwd, path, got
            );
        }

        Ok(())
    }
}