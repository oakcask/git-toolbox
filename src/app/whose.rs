use std::{ffi::OsStr, fmt::Debug, os::unix::ffi::OsStrExt as _};

use git2::{Pathspec, PathspecFlags, Repository};
use log::info;

use crate::{
    github::codeowners::{self, CodeOwners, CodeOwnersError},
    pathname,
};

#[derive(thiserror::Error, Debug)]
pub enum ApplicationError {
    #[error("{0}")]
    Git(#[from] git2::Error),
    #[error("{0}")]
    PathError(#[from] pathname::NormalizePathError),
    #[error("{0}")]
    CodeOwnersError(#[from] CodeOwnersError),
}

pub trait Application {
    fn run(&self) -> Result<(), ApplicationError>;
}

struct FinderApplication {
    repo: Repository,
    codeowners: CodeOwners,
    pathspecs: Vec<String>,
}

impl Application for FinderApplication {
    fn run(&self) -> Result<(), ApplicationError> {
        env_logger::init();

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

struct DebugInfo {
    line: String,
    line_no: usize,
}

impl codeowners::DebugInfo for DebugInfo {
    fn parse(line: &str, line_no: usize) -> Self {
        Self {
            line: line.to_owned(),
            line_no,
        }
    }
}

struct DebugApplication {
    repo: Repository,
    codeowners: CodeOwners<DebugInfo>,
    pathspecs: Vec<String>,
}

impl Application for DebugApplication {
    fn run(&self) -> Result<(), ApplicationError> {
        env_logger::init();

        let index = self.repo.index()?;
        let pathspec = Pathspec::new(self.pathspecs.iter())?;
        let matches = pathspec.match_index(&index, PathspecFlags::default())?;

        for entry in matches.entries() {
            let path = OsStr::from_bytes(entry);
            if let Some(path) = OsStr::from_bytes(entry).to_str() {
                // export in TOML
                for e in self.codeowners.debug(path) {
                    let debug = e.debug_info();
                    println!("[[{:?}]]", path);
                    println!("line = {:?}", debug.line_no);
                    println!("rule = {:?}", debug.line);
                    println!("owners = {:?}", e.owners());
                    println!("effective = {:?}", e.is_effective());
                }
            } else {
                log::error!("cannot convet {:?} into utf-8 string.", path)
            }
        }

        Ok(())
    }
}

pub struct ApplicationBuilder {
    repo: Repository,
    pathspecs: Vec<String>,
    debug: bool,
}

impl ApplicationBuilder {
    pub fn new(repo: Repository) -> Self {
        Self {
            repo,
            pathspecs: Default::default(),
            debug: Default::default(),
        }
    }

    pub fn with_pathspecs(self, pathspecs: Vec<String>) -> Result<Self, ApplicationError> {
        let pathspecs = if self.repo.is_bare() {
            info!("this is bare repository");
            self.pathspecs
        } else {
            pathname::normalize_paths(&self.repo, pathspecs)?
        };

        Ok(Self { pathspecs, ..self })
    }

    pub fn with_debug(self, debug: bool) -> Self {
        Self { debug, ..self }
    }

    pub fn build(self) -> Result<Box<dyn Application>, ApplicationError> {
        if self.debug {
            let codeowners = CodeOwners::<DebugInfo>::try_from_repo(&self.repo)?;
            Ok(Box::new(DebugApplication {
                repo: self.repo,
                codeowners,
                pathspecs: self.pathspecs,
            }))
        } else {
            let codeowners = CodeOwners::try_from_repo(&self.repo)?;
            Ok(Box::new(FinderApplication {
                repo: self.repo,
                codeowners,
                pathspecs: self.pathspecs,
            }))
        }
    }
}
