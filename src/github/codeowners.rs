use std::{io::BufRead, path::Path};

use git2::{ErrorCode, Repository};
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
    use std::path::Path;

    use git2::{Repository, Signature};

    use super::{CodeOwners, CodeOwnersEntryError, Record};

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
                "#{i}: wants {want:?} for {input}, but got {got:?}"
            );
        }
    }

    #[test]
    fn try_from_repo_reads_from_head_when_absent_from_index() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = Signature::now("t", "t@example.com").unwrap();

        let co_path = dir.path().join(".github").join("CODEOWNERS");
        std::fs::create_dir_all(co_path.parent().unwrap()).unwrap();
        std::fs::write(&co_path, "* @owner\n").unwrap();

        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new(".github/CODEOWNERS")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
                .unwrap();
        }

        {
            let mut index = repo.index().unwrap();
            index.remove_path(Path::new(".github/CODEOWNERS")).unwrap();
            index.write().unwrap();
        }

        let co = CodeOwners::<()>::try_from_repo(&repo).unwrap();
        assert_eq!(co.find_owners("any.rs"), Some(&vec!["@owner".to_string()]));
    }
}

#[derive(Debug)]
pub struct CodeOwners<D: DebugInfo = ()> {
    entries: Vec<CodeOwnersEntry<D>>,
}

pub trait DebugInfo: Sized {
    fn parse(line: &str, line_no: usize) -> Self;
}

impl DebugInfo for () {
    fn parse(_line: &str, _line_no: usize) -> Self {}
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
        "missing CODEOWNERS; must be in INDEX or HEAD for .github/CODEOWNERS, CODEOWNERS, or docs/CODEOWNERS"
    )]
    Missing,
    #[error("libgit2 API error: {0}")]
    GitError(#[from] git2::Error),
    #[error("i/o error: {0}")]
    IOError(#[from] std::io::Error),
}

fn blob_for_codeowners_path<'a>(
    repo: &'a Repository,
    path: &Path,
) -> Result<Option<git2::Blob<'a>>, CodeOwnersError> {
    if let Some(entry) = repo.index()?.get_path(path, IndexStage::Normal.into()) {
        let blob = repo
            .find_object(entry.id, Some(git2::ObjectType::Blob))?
            .into_blob()
            .unwrap();
        return Ok(Some(blob));
    }

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return Ok(None),
    };
    let commit = head.peel_to_commit()?;
    let tree = commit.tree()?;
    let tree_entry = match tree.get_path(path) {
        Ok(e) => e,
        Err(e) if e.code() == ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let blob = repo
        .find_object(tree_entry.id(), Some(git2::ObjectType::Blob))?
        .into_blob()
        .unwrap();
    Ok(Some(blob))
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
    /// let codeowners = CodeOwners::<()>::try_from_bufread(data.as_bytes()).unwrap();
    ///
    /// assert_eq!(codeowners.find_owners("foo.ts"), None);
    /// assert_eq!(codeowners.find_owners("foo/bar.js"), Some(&vec![String::from("frontend-developer")]));
    /// ```
    pub fn try_from_bufread<T: BufRead>(blob: T) -> Result<Self, CodeOwnersError> {
        // Forgetting errors in parsing is reasonable the repository barely contains invalid code owner records,
        // as GitHub enforces CODEOWNERS file being valid.
        // (content comes from the index or from HEAD — see try_from_repo)
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

    /// Read CODEOWNERS from the repository.
    ///
    /// For each of GitHub's locations (`.github/CODEOWNERS`, `CODEOWNERS`, `docs/CODEOWNERS`),
    /// the staged copy in the index is tried first, then the blob at that path in `HEAD`'s tree.
    pub fn try_from_repo(repo: &Repository) -> Result<Self, CodeOwnersError> {
        let paths = [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"];
        for path in paths {
            let path = Path::new(path);
            if let Some(blob) = blob_for_codeowners_path(repo, path)? {
                return Self::try_from_bufread(blob.content());
            }
        }
        Err(CodeOwnersError::Missing)
    }

    pub fn debug<'a>(&'a self, path: &str) -> impl Iterator<Item = Match<'a, D>> {
        self.entries
            .iter()
            .filter(|&entry| entry.pattern.is_match(path))
            .rev()
            .enumerate()
            .map(|(nth, entry)| Match {
                entry,
                effective: nth == 0,
            })
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
