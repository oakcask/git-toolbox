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
        // re_out is a buffer where to output "compiled" pattern.
        enum State {
            Head {
                re_out: Vec<u8>,
            },
            HeadAsterisk {
                re_out: Vec<u8>,
            },
            // must_escape is temporary storage to buffer a part of literal string
            // taken from pattern. this must be escaped before it concatinated to re_out.
            Default {
                re_out: Vec<u8>,
                must_escape: Vec<u8>,
            },
            Asterisk {
                re_out: Vec<u8>,
            },
            DoubleAsterisk {
                re_out: Vec<u8>,
            },
            DoubleAsteriskSlash {
                re_out: Vec<u8>,
            },
            Slash {
                re_out: Vec<u8>,
            },
        }
        let state = pattern
            .chars()
            .fold(State::Head { re_out: Vec::new() }, |st, c| match st {
                State::Head { mut re_out } => {
                    if c == '/' {
                        write!(&mut re_out, r"\A").unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else if c == '*' {
                        write!(&mut re_out, r"(?:\A|/)").unwrap();
                        State::HeadAsterisk { re_out }
                    } else if c == '?' {
                        write!(&mut re_out, r"(?:\A|/)[^/]").unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else {
                        let mut must_escape = Vec::new();
                        write!(&mut re_out, r"(?:\A|/)").unwrap();
                        write!(&mut must_escape, "{c}").unwrap();
                        State::Default {
                            re_out,
                            must_escape,
                        }
                    }
                }
                State::Default {
                    mut re_out,
                    mut must_escape,
                } => {
                    if c == '/' {
                        let s = unsafe { String::from_utf8_unchecked(must_escape) };
                        write!(&mut re_out, "{}", regex::escape(&s)).unwrap();
                        State::Slash { re_out }
                    } else if c == '*' {
                        let s = unsafe { String::from_utf8_unchecked(must_escape) };
                        write!(&mut re_out, "{}", regex::escape(&s)).unwrap();
                        State::Asterisk { re_out }
                    } else if c == '?' {
                        let s = unsafe { String::from_utf8_unchecked(must_escape) };
                        write!(&mut re_out, r"{}[^/]", regex::escape(&s)).unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else {
                        write!(&mut must_escape, "{c}").unwrap();
                        State::Default {
                            re_out,
                            must_escape,
                        }
                    }
                }
                State::Asterisk { mut re_out } | State::HeadAsterisk { mut re_out } => {
                    if c == '/' {
                        write!(&mut re_out, r"[^/]*").unwrap();
                        State::Slash { re_out }
                    } else if c == '*' {
                        State::DoubleAsterisk { re_out }
                    } else if c == '?' {
                        write!(&mut re_out, r"[^/]+").unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else {
                        let mut must_escape = Vec::new();
                        write!(&mut re_out, r"[^/]*").unwrap();
                        write!(&mut must_escape, "{c}").unwrap();
                        State::Default {
                            re_out,
                            must_escape,
                        }
                    }
                }
                State::DoubleAsterisk { mut re_out } => {
                    if c == '/' {
                        write!(&mut re_out, r"(?:[^/]+/)*").unwrap();
                        State::DoubleAsteriskSlash { re_out }
                    } else if c == '*' {
                        State::DoubleAsterisk { re_out }
                    } else if c == '?' {
                        write!(&mut re_out, r"[^/]+").unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else {
                        let mut must_escape = Vec::new();
                        write!(&mut re_out, r"[^/]*").unwrap();
                        write!(&mut must_escape, "{c}").unwrap();
                        State::Default {
                            re_out,
                            must_escape,
                        }
                    }
                }
                State::DoubleAsteriskSlash { mut re_out } => {
                    if c == '/' {
                        State::DoubleAsteriskSlash { re_out }
                    } else if c == '*' {
                        State::Asterisk { re_out }
                    } else if c == '?' {
                        write!(&mut re_out, r"[^/]").unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else {
                        let mut must_escape = Vec::new();
                        write!(&mut must_escape, "{c}").unwrap();
                        State::Default {
                            re_out,
                            must_escape,
                        }
                    }
                }
                State::Slash { mut re_out } => {
                    if c == '/' {
                        State::Slash { re_out }
                    } else if c == '*' {
                        write!(&mut re_out, r"/").unwrap();
                        State::Asterisk { re_out }
                    } else if c == '?' {
                        write!(&mut re_out, r"/[^/]").unwrap();
                        State::Default {
                            re_out,
                            must_escape: Vec::new(),
                        }
                    } else {
                        let mut must_escape = Vec::new();
                        write!(&mut re_out, r"/").unwrap();
                        write!(&mut must_escape, "{c}").unwrap();
                        State::Default {
                            re_out,
                            must_escape,
                        }
                    }
                }
            });

        match state {
            State::Head { .. } => Err(PatternError::Empty)?,
            State::Default {
                mut re_out,
                must_escape,
            } => {
                // add [/\z] to pattern and path for preventing partial match.
                // Pattern `path/to/foo` should only maches directory or file named `foo` under
                // `path/to` directory. `path/to/foobar` shouldn't match.
                let s = unsafe { String::from_utf8_unchecked(must_escape) };
                write!(&mut re_out, r"{}(?:/|\z)", regex::escape(&s)).unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_out) })
            }
            State::Asterisk { mut re_out } => {
                // trailing asterisk doesn't match further nested path
                write!(&mut re_out, r"[^/]*\z").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_out) })
            }
            State::HeadAsterisk { mut re_out } => {
                // lone single asterisk should match everything
                write!(&mut re_out, r"").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_out) })
            }
            State::DoubleAsterisk { mut re_out } => {
                write!(&mut re_out, r".*").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_out) })
            }
            State::DoubleAsteriskSlash { re_out } => {
                Ok(unsafe { String::from_utf8_unchecked(re_out) })
            }
            State::Slash { mut re_out } => {
                // Pattern `app/` should match
                write!(&mut re_out, r"/").unwrap();
                Ok(unsafe { String::from_utf8_unchecked(re_out) })
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
                        "#{idx}: wants {pat_want} but got {pat_got}"
                    );
                }
                (Err(e_got), Err(e_want)) => {
                    assert_eq!(
                        e_got, e_want,
                        "#{idx}: wants {e_want} but got {e_got}"
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
                "#{idx}: wanted Ok but got {pat:?} for given pat:{pat_s:?} path:{path:?}"
            );

            let pat = pat.unwrap();
            let got = pat.is_match(path);
            assert_eq!(
                want, got,
                "#{idx}: wanted {want} but got {got}; pat = {pat:?} for given pat:{pat_s:?} path:{path:?}"
            );
        }
    }
}
