# Testing

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

A CLI's observable surface is three things: what a command line parses to, what a user reads
when it does not, and what a shell offers while one is being typed. `usage::test` asserts on all
three from the static tables the derive already emitted — no process to spawn, no terminal to
fake, no snapshot of a binary's stdout.

```toml
[dev-dependencies]
usage = { package = "usage-rs", version = "6", features = ["test"] }
```

The feature belongs in `dev-dependencies`: nothing in an application's own code calls it.

Nothing here formats a page or a message of its own. Every page comes from the same function
`parse()` renders a help request with, and every failure from the same one it prints — a harness
that renders its own approximation is a harness whose passing tests mean nothing.

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

```
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

With the `completions` feature on, `candidates` answers the question a shell asks: given this
half-typed line, what could this word be? The line includes the program name, exactly as a shell
passes it.

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
let answer = harness::completion_at(Ex::spec(), "ex bui release", "ex bui".len(), Shell::Bash);
```

## The one-line spec test

Structural checks on the declaration itself are not in this module — they are in
[`to_kdl`](/rust/spec#round-trip-guarantee), which asserts in debug builds that the tree is
coherent. That test is worth writing beside these:

```rust
#[test]
fn spec_is_valid() {
    let _: usage_parser::Spec = Cli::to_kdl().parse().unwrap();
}
```

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
