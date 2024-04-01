use std::path::{Component, PathBuf, MAIN_SEPARATOR_STR};

/// Canonicalize path, but without symlink resolve.
pub fn canonicalize(path: PathBuf) -> PathBuf {
    let mut buf = PathBuf::with_capacity(path.capacity());

    for part in path.components() {
        match part {
            Component::Prefix(pfx) => {
                buf.push(pfx.as_os_str());
            }
            Component::RootDir => {
                buf.push(MAIN_SEPARATOR_STR)
            }
            Component::CurDir => {
                // do nothing
            }
            Component::ParentDir => {
                buf.pop();
            }
            Component::Normal(p) => {
                buf.push(p);                    
            }
        }
    }

    buf
}

#[cfg(test)]
mod tests {
    use std::{path::PathBuf, str::FromStr};

    #[test]
    #[cfg(unix)]
    fn test_canonicalize() {
        let cases = [
            ("a", "a"),
            ("a/b", "a/b"),
            ("a/./b", "a/b"),
            ("a//b", "a/b"),
            ("/a/b", "/a/b"),
            ("/a/b/..", "/a"),
        ]; 

        for (idx, (path, want)) in cases.into_iter().enumerate() {
            let got = super::canonicalize(PathBuf::from_str(path).unwrap());
            assert_eq!(got.to_str(), Some(want), "#{} wants {} but got {:?}", idx, want, got);
        }
    }
}