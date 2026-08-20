# Completions

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

Completion support is opt-in: add `completion` to the root attribute and enable the
`completions` cargo feature (forgetting the feature is a compile error that names it):

```toml
[dependencies]
usage = { package = "usage-rs", version = "6", features = ["completions"] }
```

```rust
#[derive(Cli)]
#[usage(bin = "ex", completion)]
struct Ex { /* … */ }
```

This generates two methods and wires the runtime protocol into `parse()`:

```rust
// the script a user installs into their shell
pub fn completion_script(shell: usage::complete::Shell) -> String;

// answer a runtime completion request, if argv is one
pub fn completion_request(argv: &[OsString]) -> Option<String>;
```

`Shell` covers `Bash`, `Zsh`, `Fish`, `Nu`, and `PowerShell`.

## How it works

The installed script calls your binary back at completion time with a hidden
`__complete_word__` request describing the line and cursor. The request is recognized _before_
any parsing, so it never appears in your grammar, help, or spec. `parse()` intercepts it
automatically; with `parse_from`, call `completion_request` first and print whatever it returns.

A typical way to expose the scripts:

```rust
#[derive(Args)]
struct Completion {
    /// Which shell to generate for
    #[usage(long, value_enum)]
    shell: Shell,
}

// in your run function:
print!("{}", Ex::completion_script(cli.completion.shell.into()));
```

Candidates come from the same tables the parser uses: subcommands and their visible aliases,
flags in scope at the cursor (globals included, hidden entries excluded), `choices` and
`ValueEnum` words for a pending value, and negation spellings.

## Completing values

Three ways to say what a value can be:

```rust
// a fixed set of words
#[usage(long, choices("json", "table"))]
format: Option<String>,

// paths — the shell's native file completion takes over
#[usage(long, value_hint = usage::ValueHint::FilePath)]
file: Option<PathBuf>,

// anything you can compute
#[usage(arg, name = "TASK", complete = tasks_in_file)]
task: Option<String>,
```

`ValueHint` (`FilePath`, `DirPath`, `AnyPath`) answers with the shell's own file/directory
completion and also emits `complete "file" type="path"` into the KDL, so external consumers of
the spec give the same answer.

A custom completer is a plain function, referenced by _path_ — a typo is a compile error, not a
silent dead completer:

```rust
fn tasks_in_file(
    partial: &<Tasks as usage::spec::CommandArgs>::Partial,
    _ctx: &usage::complete::CompleteCtx<'_>,
) -> Vec<usage::complete::Candidate<'static>> {
    let file = partial.file.as_deref().unwrap_or("tasks.toml");
    read_tasks(file)
        .map(|t| usage::complete::Candidate::described(t.name, t.about))
        .collect()
}
```

The first parameter is the _partial parse_ of the completer's own command — flags the user has
already typed are available, so a `--file` flag can steer what gets completed. Build candidates
with `Candidate::new(value)` or `Candidate::described(value, description)`; shells that display
descriptions (zsh, fish) show them, shells that don't get the value alone.
