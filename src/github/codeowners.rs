use std::{io::BufRead, path::Path};

use git2::Repository;
use log::warn;

use crate::git2_consts::IndexStage;

use self::pattern::{Pattern, PatternError};

mod pattern;

#[derive(Debug, PartialEq)]
struct Record {
    pattern: String,
    owners: Vec<String>
}

#[derive(PartialEq, Debug, thiserror::Error)]
enum RecordError {
    #[error("pattern missing")]
    PatternMissing,
}

impl TryFrom<String> for Record {
    type Error = RecordError;
    
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
                owners
            })
        } else {
            Err(Self::Error::PatternMissing)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RecordError, Record};

    #[test]
    fn parse() {
        let test_cases = [
            ("# * @foo @bar", Err(RecordError::PatternMissing)),
            ("* # @foo @bar", Ok(Record { pattern: "*".to_string(), owners: vec![] })),
            ("* @foo", Ok(Record { pattern: "*".to_string(), owners: vec!["@foo".to_string()]})),
            ("* @foo # @bar", Ok(Record { pattern: "*".to_string(), owners: vec!["@foo".to_string()]})),
            ("* @foo @bar", Ok(Record { pattern: "*".to_string(), owners: vec!["@foo".to_string(), "@bar".to_string()]})),
        ];

        for (i, (input, want)) in test_cases.into_iter().enumerate() {
            let got = Record::try_from(input.to_string());
            assert!(got == want, "#{}: wants {:?} for {}, but got {:?}", i, want, input, got);
        }
    }
}


#[derive(Debug)]
pub struct CodeOwners {
    // CODEOWNERS file entries, in reversed order.
    // Winning owners are from last-match entry in the file.
    entries: Vec<CodeOwnersEntry>
}

#[derive(Debug)]
struct CodeOwnersEntry {
    pattern: Pattern,
    owners: Vec<String>
}

impl TryFrom<Record> for CodeOwnersEntry {
    type Error = PatternError;

    fn try_from(value: Record) -> Result<Self, Self::Error> {
        let Record { pattern, owners } = value;

        Ok(CodeOwnersEntry {
            pattern: Pattern::new(pattern)?,
            owners
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum CodeOwnersError {
    #[error("CODEOWNERS file is not indexed in the repository; did you already commit or stage it?")]
    NotIndexed,
    #[error("libgit2 API error: {0}")]
    GitError(#[from] git2::Error),
    #[error("i/o error: {0}")]
    IOError(#[from] std::io::Error)
}

impl CodeOwners {
    pub fn new(repo: &Repository) -> Result<CodeOwners, CodeOwnersError> {
        let path = Path::new(".github/CODEOWNERS");
        let index =  repo.index()?;

        if let Some(entry) = index.get_path(path, IndexStage::Normal.into()) {
            let blob = repo.find_object(entry.id, Some(git2::ObjectType::Blob))?.into_blob().unwrap();

            // Forgetting errors in parsing is reasonable the repository barely contains invalid code owner records,
            // as GitHub enforces CODEOWNERS file being valid.
            // (and we are reading CODEOWNERS from index)
            let mut entries: Vec<CodeOwnersEntry> = blob.content().lines().enumerate().filter_map(|(idx, ln)| {
                match ln {
                    Ok(s) => {
                        match Record::try_from(s) {
                            Ok(r) => {
                                match CodeOwnersEntry::try_from(r) {
                                    Ok(entry) => {
                                        Some(entry)
                                    },
                                    Err(e) => {
                                        warn!("line {} at CODEOWNERS: {}", idx + 1, e);
                                        None
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("line {} at CODEOWNERS: {}", idx + 1, e);
                                None                                
                            }
                        }
                    }
                    Err(e) => {
                        warn!("line {} at CODEOWNERS: {}", idx + 1, e);
                        None
                    }
                }
            }).collect();
            entries.reverse();

            Ok(CodeOwners { entries })
        } else {
            Err(CodeOwnersError::NotIndexed)
        }
    }

    pub fn find_owners(&self, path: &str) -> Option<&Vec<String>> {
        let entry = self.entries.iter().find(|&entry| {
            entry.pattern.is_match(path)
        });

        entry.map(|entry| &entry.owners)
    }
}
