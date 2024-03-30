use std::error::Error;

use clap::Parser;
use git2::Repository;
use git_toolbox::github::codeowners::CodeOwners;

#[derive(Parser)]
#[command(
    about = "find GitHub CODEOWNERS for path(s)",
    long_about = None)]
struct Cli {
    #[arg()]
    paths: Vec<String>
}

struct Command {
    repo: Repository,
    codeowners: CodeOwners,
    paths: Vec<String>
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        for path in self.paths.iter() {
            println!("{:?}", self.codeowners.find_owners(&path));
        }

        Ok(())
    }
}

impl Cli {
    fn to_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let codeowners = CodeOwners::new(&repo)?;


        Ok(Command {
            repo,
            codeowners,
            paths: self.paths
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    Cli::parse().to_command()?.run()
}
