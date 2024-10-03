use std::{error::Error, ffi::OsStr, os::unix::ffi::OsStrExt as _, process::exit};
use clap::Parser;
use git2::{Pathspec, PathspecFlags, Repository};
use git_toolbox::{github::codeowners::CodeOwners, pathname};
use log::error;

#[derive(Parser)]
#[command(
    about = "find GitHub CODEOWNERS for path(s)",
    long_about = None)]
struct Cli {
    #[arg()]
    pathspecs: Vec<String>
}

struct Command {
    repo: Repository,
    codeowners: CodeOwners,
    pathspecs: Vec<String>,
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        let index = self.repo.index()?;
        let pathspec = Pathspec::new(self.pathspecs.iter())?;
        let matches = pathspec.match_index(&index, PathspecFlags::default())?;

        for entry in matches.entries() {
            let path = OsStr::from_bytes(entry);
            if let Some(path) = OsStr::from_bytes(entry).to_str() {
                match self.codeowners.find_owners(path) {
                    Some(owners) => {
                        println!("{}: {}", path, owners.join(", "));
                    }
                    None => {
                        println!("{}:", path);
                    }
                }
            } else {
                log::error!("cannot convet {:?} into utf-8 string.", path)
            }

        }

        Ok(())
    }
}

impl Cli {
    fn to_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let pathspecs = if repo.is_bare() {
            self.pathspecs
        } else {
            pathname::normalize_paths(&repo, self.pathspecs)?
        };

        let codeowners = CodeOwners::try_from_repo(&repo)?;

        Ok(Command {
            repo,
            codeowners,
            pathspecs
        })
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