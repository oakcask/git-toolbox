use std::{io::BufRead, path::Path};

use git2::Repository;
use log::warn;

use crate::git::IndexStage;

use self::pattern::{Pattern, PatternError};

mod pattern;

#[derive(Debug, PartialEq)]
struct Record {
    pattern: String,
    owners: Vec<String>,
}

#[derive(PartialEq, Debug, thiserror::Error)]
enum CodeOwnersEntryError {
    #[error("pattern missing")]
    PatternMissing,
    #[error("{0}")]
    PatternError(String),
}

impl From<PatternError> for CodeOwnersEntryError {
    fn from(value: PatternError) -> Self {
        CodeOwnersEntryError::PatternError(value.to_string())
    }
}

impl TryFrom<String> for Record {
    type Error = CodeOwnersEntryError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = if let Some((i, _)) = value.chars().enumerate().find(|(_, c)| c == &'#') {
            &value[0..i]
        } else {
            &value[..]
        };

        let mut iter = value.split_whitespace();
        if let Some(pat) = iter.next() {
            let owners: Vec<String> = iter.map(|s| s.to_string()).collect();

            Ok(Record {
                pattern: pat.to_string(),
                owners,
            })
        } else {
            Err(Self::Error::PatternMissing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CodeOwnersEntryError, Record};

    #[test]
    fn parse() {
        let test_cases = [
            ("# * @foo @bar", Err(CodeOwnersEntryError::PatternMissing)),
            (
                "* # @foo @bar",
                Ok(Record {
                    pattern: "*".to_string(),
                    owners: vec![],
                }),
            ),
            (
                "* @foo",
                Ok(Record {
                    pattern: "*".to_string(),
                    owners: vec!["@foo".to_string()],
                }),
            ),
            (
                "* @foo # @bar",
                Ok(Record {
                    pattern: "*".to_string(),
                    owners: vec!["@foo".to_string()],
                }),
            ),
            (
                "* @foo @bar",
                Ok(Record {
                    pattern: "*".to_string(),
                    owners: vec!["@foo".to_string(), "@bar".to_string()],
                }),
            ),
        ];

        for (i, (input, want)) in test_cases.into_iter().enumerate() {
            let got = Record::try_from(input.to_string());
            assert!(
                got == want,
                "#{}: wants {:?} for {}, but got {:?}",
                i,
                want,
                input,
                got
            );
        }
    }
}

#[derive(Debug)]
pub struct CodeOwners<D: DebugInfo = ()> {
    // CODEOWNERS file entries, in reversed order.
    // Winning owners are from last-match entry in the file.
    entries: Vec<CodeOwnersEntry<D>>,
}

pub trait DebugInfo: Sized {
    fn parse(line: &str, line_no: usize) -> Self;
}

impl DebugInfo for () {
    fn parse(_line: &str, _line_no: usize) -> Self {
        ()
    }
}

#[derive(Debug)]
struct CodeOwnersEntry<D: DebugInfo = ()> {
    pattern: Pattern,
    owners: Vec<String>,
    debug: D,
}

pub struct Match<'a, D: DebugInfo> {
    entry: &'a CodeOwnersEntry<D>,
    effective: bool,
}

impl<'a, D: DebugInfo> Match<'a, D> {
    pub fn owners(&self) -> &Vec<String> {
        &self.entry.owners
    }

    pub fn is_effective(&self) -> bool {
        self.effective
    }
    
    pub fn debug_info(&self) -> &'a D {
        &self.entry.debug
    }
}

impl<D: DebugInfo> CodeOwnersEntry<D> {
    pub fn parse(value: String, line_no: usize) -> Result<Self, CodeOwnersEntryError> {
        let debug = D::parse(&value, line_no);
        let Record { pattern, owners } = Record::try_from(value)?;

        Ok(Self {
            pattern: Pattern::new(pattern)?,
            owners,
            debug,
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CodeOwnersError {
    #[error(
        "CODEOWNERS file is not indexed in the repository; did you already commit or stage it?"
    )]
    NotIndexed,
    #[error("libgit2 API error: {0}")]
    GitError(#[from] git2::Error),
    #[error("i/o error: {0}")]
    IOError(#[from] std::io::Error),
}

impl<D: DebugInfo> CodeOwners<D> {
    /// Parse CODEOWNERS file data in buffer.
    ///
    /// Examples
    ///
    /// ```
    /// use git_toolbox::github::codeowners::CodeOwners;
    ///
    /// let data = r#"
    /// *.js frontend-developer
    /// "#;
    /// let codeowners = CodeOwners::try_from_bufread(data.as_bytes()).unwrap();
    ///
    /// assert_eq!(codeowners.find_owners("foo.ts"), None);
    /// assert_eq!(codeowners.find_owners("foo/bar.js"), Some(&vec![String::from("frontend-developer")]));
    /// ```
    pub fn try_from_bufread<T: BufRead>(blob: T) -> Result<Self, CodeOwnersError> {
        // Forgetting errors in parsing is reasonable the repository barely contains invalid code owner records,
        // as GitHub enforces CODEOWNERS file being valid.
        // (and we are reading CODEOWNERS from index)
        let entries: Vec<CodeOwnersEntry<D>> = blob
            .lines()
            .enumerate()
            .filter_map(|(idx, ln)| match ln {
                Ok(s) => match CodeOwnersEntry::<_>::parse(s, idx + 1) {
                    Ok(entry) => Some(entry),
                    Err(CodeOwnersEntryError::PatternMissing) => None,
                    Err(e) => {
                        warn!("line {} at CODEOWNERS: {}", idx + 1, e);
                        None
                    }
                },
                Err(e) => {
                    warn!("line {} at CODEOWNERS: {}", idx + 1, e);
                    None
                }
            })
            .collect();

        Ok(CodeOwners { entries })
    }

    /// Read CODEOWNERS file from repository's index.
    pub fn try_from_repo(repo: &Repository) -> Result<Self, CodeOwnersError> {
        let path = Path::new(".github/CODEOWNERS");

        if let Some(entry) = repo.index()?.get_path(path, IndexStage::Normal.into()) {
            let blob = repo
                .find_object(entry.id, Some(git2::ObjectType::Blob))?
                .into_blob()
                .unwrap();
            Ok(Self::try_from_bufread(blob.content())?)
        } else {
            Err(CodeOwnersError::NotIndexed)
        }
    }

    pub fn debug<'a, 'b>(&'a self, path: &str) -> impl Iterator<Item = Match<'a, D>> {
        self.entries
            .iter()
            .filter(|&entry| entry.pattern.is_match(path))
            .rev()
            .enumerate()
            .map(|(nth, entry)| Match { entry, effective: nth == 0 })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
    }

    /// Find owners for matching path.
    pub fn find_owners(&self, path: &str) -> Option<&Vec<String>> {
        let entry = self
            .entries
            .iter()
            .rev()
            .find(|&entry| entry.pattern.is_match(path));

        entry.map(|entry| &entry.owners)
    }
}
