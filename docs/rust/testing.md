# Testing

::: warning Draft
This page is a draft and has not yet been human reviewed. Details may change.
:::

A CLI test can run the compiled program and assert on stdout, stderr, and its exit status.
Lower-level helpers assert what argv parses to, what a user reads when it does not, and what a
shell offers while one is being typed.

The third has no clap equivalent at all: testing a `clap_complete` script means golden-filing
the script's text or driving a real shell, neither of which asserts what a user is actually
offered. Here a completion is a function of the spec and a half-typed line, so it is a plain
assertion.

```toml
[dev-dependencies]
usage = { package = "usage-rs", version = "6", features = ["test"] }
```

The feature belongs in `dev-dependencies`: nothing in an application's own code calls it.

Nothing here formats a page or a message of its own. Every page comes from the same function
`parse()` renders a help request with, and every failure from the same one it prints — a harness
that renders its own approximation is a harness whose passing tests mean nothing.

## What a command writes

`command!` runs one of the package's binary targets and captures its output:

```rust
// tests/cli.rs
#[test]
fn hello_greets_by_name() {
    let output = usage::test::command!("greet", "hello", "Jeff").assert_success();

    assert_eq!(output.stdout_text(), "hello, Jeff\n");
    assert_eq!(output.stderr_text(), "");
}
```

The test must be under `tests/`. Cargo provides the compiled path as
`CARGO_BIN_EXE_<name>` to integration tests; the macro uses that path instead of assuming where
`target/` lives. The raw `stdout` and `stderr` byte vectors remain available for commands that
intentionally write non-UTF-8 data.

## What a command line does

`outcome` gives back what `parse()` would have _done_, as a value: a struct, a page, a version,
or a failure.

```rust
use usage::test::{self as harness, Outcome};

#[test]
fn a_command_line_parses() {
    let words = harness::argv(["-j", "4", "build", "release"]);
    let cli = harness::parse(Ex::spec(), &words.words(), Ex::parse_from).unwrap();

    assert_eq!(cli.jobs, Some(4));
}
```

`argv` owns the words, because a parse entry point borrows them. `Ex::parse_from` is passed as a
function item — the harness calls it — so the CLI's own generated parser is what runs.

`parse` is the two-way version: the struct, or the text a user would have read instead.

```rust
#[test]
fn a_bad_value_says_what_was_wrong_with_it() {
    let words = harness::argv(["--jobs", "many"]);
    let message = harness::parse(Ex::spec(), &words.words(), Ex::parse_from).unwrap_err();

    assert!(message.contains("invalid value 'many'"));
}
```

That string is the rendered diagnostic, not a debug-printed error code — the same bytes the
process writes to stderr. Which means the message your users read is a thing your test suite can
hold still.

When the difference between a failure and a question matters, `outcome` keeps them apart, along
with the stream and exit status each would have used:

```rust
#[test]
fn an_empty_command_line_shows_help() {
    let words = harness::argv([] as [&str; 0]);

    let Outcome::Help(printed) = harness::outcome(Bare::spec(), &words.words(), Bare::parse_from)
    else {
        panic!("arg_required_else_help shows help");
    };
    assert!(printed.stderr); // help nobody asked for is not stdout's business
    assert_eq!(printed.code, 2);
}
```

| Outcome   | What it is                                                              |
| --------- | ----------------------------------------------------------------------- |
| `Parsed`  | the struct                                                              |
| `Help`    | a page — asked for on stdout, or shown on stderr with a non-zero status |
| `Version` | `-V` / `--version`                                                      |
| `Failed`  | the rendered diagnostic, on stderr, with clap's status                  |

## Help, page by page

`help` renders one command's page, found by the words a user would type. `&[]` is the root, and
aliases work, because a test should be able to ask the way a user would.

```rust
use usage::test::Page;

assert!(harness::help(Ex::spec(), &["build"], Page::Long).contains("--out"));
```

A path that names no command panics and lists what the parent does have — a test asking about a
command that has since been renamed should say so, not quietly assert about a different page.

`help_tree` is the drift test: every command in the tree, depth-first, in one string.

```rust
#[test]
fn help_has_not_drifted() {
    insta::assert_snapshot!(harness::help_tree(Ex::spec(), Page::Long));
}
```

```text
=== ex ===
A tool that does things
...
=== ex build ===
Build the thing
...
=== ex secret (hidden) ===
```

Any change to any page — a flag's help, a new subcommand, a heading that moved — is one diff in
one file. Hidden commands are included and marked: a hidden command still has a page that can
regress.

## What a shell would offer

Completion assertions need both features — `test` for the harness, `completions` for the runtime
that answers a line:

```toml
[dev-dependencies]
usage = { package = "usage-rs", version = "6", features = ["test", "completions"] }
```

`candidates` then answers the question a shell asks: given this half-typed line, what could this
word be? The line includes the program name, exactly as a shell passes it.

```rust
assert_eq!(harness::candidates(Ex::spec(), "ex bui"), ["build"]);
```

`described` adds the text a shell shows beside each candidate, and `completion` returns the whole
answer — including whether the position admits paths, which is how a test says that `<PATH>`
offers files and `--jobs` does not.

```rust
let answer = harness::completion(Ex::spec(), "ex build --out ");
assert_eq!(answer.files, Some(usage::test::Files::Any));
```

`completion_at` takes the cursor as a byte offset and the shell, which is what makes a test about
completing in the _middle_ of a command line possible at all:

```rust
use usage::test::Shell;

let answer = harness::completion_at(Ex::spec(), "ex bui release", "ex bui".len(), Shell::Bash);
```

## The one-line spec test

Structural checks on the declaration itself are not in this module. They are in `to_kdl`, which
asserts in debug builds that the tree is coherent — no duplicate keys, no duplicate flag
spellings across a `flatten` boundary, no argument no word can reach. The one-line test that
fires them is on the [Spec Output](/rust/spec#round-trip-guarantee) page, and is worth writing
beside these; it parses the emitted KDL, so it needs `usage-lib` as a dev-dependency of its
own.

## What is not covered

- **Executable views.** These entry points are the plain ones; a view is selected by argv0 in
  `parse_from_argv`.
- **A runtime identity.** A CLI whose version or name is computed at run time reports its
  declared spec values here, since the harness has the spec and not the program.
- **A build without `diagnostics`.** The failure text is whatever that build would print,
  which without the renderer is the compact error rather than the clap-shaped message. The
  facade's defaults include it.
- **Colour.** Pages and messages come back plain. A snapshot with escape sequences in it is a
  snapshot nobody can read; colour is asserted in usage's own suite, not in yours.
