mod support;

use git_toolbox::github::codeowners::{CodeOwners, CodeOwnersError};
use rstest::rstest;
use support::{git_add, git_init, mkdir_p, test_logger, write};
use tempfile::TempDir;

#[rstest]
#[case(".github/CODEOWNERS")]
#[case("CODEOWNERS")]
#[case("docs/CODEOWNERS")]
fn codeowner_try_from_repo_fails_when_codeowners_file_is_not_indexed_(#[case] path: &str) {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    let co_path = root.join(path);
    mkdir_p(co_path.parent().unwrap());
    write(
        co_path,
        "\
# comment line
*.js frontend-developer # comment
baz/ baz-owner

"
        .as_bytes(),
    );

    assert!(matches!(
        CodeOwners::<()>::try_from_repo(&repo),
        Err(CodeOwnersError::NotIndexed)
    ));
}

#[rstest]
#[case(".github/CODEOWNERS")]
#[case("CODEOWNERS")]
#[case("docs/CODEOWNERS")]
fn codeowner_try_from_repo_does_not_generate_warnings_for_lines_without_pattern(
    #[case] path: &str,
) {
    let logger = test_logger();
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    let co_path = root.join(path);
    mkdir_p(co_path.parent().unwrap());
    write(
        co_path,
        "\
# comment line
*.js frontend-developer # comment
baz/ baz-owner

"
        .as_bytes(),
    );

    git_add(&repo, path);

    let co = CodeOwners::<()>::try_from_repo(&repo).unwrap();
    assert_eq!(logger.take(), vec![]);

    assert_eq!(
        co.find_owners("foo/bar.js"),
        Some(&vec![String::from("frontend-developer")])
    );
}

#[rstest]
#[case(".github/CODEOWNERS", "CODEOWNERS")]
#[case("CODEOWNERS", "docs/CODEOWNERS")]
fn codeowner_try_from_repo_find_codeowners_file_in_priority(
    #[case] prior: &str,
    #[case] after: &str,
) {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    let co1_path = root.join(prior);
    let co2_path = root.join(after);
    mkdir_p(co1_path.parent().unwrap());
    mkdir_p(co2_path.parent().unwrap());
    write(
        co1_path,
        "\
*.js owner-1
"
        .as_bytes(),
    );
    write(
        co2_path,
        "\
*.js owner-2
"
        .as_bytes(),
    );
    git_add(&repo, prior);
    git_add(&repo, after);

    let co = CodeOwners::<()>::try_from_repo(&repo).unwrap();
    assert_eq!(co.find_owners("a.js"), Some(&vec![String::from("owner-1")]));
}
