use clap::Parser;
use git_toolbox::{
    app::stale::{Application, StaleOptions},
    reltime::Reltime,
};
use log::error;
use std::{error::Error, process::exit};

#[derive(Parser)]
#[command(
    about = "List or delete stale branches",
    long_about = None)]
struct Cli {
    #[arg(
        long,
        help = "Select origin remote-tracking branches instead of local branches"
    )]
    remote: bool,
    #[arg(short, long, help = "Perform deletion of selected branches")]
    delete: bool,
    #[arg(
        long,
        help = "Combined with --delete, perform deletion on remote repository instead"
    )]
    push: bool,
    #[arg(long,
        help = "Select local branch with commit times older than the specified relative time",
        value_parser = parse_reltime)]
    since: Option<Reltime>,
    #[arg(help = "Select branches with specified prefixes, or select all if unset")]
    branches: Vec<String>,
}

impl Cli {
    fn into_app(self) -> Result<Application, Box<dyn Error>> {
        Application::from_options(StaleOptions {
            remote: self.remote,
            delete: self.delete,
            push: self.push,
            since: self.since,
            branches: self.branches,
        })
    }
}

fn parse_reltime(arg: &str) -> Result<Reltime, String> {
    Reltime::try_from(arg).map_err(|e| format!("while parsing {arg} got error: {e}"))
}

fn main() -> ! {
    env_logger::init();
    match Cli::parse().into_app().and_then(|app| app.run()) {
        Err(e) => {
            error!("{e}");
            exit(1)
        }
        Ok(_) => exit(0),
    }
}
