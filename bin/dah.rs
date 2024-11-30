use std::{
    ffi::OsString,
    io::{self},
    process::Stdio,
};

use clap::Parser;
use git2::Repository;
use git_toolbox::{
    app::dah::{self, Action, GitCli, RepositoryCollector, StepResult},
    git::{HeadRef, RemoteRef},
};
use log::{error, info};
use regex::Regex;
use ulid::Ulid;

#[derive(thiserror::Error, Debug)]
enum DahError {
    #[error("{command:?} failed with exit code {code:?}")]
    ExitStatus {
        command: OsString,
        code: Option<i32>,
    },
    #[error("internal error: {0}")]
    IO(#[from] io::Error),
    #[error("internal error: {0}")]
    Git(#[from] git2::Error),
}

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

struct Command {
    repo: Repository,
    step: bool,
    limit: usize,
}

impl Cli {
    fn into_command(self) -> Result<Command, Box<dyn std::error::Error>> {
        let repo = Repository::open_from_env()?;

        Ok(Command {
            repo,
            step: self.step,
            limit: self.limit,
        })
    }
}

fn get_command_line(command: &std::process::Command) -> OsString {
    let mut cmd = command.get_program().to_owned();
    for arg in command.get_args() {
        cmd.push(" ");
        cmd.push(arg);
    }
    cmd
}

impl Command {
    fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        env_logger::init();

        loop {
            let collector = RepositoryCollector::new(&self.repo).with_walk_limit(self.limit);
            let action = Action::new(collector)?;
            match dah::step(action, &self)? {
                StepResult::Abort { reason } => {
                    error!("{}", reason);
                    break;
                }
                StepResult::Stop => break,
                StepResult::Continue => {
                    if self.step {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn generate_branch_name(&self) -> Result<String, DahError> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        let mesg = commit.message().and_then(|m| m.lines().next());
        let random = Ulid::new().to_string().to_ascii_lowercase();
        if let Some(mesg) = mesg {
            let mesg = Regex::new(r#"\s+"#).unwrap().replace_all(mesg, "-");
            let mesg = Regex::new(r#"[^-\w]"#).unwrap().replace_all(&mesg, "_");

            let mut mesg = mesg.into_owned();
            mesg.push_str("-dah");
            mesg.push_str(&random);
            Ok(mesg)
        } else {
            let mut mesg = String::from("dah");
            mesg.push_str(&random);
            Ok(mesg)
        }
    }

    fn run_command(&self, command: &mut std::process::Command) -> Result<(), DahError> {
        let cmdline = get_command_line(command);
        info!("invoking {:?}", cmdline);

        command
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        let status = command.status()?;

        if status.success() {
            Ok(())
        } else {
            Err(DahError::ExitStatus {
                command: cmdline,
                code: status.code(),
            })
        }
    }
}

impl GitCli for Command {
    type Error = DahError;

    fn status(&self) -> Result<(), Self::Error> {
        self.run_command(std::process::Command::new("git").arg("status"))
    }

    fn create_branch_and_switch(&self) -> Result<(), Self::Error> {
        let branch_name = self.generate_branch_name()?;
        self.run_command(
            std::process::Command::new("git")
                .arg("switch")
                .arg("-c")
                .arg(branch_name),
        )
    }

    fn rename_branch_and_switch(&self) -> Result<(), Self::Error> {
        let branch_name = self.generate_branch_name()?;
        self.run_command(
            std::process::Command::new("git")
                .arg("branch")
                .arg("-m")
                .arg(branch_name),
        )
    }

    fn stage_changes(&self) -> Result<(), Self::Error> {
        self.run_command(std::process::Command::new("git").arg("add").arg("-u"))
    }

    fn commit(&self) -> Result<(), Self::Error> {
        self.run_command(std::process::Command::new("git").arg("commit"))
    }

    fn pull_with_rebase(&self, upstream_ref: &str) -> Result<(), Self::Error> {
        // TODO: receive RemoteRef
        let upstream_ref = RemoteRef::new(upstream_ref).unwrap();
        self.run_command(
            std::process::Command::new("git")
                .arg("pull")
                .arg("--rebase")
                .arg(upstream_ref.remote())
                .arg(upstream_ref.branch()),
        )
    }

    fn push(&self, head_ref: &str, upstream_ref: Option<&str>) -> Result<(), Self::Error> {
        let head_ref = HeadRef::new(head_ref).unwrap();
        if let Some(upstream_ref) = upstream_ref {
            let upstream_ref = RemoteRef::new(upstream_ref).unwrap();
            self.run_command(
                std::process::Command::new("git")
                    .arg("push")
                    .arg("--force-with-lease")
                    .arg("--force-if-includes")
                    .arg("-u")
                    .arg(upstream_ref.remote())
                    .arg(head_ref.branch().unwrap()),
            )
        } else {
            self.run_command(
                std::process::Command::new("git")
                    .arg("push")
                    .arg("--force-with-lease")
                    .arg("--force-if-includes")
                    .arg("-u")
                    .arg("origin")
                    .arg(head_ref.branch().unwrap()),
            )
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Cli::parse().into_command().and_then(|cmd| cmd.run())
}
