use std::{error::Error, process::exit};
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

struct Command {
    _repo: Repository,
    codeowners: CodeOwners,
    paths: Vec<String>,
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        for path in self.paths.iter() {
            match self.codeowners.find_owners(path) {
                Some(owners) => {
                    println!("{}: {}", path, owners.join(", "));
                }
                None => {
                    println!("{}:", path);
                }
            }
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
            pathname::normalize_paths(&repo, self.paths)?
        };

        let codeowners = CodeOwners::new(&repo)?;

        Ok(Command {
            _repo: repo,
            codeowners,
            paths
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