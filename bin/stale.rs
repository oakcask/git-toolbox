use chrono::{DateTime, Local};
use clap::{arg, Parser};
use git2::{Branch, BranchType, PushOptions, RemoteCallbacks, Repository};
use git_toolbox::{git::GitTime, reltime::Reltime};
use log::{error, info, warn};
use std::{collections::HashMap, error::Error, process::exit};

#[derive(Parser)]
#[command(
    about = "List or delete stale branches",
    long_about = None)]
struct Cli {
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

fn parse_reltime(arg: &str) -> Result<Reltime, String> {
    Reltime::try_from(arg).map_err(|e| format!("while parsing {} got error: {}", arg, e))
}

struct Command {
    repo: Repository,
    delete: bool,
    push: bool,
    since: Option<DateTime<Local>>,
    branches: Vec<String>,
}

impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        if self.delete && self.push {
            let refspecs: HashMap<String, Vec<String>> = HashMap::new();
            let mut refspecs = self.for_each(refspecs, |mut refspecs, branch| {
                let upstream = branch.upstream()?;
                let upstream = upstream.get();
                let upstream = upstream
                    .name()
                    .and_then(|u| u.strip_prefix("refs/remotes/"))
                    .and_then(|u| u.split('/').next());
                let branch_name = branch.get().name();

                if let (Some(remote_name), Some(branch_name)) = (upstream, branch_name) {
                    info!(
                        "branch '{}' will be deleted from {}",
                        branch_name, remote_name
                    );

                    // refspec has <src>:<dst> format, so leaving <src> empty will delete <dst>.
                    let refspec = format!(":{}", branch_name);
                    if let Some(branches) = refspecs.get_mut(remote_name) {
                        branches.push(refspec)
                    } else {
                        refspecs.insert(remote_name.to_owned(), vec![refspec]);
                    }
                }

                Ok(refspecs)
            })?;
            for (remote_name, refspecs) in refspecs.drain() {
                let mut remote = self.repo.find_remote(&remote_name)?;
                let mut callbacks = RemoteCallbacks::new();
                callbacks.push_update_reference(|refname, status| {
                    if let Some(error) = status {
                        warn!("push failed: {}, status = {}", refname, error);
                    } else {
                        info!("pushed: {}", refname);
                    }
                    Ok(())
                });
                let mut push_options = PushOptions::new();
                push_options.remote_callbacks(callbacks);
                if let Err(e) = remote.push(refspecs.as_slice(), Some(&mut push_options)) {
                    warn!("failed to remove branches from {}: {}", remote_name, e)
                }
            }
        } else if self.delete {
            self.for_each((), |_, mut branch| {
                if let Some(branch_name) = branch.get().name() {
                    let branch_name = branch_name.to_owned();
                    if let Err(e) = branch.delete() {
                        warn!("failed to remove branch '{}': {}", branch_name, e)
                    }
                }
                Ok(())
            })?;
        } else {
            self.for_each((), |_, branch| {
                println!("{}", branch.get().name().unwrap());
                Ok(())
            })?;
        }
        Ok(())
    }

    fn for_each<S, F: Fn(S, Branch<'_>) -> Result<S, Box<dyn Error>>>(
        &self,
        init: S,
        f: F,
    ) -> Result<S, Box<dyn Error>> {
        let mut st = init;
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            if !self.match_branch(&branch)? {
                continue;
            }

            let commit = branch.get().peel_to_commit()?;
            let commit_time: GitTime = commit.time().into();

            if let Some(s) = self.since {
                if s > commit_time.into() {
                    st = f(st, branch)?;
                }
            } else if branch.upstream().is_err() {
                st = f(st, branch)?;
            }
        }

        Ok(st)
    }

    fn match_branch(&self, branch: &Branch) -> Result<bool, Box<dyn Error>> {
        match branch.name()? {
            None => Ok(false),
            Some(branch_name) => {
                if branch.is_head() {
                    info!(
                        "branch '{}' ignored. NOTE: HEAD branch is always ignored.",
                        branch_name
                    );
                    Ok(false)
                } else if self.branches.is_empty() {
                    Ok(true)
                } else {
                    match self
                        .branches
                        .iter()
                        .find(|&prefix| branch_name.starts_with(prefix))
                    {
                        Some(_) => Ok(true),
                        None => Ok(false),
                    }
                }
            }
        }
    }
}

impl Cli {
    fn into_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let now = Local::now();
        let since = self.since.map(|s| now - s);

        Ok(Command {
            repo,
            delete: self.delete,
            push: self.push,
            since,
            branches: self.branches,
        })
    }
}

fn main() -> ! {
    env_logger::init();
    match Cli::parse().into_command().and_then(|cmd| cmd.run()) {
        Err(e) => {
            error!("{}", e);
            exit(1)
        }
        Ok(_) => exit(0),
    }
}
