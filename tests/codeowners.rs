#[path = "common/bare_single.rs"]
mod bare_single;
#[path = "common/git_worktree.rs"]
mod git_worktree;
#[path = "common/log.rs"]
mod test_log;

use bare_single::bare_repo_with_committed_file;
use git_toolbox::github::codeowners::{CodeOwners, CodeOwnersError};
use git_worktree::{git_add, git_command, git_init, git_set_config, mkdir_p, write};
use rstest::rstest;
use tempfile::TempDir;
use test_log::test_logger;

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
        Err(CodeOwnersError::Missing)
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

#[rstest]
#[case(".github/CODEOWNERS")]
#[case("CODEOWNERS")]
#[case("docs/CODEOWNERS")]
fn codeowner_try_from_repo_works_for_bare_repository(#[case] path: &str) {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let content = b"*.rs rust-team\n";
    let repo = bare_repo_with_committed_file(root, path, content);
    assert!(repo.is_bare());

    let co = CodeOwners::<()>::try_from_repo(&repo).unwrap();
    assert_eq!(
        co.find_owners("src/lib.rs"),
        Some(&vec![String::from("rust-team")])
    );
}

#[test]
fn codeowner_try_from_repo_reads_skip_worktree_codeowners_from_index() {
    let tmpdir = TempDir::new().unwrap();
    let root = tmpdir.path();

    let repo = git_init(root);
    git_set_config(&repo, "user.name", "t");
    git_set_config(&repo, "user.email", "t@example.com");
    mkdir_p(root.join(".github"));
    write(root.join(".github/CODEOWNERS"), b"docs/*.md @docs-team\n");
    mkdir_p(root.join("docs"));
    write(root.join("docs/guide.md"), b"# Guide\n");

    git_add(&repo, ".github/CODEOWNERS");
    git_add(&repo, "docs/guide.md");
    git_command(&repo, &["commit", "-m", "init"]);
    git_command(&repo, &["sparse-checkout", "init", "--no-cone"]);
    std::fs::write(root.join(".git/info/sparse-checkout"), b"docs/*\n").unwrap();
    git_command(&repo, &["read-tree", "-mu", "HEAD"]);

    assert!(!root.join(".github/CODEOWNERS").exists());

    let co = CodeOwners::<()>::try_from_repo(&repo).unwrap();
    assert_eq!(
        co.find_owners("docs/guide.md"),
        Some(&vec![String::from("@docs-team")])
    );
}

#[test]
fn codeowner_debug_marks_only_last_match_effective() {
    let co = CodeOwners::<()>::try_from_bufread(
        &b"*.rs @rust-team\nsrc/** @src-team\nsrc/lib.rs @lib-team\n"[..],
    )
    .unwrap();

    let matches = co.debug("src/lib.rs").collect::<Vec<_>>();

    assert_eq!(matches.len(), 3);
    assert!(!matches[0].is_effective());
    assert!(!matches[1].is_effective());
    assert!(matches[2].is_effective());
    assert_eq!(matches[2].owners(), &vec![String::from("@lib-team")]);
}
