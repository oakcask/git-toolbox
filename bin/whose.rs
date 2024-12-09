use clap::Parser;
use git2::Repository;
use git_toolbox::app::whose::{Application, ApplicationBuilder};

#[derive(Parser)]
#[command(
    about = "find GitHub CODEOWNERS for path(s)",
    long_about = None)]
struct Cli {
    #[arg()]
    pathspecs: Vec<String>,
}

impl Cli {
    fn into_app(self) -> Result<Application, Box<dyn std::error::Error>> {
        let repo = Repository::open_from_env()?;
        Ok(ApplicationBuilder::new(repo)
            .with_pathspecs(self.pathspecs)?
            .build()?)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = Cli::parse()
        .into_app()
        .and_then(|cmd| cmd.run().map_err(|e| e.into()))
    {
        eprintln!("{}", e);
        Err(e)
    } else {
        Ok(())
    }
}
