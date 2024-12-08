# git-toolbox: shorthands for git operation

## Installation

If you have `cargo`, run following command:

```
cargo install --git https://github.com/oakcask/git-toolbox.git
```

Or, download pre-built binary:

### Linux x86-64

```
curl -sSL https://github.com/oakcask/git-toolbox/releases/latest/download/x86_64-unknown-linux-gnu.tar.gz | tar zx -C /path/to/bin
```

### Linux ARM64

```
curl -sSL https://github.com/oakcask/git-toolbox/releases/latest/download/aarch64-unknown-linux-gnu.tar.gz | tar zx -C /path/to/bin
```

### MacOS (Apple Silicon)

```
curl -sSL https://github.com/oakcask/git-toolbox/releases/latest/download/aarch64-apple-darwin.tar.gz | tar zx -C /path/to/bin
```

## Usage

### git-stale

```
List or delete stale branches

Usage: git-stale [OPTIONS] [BRANCHES]...

Arguments:
  [BRANCHES]...  Select branches with specified prefixes, or select all if unset

Options:
  -d, --delete         Perform deletion of selected branches
      --push           Combined with --delete, perform deletion on remote repository instead
      --since <SINCE>  Select local branch with commit times older than the specified relative time
  -h, --help           Print help
```

### git-whose

```
find GitHub CODEOWNERS for path(s)

Usage: git-whose [PATHSPECS]...

Arguments:
  [PATHSPECS]...  

Options:
  -h, --help  Print help
```

git-whose is a support tool to improve usability of GitHub CODEOWNERS[^1];
which searches over git index and lists owner(s) specified in `.github/CODEOWNERS` for given files where pathspecs[^2] match.
Output will be list of pairs consisted of the file path and its code owners.

Note that only committed and/or staged files are listed.
Becaue git-whose only searches in git index, as described above.
So, maybe it is inconvinient, git-whose requires `.github/CODEOWNERS` and all other files to be commited or staged,
but this enables us to search large repository (like monorepo) faster, and to search over bare repository and sparse tree.

#### Pathspecs parameter

In non-bare repository for most use cases, relative paths can be passed as pathspecs parameters.
The paths will be normalized into relative paths from repository root (parent repository to `.git/`) based on
the current working directory and repository root location.
Paths cannot point the outside of repository.

```sh
git init . # assume $PWD is repository root.
mkdir -p foo/bar .github
touch foo/baz foo/bar/.keep .github/CODEOWNERS
git add .

# those are equivalent:
git whose foo/baz
(cd foo && git whose baz)
(cd foo/bar && git whose ../baz)

# ERROR: because path points outside of repo.
git whose ../outside
```

In other case, for bare repository, pathspecs are interpreted as-is.

[^1]: https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners
[^2]: https://git-scm.com/docs/gitglossary#Documentation/gitglossary.txt-aiddefpathspecapathspec

### git-dah

An alternative of git-push, knows what you want to do.
"Dah" stands for "Fus Ro Dah" (means "force, blance, push")
which comes from unrelenting force shout (thu'um) in Skyrim.

People who are most likely to be interested in this command, are like:

* Have shared repository (like GitHub repo)
* Never push the default branch because it is always updated by
  other process or mechanism (like Pull Requests)
* Usually work on monorepo and busy pushing multiple branches around, for many Pull Requests; morning you change `monorepo/foo`, then after noon `monorepo/bar`... Phew!
* Push on topic branch triggers deployment or test. And almost all the time you want it when you push.

git-dah will change your workflow to:

* switch to default branch
* (skip `git switch -c`)
* write some code...
* (skip `git add -u`)
* (skip `git commit`)
* (skip `git push`)
* `git dah`
* write some code...
* `git dah`
* go back to start when you have to do other thing

#### Synopsis

```
Push local changes anyway -- I know what you mean

Usage: git-dah [OPTIONS]

Options:
  -1, --step           Do stepwise execution
      --limit <LIMIT>  Increase number of commits to scan in history [default: 100]
      --cooperative    Extra safety for team programming; meaning always rebase HEAD onto the remote branch and don't push with force [aliases: no-force]
  -h, --help           Print help
```

git-dah will automatically and repeatedly invoke git commands until stop in following rule:

* Stop if working tree is conflicted or HEAD and its remote tracking branch is synchronized.
* Stage changes by `git add -u` if working tree is "dirty".
* Commit changes if staged changes exist.
* Rename branch then switch to it, if HEAD points to the defualt or protected branch.
  This will clean up the revisions "wrongly" commited on the default or protected branches.
* Create branch then switch to it, if HEAD is detached.
* Rebase with `git pull --rebase` if HEAD branch is diverged from its remote tracking branch.
  * Without `--cooperative` option, this step is skipped if HEAD's reflog includes the commit on the top of the remote tracking branch.
* Push with `git push --force-with-lease --force-if-includes -u origin <HEAD BRANCH>` then stop,
  if HEAD branch is ahead of the remote tracking branch.
  * With `--cooperative` option, `--force-*` options are omited.

Enabling stepwise exection (by `--step` option), git-dah will stop after invoking just one command for cautious user.

#### Configuration

##### Disable push of default or protected branch

git-dah never push the default branch or pre-configured protected branch.
git-dah guesses the name of default branch by checking `init.defaultbranch`[^3] configuration.

Or, and also, you can have extra branches which git-dah respects them as protected, by setting `dah.protectedbranch`.
This is glob patterns separated by `:`.
For example, to protect "develop", "release", and branchs starts with "release/" for all repository on your computer,
execute git-config like below:

```sh
git config --global dah.protectedbranch "develop:release:release/*"
```

[^3]: https://git-scm.com/docs/git-init#Documentation/git-init.txt-code--initial-branchcodeemltbranch-namegtem

##### Add prefix to auto-created branch

Branch name created by git-dah is based on the first line of HEAD commit message and
random number (ULID).

If you want to force git-dah to use prefered prefix for branch name, set `dah.branchprefix` like below:

```sh
git config --global dah.branchprefix feature/
```

In this case, git-dah will generate branch name like `feature/add-something-dah01je3k586pjjq4e5hxb13cwysp`.

### Relative Date Format

Some option in `git-stale` accepts relative date.

- "1mo 2days" will be 1 month and 2 days.
- "3y4w" will be 3 years and 1 month.
- 4 weeks will be rounded up to 1 month.
- each years will be interpreted as 12 months.
- if invalid date is pointed by moving around between months, the last day of month will be used instead:
  - "1mo" before 31st of March will be 28th (or 29th for leap year) of February.

Syntax in BNF is roughly described as below:

```
<period> ::= [<digits> <year-suffix>] [<digits> <month-suffix>] [<digits> <week-suffix>] [<digits> <day-suffix>]
<year-suffix> ::= "y" | "yr" | "yrs" | "year" | "years"
<month-suffix> ::= "mo" | "month" | "months"
<week-suffix> ::= "w" | "week" | "weeks"
<day-suffix> ::= "d" | "day" | "days"
```
