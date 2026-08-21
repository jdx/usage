# Spec Output

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

`Cli::to_kdl()` writes a complete [usage spec](/spec/) from the same static metadata the parser
runs on. This is the bridge to the rest of the toolkit: markdown docs, manpages, completion
scripts for other consumers, SDK generation, and linting all consume that KDL.

For the declarations shown across these pages, the emitted spec looks like:

```kdl
name "ex"
bin "ex"
version "1.2.3"
about "does things"
flag "-j --jobs" help="how many jobs" global=#true help_heading="Performance" env="EX_JOBS" default="4" {
    long_help "More about jobs.\nOn two lines."
    arg "<n>"
}
flag "--color" help="colorize output" negate="--no-color" default="true"
flag "-v --verbose" hide=#true count=#true
flag "--include" var=#true var_min=1 var_max=5 overrides="--exclude" {
    arg "<pattern>..."
}
group "input" "--file" "--url" "--stdin" required=#true
arg "[file]" help="the file" env="EX_FILE" default="a.txt"
cmd "install" help="install a tool" effect="write" {
    alias "i"
    alias "add" hide=#true
    flag "-f --force"
    arg "<tool>"
}
cmd "run" help="run a task" restart_token=":::" {
    mount run="ex tasks --usage"
    arg "[args]..." double_dash="preserve"
}
```

## Round-trip guarantee

The emitted KDL parses with usage-lib and every property survives the trip — this is enforced by
the conformance suite. The test every adopter should write is one line:

```rust
#[test]
fn spec_is_valid() {
    let spec: usage::Spec = Cli::to_kdl().parse().unwrap();
    let _ = spec;
}
```

Beyond parsing, `to_kdl` asserts (in debug builds) that the tree is coherent: no duplicate keys,
no duplicate flag spellings across a `flatten` boundary, no duplicate group names, no unfillable
argument after an unbounded variadic. Those fire in your test, not on users.

## Feeding usage-cli

The pattern usage-cli itself ships is a hidden flag that prints the spec, so the binary is the
source of truth:

```rust
#[usage(long, hide)]
usage_spec: bool,
```

```bash
mycli --usage-spec > mycli.usage.kdl

usage g markdown -f mycli.usage.kdl --out-dir docs   # markdown docs
usage g manpage  -f mycli.usage.kdl > mycli.1        # man page
usage g completion bash mycli --file mycli.usage.kdl # completion script
usage g json     -f mycli.usage.kdl                  # JSON form
usage lint       -f mycli.usage.kdl                  # lint the spec
```

`min_usage_version = "…"` on the root is written first in the document, as the CLI's claim about
which usage consumers can read it.

## Runtime identity and portable identity

An embedded CLI may be invoked under a name chosen by its caller. Pair each computed identity
with the literal written to portable artifacts:

```rust
#[derive(usage::Cli)]
#[usage(
    name = host::program_name(),
    name_spec = "mycli",
    bin = host::program_name(),
    bin_spec = "mycli",
    version = build::version(),
    version_spec = "6.0.0"
)]
struct Cli;
```

The name and bin expressions return `&'static str`; a computed version implements `ToString`.
They are evaluated only when the process renders help, version output, diagnostics, or a
completion script. Successful argument parsing still reads the static tables directly and does
not allocate or build a command graph. `to_kdl()` keeps `mycli` and `6.0.0`, so generated
artifacts are deterministic and do not depend on the embedding process. `--version` formats the
computed version, while `version_spec` remains the static value exported to KDL.

`Cli::runtime_app()` returns the borrowed view with the computed name and bin applied; it does
not currently apply the computed version. For a caller that already has different identity
values, `Cli::app().name(...).bin(...).version(...)` provides the split explicitly.

## What the parser does with the spec

Nothing, at runtime. The derive compiles your declaration into static tables that usage-argv
parses and renders help from directly — no KDL is parsed when your CLI runs, and usage-lib is
not a dependency of your binary. The spec is the _export_ format. The two implementations are
held to identical behavior by a shared conformance corpus and by rendering all 211 of mise's
help pages through both.

## What can't be expressed from the derive

Nothing, as of `example`: every node the spec format defines now has a derive attribute. That is
the rule this project holds itself to — a typed declaration must lower into the spec losslessly,
and where it cannot, the derive gains the vocabulary rather than the spec losing it.

The one property that does not carry over is an example's `lang`, which picks syntax
highlighting for a KDL-authored example and has nothing to choose between in Rust.
