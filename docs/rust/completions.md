# Completions

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

This generates script methods and wires the runtime protocol into `parse()`:

```rust
// the script a user installs into their shell
pub fn completion_script(shell: usage::complete::Shell) -> String;

// register an alias while still invoking this binary for answers
pub fn completion_script_for_alias(alias: &str, shell: usage::complete::Shell) -> String;

// where that script goes, and where it went — see "Installing the script" below
pub fn completion_install_plan(shell, env) -> Result<Plan, install::Error>;
pub fn install_completion(shell, env, on_foreign) -> Result<Installed, install::Error>;

// and the same pair for an alias
pub fn completion_install_plan_for_alias(alias, shell, env) -> Result<Plan, install::Error>;
pub fn install_completion_for_alias(alias, shell, env, on_foreign) -> Result<Installed, …>;

// answer a runtime completion request, if argv is one
pub fn completion_request(argv: &[OsString]) -> Option<String>;
```

`Shell` covers `Bash`, `Zsh`, `Fish`, `Nu`, and `PowerShell`.

For clap parity, bash, fish, PowerShell, and zsh are the covered set. Nushell is an additional
usage-native target. Elvish is not supported, so a clap application that currently publishes an
Elvish script must keep that generator or defer the migration of that artifact.

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
    shell: String,
}

// in your run function:
let shell = usage::complete::Shell::from_name(&completion.shell)
    .expect("a supported shell name");
print!("{}", Ex::completion_script(shell));
```

Shell aliases are explicit because each shell stores and expands them differently. To complete
`m` exactly like `mise`, install `Ex::completion_script_for_alias("m", shell)`. The generated
script registers `m`, but its callback executes `mise`; it does not depend on alias expansion in
the completion subprocess. Embedders can make the same distinction with
`usage::script::script_for(real_binary, registered_name, shell)`.

Candidates come from the same tables the parser uses: subcommands and their visible aliases,
flags in scope at the cursor (globals included, hidden entries excluded), `choices` and
`ValueEnum` words for a pending value, and negation spellings.

## Installing the script

A script your user still has to redirect by hand is only half of shipping one, so
`#[usage(completion)]` also generates the pair that puts it where the shell will look:

```rust
use usage::install::{Env, OnForeign};

// Where it would go, and what else the user must do. Touches no filesystem.
let plan = Ex::completion_install_plan(shell, &Env::from_process())?;

// The same thing, written.
let done = Ex::install_completion(shell, &Env::from_process(), OnForeign::Refuse)?;
println!("installed to {}", done.plan.path.display());
if let Some(line) = done.plan.loading.instruction() {
    println!("add this to your shell's startup file, once:\n{line}");
}
```

`Env` is the environment _described_ rather than read at the point of use, the way
`usage-config` describes one: `Env::from_process()` is what a CLI passes, while a test builds
`Env::new(Platform::Linux, …)` and asks where a script would go on a machine it is not running
on. `plan()` is also the whole of a `--dry-run` — there is no flag for one, because a plan is
what a preview prints.

Where each shell keeps a user's own scripts, and whether it finds one without being told:

| Shell      | Directory                                                                                     | Loads by itself            |
| ---------- | --------------------------------------------------------------------------------------------- | -------------------------- |
| bash       | `$BASH_COMPLETION_USER_DIR/completions`, else `$XDG_DATA_HOME/bash-completion/completions`    | yes, via bash-completion   |
| fish       | `$XDG_CONFIG_HOME/fish/completions`                                                           | yes                        |
| nushell    | `$NU_VENDOR_AUTOLOAD_DIR`, else the nushell config directory                                  | only in a vendor directory |
| zsh        | `$XDG_DATA_HOME/zsh/site-functions`, as `_<name>`                                             | no — needs `fpath+=`       |
| PowerShell | `$XDG_CONFIG_HOME/powershell/completions`, `%LOCALAPPDATA%\PowerShell\completions` on Windows | no — needs dot-sourcing    |

