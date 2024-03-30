use std::{error::Error, process::{exit}};
use clap::Parser;
use git2::Repository;
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

struct Command {
    _repo: Repository,
    codeowners: CodeOwners,
    paths: Vec<String>
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        for path in self.paths.iter() {
            println!("{:?}", self.codeowners.find_owners(path));
        }

        Ok(())
    }
}

impl Cli {
    fn to_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let codeowners = CodeOwners::new(&repo)?;
        
        Ok(Command {
            _repo: repo,
            codeowners,
            paths: self.paths
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
