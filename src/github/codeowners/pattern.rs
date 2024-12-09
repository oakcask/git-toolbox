use std::io::Write;

use regex::Regex;

#[derive(Debug)]
pub struct Pattern {
    re: Regex,
}

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum PatternError {
    #[error("pattern is empty")]
    Empty,
    #[error("pattern compilation failed: {pattern}, {error}")]
    CompileError {
        pattern: String,
        error: regex::Error,
    },
}

impl Pattern {
    pub fn new(pattern: String) -> Result<Pattern, PatternError> {
        let pat = Self::compile(&pattern)?;
        let re = Regex::new(&pat).map_err(|error| PatternError::CompileError { pattern, error })?;
        Ok(Pattern { re })
    }

    pub fn is_match(&self, path: &str) -> bool {
        self.re.is_match(path)
    }

    fn compile(pattern: &str) -> Result<String, PatternError> {
        enum State {
            Head(Vec<u8>),
            HeadAsterisk(Vec<u8>),
            Default(Vec<u8>, Vec<u8>),
            Asterisk(Vec<u8>),
            DoubleAsterisk(Vec<u8>),
            DoubleAsteriskSlash(Vec<u8>),
            Slash(Vec<u8>),
        }
        let state = pattern
            .chars()
            .fold(State::Head(Vec::new()), |st, c| match st {
                State::Head(mut re_buf) => {
                    if c == '/' {
                        write!(&mut re_buf, r"\A").unwrap();
                        State::Default(re_buf, Vec::new())
                    } else if c == '*' {
                        write!(&mut re_buf, r"(?:\A|/)").unwrap();
                        State::HeadAsterisk(re_buf)
                    } else if c == '?' {
                        write!(&mut re_buf, r"(?:\A|/)[^/]").unwrap();
                        State::Default(re_buf, Vec::new())
                    } else {
                        let mut escape = Vec::new();
                        write!(&mut re_buf, r"(?:\A|/)").unwrap();
                        write!(&mut escape, "{}", c).unwrap();
                        State::Default(re_buf, escape)
                    }
                }
                State::Default(mut re_buf, mut escape) => {
                    if c == '/' {
                        let s = unsafe { String::from_utf8_unchecked(escape) };
                        write!(&mut re_buf, "{}", regex::escape(&s)).unwrap();
                        State::Slash(re_buf)
                    } else if c == '*' {
                        let s = unsafe { String::from_utf8_unchecked(escape) };
                        write!(&mut re_buf, "{}", regex::escape(&s)).unwrap();
                        State::Asterisk(re_buf)
                    } else if c == '?' {
                        let s = unsafe { String::from_utf8_unchecked(escape) };
                        write!(&mut re_buf, r"{}[^/]", regex::escape(&s)).unwrap();
                        State::Default(re_buf, Vec::new())
                    } else {
                        write!(&mut escape, "{}", c).unwrap();
                        State::Default(re_buf, escape)
                    }
                }
                State::Asterisk(mut re_buf) | State::HeadAsterisk(mut re_buf) => {
                    if c == '/' {
                        write!(&mut re_buf, r"[^/]*").unwrap();
                        State::Slash(re_buf)
                    } else if c == '*' {
                        State::DoubleAsterisk(re_buf)
                    } else if c == '?' {
                        write!(&mut re_buf, r"[^/]+").unwrap();
                        State::Default(re_buf, Vec::new())
                    } else {
                        let mut escape = Vec::new();
                        write!(&mut re_buf, r"[^/]*").unwrap();
                        write!(&mut escape, "{}", c).unwrap();
                        State::Default(re_buf, escape)
                    }
                }
                State::DoubleAsterisk(mut re_buf) => {
                    if c == '/' {
                        write!(&mut re_buf, r"(?:[^/]+/)*").unwrap();
                        State::DoubleAsteriskSlash(re_buf)
                    } else if c == '*' {
                        State::DoubleAsterisk(re_buf)
                    } else if c == '?' {
                        write!(&mut re_buf, r"[^/]+").unwrap();
                        State::Default(re_buf, Vec::new())
                    } else {
                        let mut escape = Vec::new();
                        write!(&mut re_buf, r"[^/]*").unwrap();
                        write!(&mut escape, "{}", c).unwrap();
                        State::Default(re_buf, escape)
                    }
                }
                State::DoubleAsteriskSlash(mut re_buf) => {
                    if c == '/' {
                        State::DoubleAsteriskSlash(re_buf)
                    } else if c == '*' {
                        State::Asterisk(re_buf)
                    } else if c == '?' {
                        write!(&mut re_buf, r"[^/]").unwrap();
                        State::Default(re_buf, Vec::new())
                    } else {
                        let mut escape = Vec::new();
                        write!(&mut escape, "{}", c).unwrap();
                        State::Default(re_buf, escape)
                    }
                }
                State::Slash(mut re_buf) => {
                    if c == '/' {
                        State::Slash(re_buf)
                    } else if c == '*' {
                        write!(&mut re_buf, r"/").unwrap();
                        State::Asterisk(re_buf)
                    } else if c == '?' {
                        write!(&mut re_buf, r"/[^/]").unwrap();
                        State::Default(re_buf, Vec::new())
                    } else {
                        let mut escape = Vec::new();
                        write!(&mut re_buf, r"/").unwrap();
                        write!(&mut escape, "{}", c).unwrap();
                        State::Default(re_buf, escape)
                    }
                }
            });

        match state {
            State::Head(_) => Err(PatternError::Empty)?,
            State::Default(mut re_buf, escape) => {
                // add [/\z] to pattern and path for preventing partial match.
                // Pattern `path/to/foo` should only maches directory or file named `foo` under
                // `path/to` directory. `path/to/foobar` shouldn't match.
                let s = unsafe { String::from_utf8_unchecked(escape) };
                write!(&mut re_buf, r"{}(?:/|\z)", regex::escape(&s)).unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_buf) })
            }
            State::Asterisk(mut re_buf) => {
                // trailing asterisk doesn't match further nested path
                write!(&mut re_buf, r"[^/]*\z").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_buf) })
            }
            State::HeadAsterisk(mut re_buf) => {
                // lone single asterisk should match everything
                write!(&mut re_buf, r"").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_buf) })
            }
            State::DoubleAsterisk(mut re_buf) => {
                write!(&mut re_buf, r".*").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_buf) })
            }
            State::DoubleAsteriskSlash(re_buf) => {
                Ok(unsafe { String::from_utf8_unchecked(re_buf) })
            }
            State::Slash(mut re_buf) => {
                // Pattern `app/` should match
                write!(&mut re_buf, r"/").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_buf) })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Pattern, PatternError};

    #[test]
    fn test_compile() {
        let test_case = [
            (r"", Err(PatternError::Empty)),
            (r"/foo", Ok(r"\Afoo(?:/|\z)")),
            (r"*", Ok(r"(?:\A|/)")),
            (r"**", Ok(r"(?:\A|/).*")),
            (r"***", Ok(r"(?:\A|/).*")), // redundant asterisk
            (r"*.js", Ok(r"(?:\A|/)[^/]*\.js(?:/|\z)")),
            (r"/build/logs", Ok(r"\Abuild/logs(?:/|\z)")),
            (r"docs/*", Ok(r"(?:\A|/)docs/[^/]*\z")),
            (r"apps/", Ok(r"(?:\A|/)apps/")),
            (r"apps//", Ok(r"(?:\A|/)apps/")), // redundant slash
            (r"apps//a", Ok(r"(?:\A|/)apps/a(?:/|\z)")), // redundant slash
            (r"**/logs", Ok(r"(?:\A|/)(?:[^/]+/)*logs(?:/|\z)")),
            (r"a/**/b", Ok(r"(?:\A|/)a/(?:[^/]+/)*b(?:/|\z)")),
        ];

        for (idx, (input, want)) in test_case.into_iter().enumerate() {
            let got = Pattern::compile(input);
            match (got, want) {
                (Ok(pat_got), Ok(pat_want)) => {
                    assert_eq!(
                        pat_got,
                        pat_want.to_string(),
                        "#{}: wants {} but got {}",
                        idx,
                        pat_want,
                        pat_got
                    );
                }
                (Err(e_got), Err(e_want)) => {
                    assert_eq!(
                        e_got, e_want,
                        "#{}: wants {} but got {}",
                        idx, e_want, e_got
                    );
                }
                _ => {
                    unreachable!("#{}: didn't match Result", idx);
                }
            }
        }
    }

    #[test]
    fn test_match() {
        let test_case = [
            (r"*", "foo", true),
            (r"*", "foo/bar", true),
            (r"*", "foo/bar/baz", true),
            (r"/foo", "foo", true),
            (r"/foo", "a/foo", false),
            (r"/foo", "fooa", false),
            (r"**/foo", "foo", true),
            (r"**/foo", "fooa", false),
            (r"**/foo", "bar/foo", true),
            (r"**/foo", "baz/bar/foo", true),
            (r"**/foo", "baz/bar/fooa", false),
            (r"**/foo", "baz/bar/foo/a", true), // RLY!?
            (r"a/**/b", "a/b", true),
            (r"a/**/b", "a/foo/b", true),
            (r"a/**/b", "a/foo/bar/b", true),
            (r"*.js", "foo.js", true),
            (r"*.js", "bar/foo.js", true),
            (r"*.js", "foo.jsx", false),
            // cases below taken from github doc:
            // https://docs.github.com/ja/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners#example-of-a-codeowners-file
            (r"docs/*", "docs/getting-started.md", true),
            (r"docs/*", "docs/build-app/troubleshooting.md", false),
            (r"**/logs", "build/logs", true),
            (r"**/logs", "scripts/logs", true),
            (r"**/logs", "deeply/nested/logs", true),
            (r"??/?", "ab/c", true),
            (r"??/?", "abc/d", false),
            (r"??/?", "ab/cd", false),
            (r"a*", "a", true),
            (r"a*", "ab", true),
            (r"a*", "abc", true),
            (r"a*", "abc/d", false),
            (r"foo/", "foo", false),
            (r"foo/", "foo/a", true),
            (r"foo/", "foo/a/b", true),
            (r"foo//", "foo", false),
            (r"foo//", "foo/a", true),
            (r"foo//", "foo/a/b", true),
            (r"*?z", "z", false),
            (r"*?z", "az", true),
            (r"*?z", "az/a", true),
            (r"*/", "a", false),
            (r"*/", "a/b", true),
            (r"**?", "a", true),
            (r"**?", "ab", true),
            (r"**?", "abc", true),
            (r"**?", "a/a", true),
            (r"**?", "a/ab", true),
            (r"**?", "a/abc", true),
            (r"**?", "a/b/a", true),
            (r"**?", "a/b/ab", true),
            (r"**?", "a/b/abc", true),
            (r"**z", "z", true),
            (r"**z", "az", true),
            (r"**z", "abz", true),
            (r"**z", "a/z", true),
            (r"**z", "a/az", true),
            (r"**z", "a/abz", true),
            (r"**z", "a/b/z", true),
            (r"**z", "a/b/az", true),
            (r"**z", "a/b/abz", true),
            // These cases are ok?: a leading "**" followed by only one "/".
            // The gitignore doc says: that means match in all directories.
            // https://git-scm.com/docs/gitignore#_pattern_format
            //
            // Root directory is also directory so every file will match??
            // I think it never reasonable just leaving "**/" to CODEOWNERS in real world...
            (r"**/", "a", true),
            (r"**/", "a/b", true),
            (r"**/", "a/b/c", true),
            (r"**//", "a", true),
            (r"**//", "a/b", true),
            (r"**//", "a/b/c", true),
            (r"**//z", "z", true),
            (r"**//z", "a/z", true),
            (r"**//z", "a/b/z", true),
            (r"**/*", "a", true),
            (r"**/*", "a/b", true),
            (r"**/*", "a/b/c", true),
            (r"**/*z", "az", true),
            (r"**/*z", "a/bz", true),
            (r"**/*z", "a/b/cz", true),
            (r"**/?", "a", true),
            (r"**/?", "ax/b", true),
            (r"**/?", "ax/bx/c", true),
            (r"**/?", "ax", false),
            (r"/**/?", "ax/bx", false),
            (r"/**/?", "ax/bx/cx", false),
            (r"**/?z", "az", true),
            (r"**/?z", "a/bz", true),
            (r"**/?z", "a/b/cz", true),
            (r"**/?z", "aaz", false),
            (r"**/?z", "a/bbz", false),
            (r"**/?z", "a/b/ccz", false),
        ];

        for (idx, (pat_s, path, want)) in test_case.into_iter().enumerate() {
            let pat = Pattern::new(pat_s.to_string());
            assert!(
                pat.is_ok(),
                "#{}: wanted Ok but got {:?} for given pat:{:?} path:{:?}",
                idx,
                pat,
                pat_s,
                path
            );

            let pat = pat.unwrap();
            let got = pat.is_match(path);
            assert_eq!(
                want, got,
                "#{}: wanted {} but got {}; pat = {:?} for given pat:{:?} path:{:?}",
                idx, want, got, pat, pat_s, path
            );
        }
    }
}
