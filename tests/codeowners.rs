mod support;

use git_toolbox::github::codeowners::{CodeOwners, CodeOwnersError};
use support::{git_add, git_init, mkdir_p, test_logger, write};
use tempfile::TempDir;

#[test]
fn codeowner_try_from_repo_fails_when_codeowners_file_is_not_indexed() {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    mkdir_p(root.join(".github"));
    write(
        root.join(".github/CODEOWNERS"),
        "\
# comment line
*.js frontend-developer # comment
baz/ baz-owner

"
        .as_bytes(),
    );

    assert!(matches!(
        CodeOwners::try_from_repo(&repo),
        Err(CodeOwnersError::NotIndexed)
    ));
}

#[test]
fn codeowner_try_from_repo_does_not_generate_warnings_for_lines_without_pattern() {
    let logger = test_logger();
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    mkdir_p(root.join(".github"));
    write(
        root.join(".github/CODEOWNERS"),
        "\
# comment line
*.js frontend-developer # comment
baz/ baz-owner

"
        .as_bytes(),
    );

    git_add(&repo, ".github/CODEOWNERS");

    let co = CodeOwners::try_from_repo(&repo).unwrap();
    assert_eq!(logger.take(), vec![]);

    assert_eq!(
        co.find_owners("foo/bar.js"),
        Some(&vec![String::from("frontend-developer")])
    );
}
