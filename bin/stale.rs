use std::error::Error;
use chrono::{DateTime, Local};
use clap::{arg, Parser};
use git2::{Branch, BranchType, PushOptions, RemoteCallbacks, Repository};
use git_toolbox::{gittime::GitTime, reltime::Reltime};
use log::{info, warn};

#[derive(Parser)]
#[command(
    about = "List or delete stale branches",
    long_about = None)]
struct Cli {
    #[arg(short, long,
        help = "Perform deletion of selected branches")]
    delete: bool,
    #[arg(long,
        help = "Combined with --delete, perform deletion on remote repository instead")]
    push: bool,
    #[arg(long,
        help = "Select local branch with commit times older than the specified relative time",
        value_parser = parse_reltime)]
    since: Option<Reltime>,
    #[arg(help = "Select branches with specified prefixes, or select all if unset")]
    branches: Vec<String>
}

fn parse_reltime(arg: &str) -> Result<Reltime, String> {
    Reltime::try_from(arg).map_err(|e| format!("while parsing {} got error: {}", arg, e.to_string()))
}

struct Command {
    repo: Repository,
    delete: bool,
    push: bool,
    since: Option<DateTime<Local>>,
    branches: Vec<String>
}


impl Command {
    fn run(&self) -> Result<(), Box<dyn Error>> {
        for branch in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            if !self.match_branch(&branch)? {
                continue;
            }
            
            let commit = branch.get().peel_to_commit()?;
            let commit_time: GitTime = commit.time().into();

            if let Some(s) = self.since {
                if s > commit_time.into() {
                    self.process(branch)?;
                }
            } else if let Err(_) = branch.upstream() {
                self.process(branch)?;
            }
        }

        Ok(())
    }

    fn match_branch(&self, branch: &Branch) -> Result<bool, Box<dyn Error>> {
        match branch.name()? {
            None => Ok(false),
            Some(branch_name) => {
                if branch.is_head() {
                    info!("branch '{}' ignored. NOTE: HEAD branch is always ignored.", branch_name);
                    Ok(false)
                } else {
                    if self.branches.is_empty() {
                        Ok(true)
                    } else {
                        match self.branches.iter().find(|&prefix| branch_name.starts_with(prefix)) {
                            Some(_) => Ok(true),
                            None => Ok(false)
                        }
                    }
                }
            }
        }
    }
    
    fn process<'a>(&self, branch: Branch<'a>) -> Result<(), Box<dyn Error>> {
        if self.delete && self.push {
            let upstream = branch.upstream()?;
            let upstream = upstream.get();
            let upstream = upstream.name()
                .and_then(|u| u.strip_prefix("refs/remotes/"))
                .and_then(|u| u.split('/').into_iter().next());
            let branch_name = branch.get().name();

            if let (Some(remote), Some(branch_name)) = (upstream, branch_name) {
                info!("branch '{}' will be deleted from {}", branch_name, remote);

                // refspec has <src>:<dst> format, so leaving <src> empty will delete <dst>.
                let refspec = format!(":{}", branch_name);
                let mut remote = self.repo.find_remote(remote)?;
                let mut callbacks = RemoteCallbacks::new();
                callbacks.push_update_reference(|refname, status| {
                    if let Some(error) = status {
                        warn!("push failed: {}, status = {}", refname, error);
                    }
                    Ok(())
                });
                let mut push_options = PushOptions::new();
                push_options.remote_callbacks(callbacks);

                remote.push(&[refspec], Some(&mut push_options))?;
            }
        } else if self.delete {
            let mut branch = branch;
            branch.delete()?;
        } else {
            println!("{}", branch.get().name().unwrap());
        }

        Ok(())
    } 
}

impl Cli {
    fn to_command(self) -> Result<Command, Box<dyn Error>> {
        let repo = Repository::open_from_env()?;
        let now = Local::now();
        let since = self.since.map(|s| now - s);

        Ok(Command {
            repo,
            delete: self.delete,
            push: self.push,
            since,
            branches: self.branches
        })
    }  
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    Cli::parse().to_command()?.run()
}