Where the answer is no, `Loading::Manual` carries the exact line and the file it belongs in.
Printing it is the caller's job.

A file already at the target is read before anything is written, which is what separates an
upgrade from a theft. Identical bytes are `Wrote::Unchanged` and nothing is written at all; a
file carrying any `@generated by usage` stamp is `Wrote::Updated` — the family, so a script
`usage g completion --install` wrote counts as much as one a binary wrote for itself; anything
else is `Error::Foreign` naming the path, unless the caller passes `OnForeign::Overwrite`. So
re-running an install after an upgrade needs no flag, while a script somebody wrote by hand
survives one.

**What installing never does**, on purpose:

- **No startup file is edited.** Not `.zshrc`, not `$PROFILE`. Writing the script again is a
  no-op, so an upgrade can re-run an install as often as it likes; appending a line to `.zshrc`
  again is not, and a tool that owns a user's dotfiles has no undo to offer.
- **No shell detection.** You name the shell. `$SHELL` is the login shell, not necessarily the
  one running, and a guess made here would be a guess owned here.

## Completing values

Three ways to say what a value can be:

```rust
// a fixed set of words
#[usage(long, choices("json", "table"))]
format: Option<String>,

// paths — the shell's native file completion takes over
#[usage(long, value_hint = usage::ValueHint::FilePath)]
file: Option<PathBuf>,

// filtered paths — directories remain available for traversal
#[usage(
    long,
    value_hint = usage::ValueHint::FilePath,
    extensions("toml", "yaml")
)]
manifest: Option<PathBuf>,

// anything you can compute
#[usage(arg, name = "TASK", complete = tasks_in_file)]
task: Option<String>,
```

`ValueHint` carries clap's full stable vocabulary. Path, executable, and command hints delegate
to the shell; username and hostname hints use system candidates; `Other`, URL, and email values
suppress the shell's misleading filename fallback. Every hint emits a portable `complete`
type into KDL so external consumers preserve the same policy.

A custom completer is a plain function, referenced by _path_ — a typo is a compile error, not a
silent dead completer:

```rust
fn tasks_in_file(
    partial: &<Tasks as usage::spec::CommandArgs>::Partial,
    _ctx: &usage::complete::CompleteCtx<'_>,
) -> Vec<usage::complete::Candidate<'static>> {
    // Partial string fields hold the bytes as typed — a word that is not valid UTF-8 is
    // still a word somebody wrote.
    let file = partial
        .file
        .as_deref()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    let file = file.as_deref().unwrap_or("tasks.toml");
    read_tasks(file)
        .map(|t| usage::complete::Candidate::described(t.name, t.about))
        .collect()
}
```

The first parameter is the _partial parse_ of the completer's own command — flags the user has
already typed are available, so a `--file` flag can steer what gets completed. Build candidates
with `Candidate::new(value)` or `Candidate::described(value, description)`; shells that display
descriptions show them, shells that don't get the value alone. Chain `.displayed(label)` when a
short insertion needs a more explanatory presentation; zsh and PowerShell keep that label
separate from the text inserted into the command line, while other shells display the value.
Chain `.with_kind(CandidateKind::Command)` (or `Flag`, `File`, or `Directory`) when a runtime
candidate has a more specific role. PowerShell maps it to the corresponding native completion
result type; shells without typed candidates keep the same value and description.

## Tracing an answer

Completion decisions are available as structured diagnostic data. This uses the same split,
parser walk, and completion tables as the runtime request:

```rust
let line = "ex build --out ";
let split = usage::complete::split(line, line.len(), usage::complete::Shell::Zsh);
let trace = usage::complete::trace(Ex::spec(), &split);

assert_eq!(trace.awaiting_value, Some("out"));
eprintln!("{trace}");
```

The trace includes the shell-split words and prefix, selected command path, cursor owner, flag and
separator state, candidates, and native shell fallback. Applications can expose its `Display`
form from a diagnostic command or render the public fields themselves.
