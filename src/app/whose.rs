use std::{ffi::OsStr, os::unix::ffi::OsStrExt as _};

use git2::{Pathspec, PathspecFlags, Repository};
use log::info;

use crate::{
    github::codeowners::{CodeOwners, CodeOwnersError},
    pathname,
};

pub struct Application {
    pub repo: Repository,
    pub codeowners: CodeOwners,
    pub pathspecs: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ApplicationError {
    #[error("{0}")]
    Git(#[from] git2::Error),
    #[error("{0}")]
    PathError(#[from] pathname::NormalizePathError),
    #[error("{0}")]
    CodeOwnersError(#[from] CodeOwnersError),
}

impl Application {
    pub fn run(&self) -> Result<(), ApplicationError> {
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

pub struct ApplicationBuilder {
    repo: Repository,
    pathspecs: Vec<String>,
}

impl ApplicationBuilder {
    pub fn new(repo: Repository) -> Self {
        Self {
            repo,
            pathspecs: Default::default(),
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

    pub fn build(self) -> Result<Application, ApplicationError> {
        let codeowners = CodeOwners::try_from_repo(&self.repo)?;
        Ok(Application {
            repo: self.repo,
            codeowners,
            pathspecs: self.pathspecs,
        })
    }
}
