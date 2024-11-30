#[derive(Debug, PartialEq, Clone)]
enum HeadRefImpl {
    Branch { full: String },
    Detached,
}

#[derive(Debug, PartialEq, Clone)]
pub struct HeadRef(HeadRefImpl);

#[derive(Debug, PartialEq, Clone)]
pub struct RemoteRef {
    full: String,
    remote_len: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum RefnameError {
    #[error("head ref name should be like refs/heads/BRANCH or just HEAD, but got {refname}")]
    InvalidHeadRefFormat { refname: String },
    #[error("remote ref name should be like refs/remotes/REMOTE/BRANCH, but got {refname}")]
    InvalidRemoteRefFormat { refname: String },
}

impl HeadRef {
    pub fn new<S: Into<String> + AsRef<str>>(refname: S) -> Result<HeadRef, RefnameError> {
        HeadRefImpl::new(refname).map(HeadRef)
    }

    pub fn detached() -> HeadRef {
        HeadRef(HeadRefImpl::Detached)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_string(self) -> String {
        self.0.into_string()
    }

    pub fn branch(&self) -> Option<&str> {
        self.0.branch()
    }
}

impl HeadRefImpl {
    const PREFIX: &'static str = "refs/heads/";
    const HEAD: &'static str = "HEAD";

    fn new<S: Into<String> + AsRef<str>>(refname: S) -> Result<HeadRefImpl, RefnameError> {
        if refname.as_ref() == Self::HEAD {
            Ok(HeadRefImpl::Detached)
        } else {
            let refname: String = refname.into();
            if refname.strip_prefix(Self::PREFIX).is_some() {
                Ok(HeadRefImpl::Branch { full: refname })
            } else {
                Err(RefnameError::InvalidHeadRefFormat { refname })
            }
        }
    }

    fn as_str(&self) -> &str {
        match self {
            HeadRefImpl::Branch { full, .. } => full,
            HeadRefImpl::Detached => Self::HEAD,
        }
    }

    fn into_string(self) -> String {
        match self {
            HeadRefImpl::Branch { full } => full,
            HeadRefImpl::Detached => Self::HEAD.to_owned(),
        }
    }

    fn branch(&self) -> Option<&str> {
        match self {
            HeadRefImpl::Branch { full } => Some(&full[Self::PREFIX.len()..]),
            HeadRefImpl::Detached => None,
        }
    }
}

impl RemoteRef {
    const PREFIX: &'static str = "refs/remotes/";

    pub fn new<S: Into<String>>(refname: S) -> Result<RemoteRef, RefnameError> {
        let refname: String = refname.into();
        if let Some(remote_and_branch) = refname.strip_prefix(Self::PREFIX) {
            if let Some((remote, _branch)) = remote_and_branch.split_once('/') {
                let remote_len = remote.len();
                return Ok(RemoteRef {
                    full: refname,
                    remote_len,
                });
            }
        }

        Err(RefnameError::InvalidRemoteRefFormat { refname })
    }

    pub fn as_str(&self) -> &str {
        self.full.as_str()
    }

    pub fn remote(&self) -> &str {
        let i = Self::PREFIX.len();
        let j = i + self.remote_len;
        &self.full[i..j]
    }

    pub fn branch(&self) -> &str {
        let i = Self::PREFIX.len() + self.remote_len + "/".len();
        &self.full[i..]
    }
}

#[cfg(test)]
mod tests {
    use crate::git::refname::HeadRef;

    use super::RemoteRef;

    #[test]
    fn test_valid_head_ref() {
        let cases = [
            ("refs/heads/foo", Some("foo")),
            ("refs/heads/foo/bar", Some("foo/bar")),
            ("HEAD", None),
        ];

        for (given, want_branch) in cases {
            let got = HeadRef::new(given);
            assert!(got.is_ok_and(|r| r.as_str() == given && r.branch() == want_branch));
        }
    }

    #[test]
    fn test_invalid_head_ref() {
        let cases = ["foo", "foo/bar", "refs/tags/v0", "refs/remotes/origin/foo"];

        for given in cases {
            let got = HeadRef::new(given);
            assert!(got.is_err())
        }
    }

    #[test]
    fn test_valid_remote_ref() {
        let cases = [
            ("refs/remotes/origin/foo", "origin", "foo"),
            ("refs/remotes/origin/foo/bar", "origin", "foo/bar"),
        ];

        for (given, want_remote, want_branch) in cases {
            let got = RemoteRef::new(given);
            assert!(got.is_ok_and(|r| r.as_str() == given
                && r.remote() == want_remote
                && r.branch() == want_branch));
        }
    }

    #[test]
    fn test_invalid_remote_ref() {
        let cases = [
            "foo",
            "foo/bar",
            "refs/heads/foo/bar",
            "refs/remotes/origin",
        ];

        for given in cases {
            let got = RemoteRef::new(given);
            assert!(got.is_err())
        }
    }
}
