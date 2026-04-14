use fnmatch_sys::FNM_NOESCAPE;
use std::ffi::{CStr, CString};

const INIT_DEFAULT_BRANCH: &str = "init.defaultbranch";
const DAH_BRANCH_PREFIX: &str = "dah.branchprefix";
const DAH_PROTECTED_BRANCHES: &str = "dah.protectedbranch";

pub struct Configuration<'a> {
    config: &'a git2::Config,
}

impl<'a> Configuration<'a> {
    pub fn new(config: &'a git2::Config) -> Self {
        Self { config }
    }

    pub fn init_default_branch(&self) -> Result<Option<String>, git2::Error> {
        get_optional_string(self.config, INIT_DEFAULT_BRANCH)
    }

    pub fn dah_branch_prefix(&self) -> Result<String, git2::Error> {
        Ok(get_optional_string(self.config, DAH_BRANCH_PREFIX)?.unwrap_or_default())
    }

    pub fn dah_protected_branches(&self) -> Result<Option<ProtectedBranches>, git2::Error> {
        Ok(get_optional_string(self.config, DAH_PROTECTED_BRANCHES)?
            .map(ProtectedBranches::from_config_value))
    }
}

pub struct ProtectedBranches {
    patterns: Vec<String>,
}

impl ProtectedBranches {
    fn from_config_value(value: String) -> Self {
        Self {
            patterns: value.split(':').map(ToOwned::to_owned).collect(),
        }
    }

    pub fn is_match(&self, branch_name: &str) -> bool {
        let branch_name = CString::new(branch_name).unwrap();
        self.patterns.iter().any(|pattern| {
            let pattern = CString::new(pattern.as_str()).unwrap();
            fnmatch(pattern.as_c_str(), branch_name.as_c_str())
        })
    }
}

fn get_optional_string(config: &git2::Config, key: &str) -> Result<Option<String>, git2::Error> {
    match config.get_string(key) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn fnmatch(pat: &CStr, s: &CStr) -> bool {
    let pat = pat.as_ptr();
    let s = s.as_ptr();

    unsafe { fnmatch_sys::fnmatch(pat, s, FNM_NOESCAPE) == 0 }
}

#[cfg(test)]
mod tests {
    use git2::{ConfigLevel, Repository};
    use tempfile::TempDir;

    use super::{Configuration, ProtectedBranches};

    #[test]
    fn init_default_branch_returns_none_when_unset() -> Result<(), Box<dyn std::error::Error>> {
        let config = git2::Config::new()?;

        assert_eq!(None, Configuration::new(&config).init_default_branch()?);

        Ok(())
    }

    #[test]
    fn dah_branch_prefix_returns_empty_string_when_unset() -> Result<(), Box<dyn std::error::Error>>
    {
        let config = git2::Config::new()?;

        assert_eq!("", Configuration::new(&config).dah_branch_prefix()?);

        Ok(())
    }

    #[test]
    fn dah_protected_branches_returns_none_when_unset() -> Result<(), Box<dyn std::error::Error>> {
        let config = git2::Config::new()?;

        assert!(Configuration::new(&config)
            .dah_protected_branches()?
            .is_none());

        Ok(())
    }

    #[test]
    fn protected_branches_matches_expected_globs() {
        let protected = ProtectedBranches::from_config_value("develop:release/*".to_owned());

        assert!(protected.is_match("develop"));
        assert!(protected.is_match("release/v1"));
    }

    #[test]
    fn protected_branches_does_not_match_partial_names() {
        let protected = ProtectedBranches::from_config_value("release/*".to_owned());

        assert!(!protected.is_match("release-latest"));
    }

    #[test]
    fn dah_protected_branches_reads_from_git_config() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = TempDir::new()?;
        let repo = Repository::init_bare(tmpdir.path())?;

        repo.config()?
            .open_level(ConfigLevel::Local)?
            .set_str("dah.protectedbranch", "develop:release/*")?;

        let config = repo.config()?;
        let protected = Configuration::new(&config)
            .dah_protected_branches()?
            .expect("protected branches should be configured");

        assert!(protected.is_match("develop"));
        assert!(protected.is_match("release/v2"));
        assert!(!protected.is_match("release-latest"));

        Ok(())
    }
}
