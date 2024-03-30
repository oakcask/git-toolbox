use std::{env::{self}, error::Error, path::{Component, Path}, process::exit};
use clap::{Parser};
use git2::{Repository};
use git_toolbox::github::codeowners::CodeOwners;
use log::error;

#[derive(Parser)]
#[command(
    about = "find GitHub CODEOWNERS for path(s)",
    long_about = None)]
struct Cli {
    #[arg()]
    paths: Vec<String>
}

#[derive(thiserror::Error, Debug)]
enum CliError {
    #[error("{0}")]
    PathError(&'static str)
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
            let mut components = Path::new(&path).components();
            let abs_path = match components.next() {
                Some(Component::CurDir)
                    | Some(Component::ParentDir)
                    | Some(Component::RootDir)
                    | Some(Component::Normal(_)) => {
                    match env::current_dir()?.join(path).strip_prefix(repo_root)?.as_os_str().to_str() {
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
            }?;
            workdir_paths.push(abs_path)
        }
        Ok(workdir_paths)
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
