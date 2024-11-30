use clap::Parser;
use git2::Repository;
use git_toolbox::app::dah::Application;

#[derive(Parser)]
#[command(
    about = "Push local changes anyway -- I know what you mean",
    long_about = None)]
struct Cli {
    #[arg(long, short = '1', help = "Do stepwise execution")]
    step: bool,
    // maybe implement --ask option?
    // #[arg(long, help = "Persistently ask before doing anything just in case")]
    // ask: bool,
    #[arg(
        long,
        help = "Increase number of commits to scan in history",
        default_value = "100"
    )]
    limit: usize,
}

impl Cli {
    fn into_app(self) -> Result<Application, Box<dyn std::error::Error>> {
        let repo = Repository::open_from_env()?;

        Ok(Application {
            repo,
            step: self.step,
            limit: self.limit,
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Cli::parse().into_app().and_then(|cmd| cmd.run())
}
