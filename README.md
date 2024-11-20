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

```
Push local changes anyway

Usage: git-dah [OPTIONS]

Options:
  -1, --step           do stepwise execution
      --limit <LIMIT>  limit number of commits to scan in history [default: 100]
  -h, --help           Print help
```

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
