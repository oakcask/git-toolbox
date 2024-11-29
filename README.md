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

### git-dah

An alternative of git-push, knows what you want to do.
"Dah" stands for "Fus Ro Dah" (means "force, blance, push")
which comes from unrelenting force shout (thu'um) in Skyrim.

People who are most likely to be interested in this command, are like:

* Have shared repository (like GitHub repo)
* Never push the default branch because it is always updated by
  other process or mechanism (like Pull Requests)
* Always search in the shell history for complex git commands:
  branch, switch, pull, push, blah blah blah...

#### Synopsis

```
Push local changes anyway -- I know what you mean

Usage: git-dah [OPTIONS]

Options:
  -1, --step           Do stepwise execution
      --limit <LIMIT>  Increase number of commits to scan in history [default: 100]
  -h, --help           Print help
```

#### Configuration

git-dah never push the default branch or pre-configured protected branch.
git-dah guesses the name of default branch by checking `init.defaultbranch`[^1] configuration.

Or, and also, you can have extra branches which git-dah respects them as protected, by setting `dah.protectedbranch`.
This is glob patterns separated by `:`.
For example, to protect "develop", "release", and branchs starts with "release/" for all repository on your computer,
execute git-config like below:

```sh
git config --global dah.protectedbranch "develop:release:release/*"
```

#### Behavior

git-dah will automatically and repeatedly invoke git commands until stop in following rule:

* Stop if working tree is conflicted or HEAD and its remote tracking branch is synchronized.
* Stage changes by `git add -u` if working tree is "dirty".
* Commit changes if staged changes exist.
* Rename branch then switch to it, if HEAD points to the defualt or protected branch.
  This will clean up the revisions "wrongly" commited on the default or protected branches.
* Create branch then switch to it, if HEAD is detached.
* Rebase with `git pull --rebase` if HEAD branch is diverged from its remote tracking branch.
* Push with `git push --force-with-lease --force-if-includes -u origin <HEAD BRANCH>` then stop,
  if HEAD branch is ahead of the remote tracking branch.

Enabling stepwise exection (by `--step` option), git-dah will stop after invoking just one command for cautious user.

[^1]: https://git-scm.com/docs/git-init#Documentation/git-init.txt-code--initial-branchcodeemltbranch-namegtem


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
