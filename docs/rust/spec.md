# Spec Output

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
    // usage-lib is a separate package from the usage-rs facade. Alias it so the
    // type path does not collide with `usage`.
    let spec: usage_parser::Spec = Cli::to_kdl().parse().unwrap();
    let _ = spec;
}
```

```toml
[dev-dependencies]
usage-parser = { package = "usage-lib", version = "6" }
```

Beyond parsing, `to_kdl` asserts (in debug builds) that the tree is coherent: no duplicate keys,
no duplicate flag spellings across a `flatten` boundary, no duplicate group names, no unfillable
argument after an unbounded variadic. Those fire in your test, not on users.

## The endpoint

Every binary answers `__usage_spec__` with its own spec. You do not declare it, and there is
nothing to wire up:

```bash
mycli __usage_spec__ > mycli.usage.kdl

usage g markdown -f mycli.usage.kdl --out-dir docs   # markdown docs
usage g manpage  -f mycli.usage.kdl > mycli.1        # man page
usage g completion bash mycli --file mycli.usage.kdl # completion script
usage g json     -f mycli.usage.kdl                  # JSON form
usage lint          mycli.usage.kdl                  # lint the spec
usage mcp        -f mycli.usage.kdl                  # serve it to an agent
```

It is a word rather than a flag, and it is answered before the parse — so it is not in your
tables, cannot collide with a flag of yours, and does not appear in the document it prints. A
command line whose first word is `__usage_spec__` is the request; anywhere else the word is an
ordinary value. If your CLI declares a command of that spelling, yours wins.

KDL is the only format the binary emits. Pipe it through `usage g json -f -` for JSON: a second
serializer in every adopter's binary is a cost the conversion does not need.

A CLI in another language has no derive to generate this, so the convention there stays a flag
its author intercepts — `--usage-spec`, as in the [cobra guide](/spec/integrations/cobra).
usage-cli answers both, being both a Rust CLI and the tool that documented the flag first.

Three ways in, for the three shapes a program takes:

| you write                  | you get                                                               |
| -------------------------- | --------------------------------------------------------------------- |
| `Cli::parse()`             | the endpoint, answered and exited, before anything else               |
| `Cli::spec_request(&argv)` | `Some(kdl)` for a request, so an embedder renders it itself           |
| `Cli::to_kdl()`            | the document, in-process, for a build script or a checked-in artifact |

`#[usage(spec_endpoint = false)]` removes it, for a CLI counting bytes. What it costs is the KDL
writer and the cold metadata being reachable from `main`, which is **65 KB** on a small CLI
(773,424 against 708,768 bytes, stripped release, four flags and two subcommands). A CLI that
calls `to_kdl()` for a checked-in artifact links the same code and pays nothing extra for the
endpoint. `to_kdl()` itself stays either way — opting out removes the entry point, not the ability
to emit a spec.

### Appending raw KDL

Every node the derive can say, it says — see [what can't be
expressed](#what-can-t-be-expressed-from-the-derive), which is nearly nothing. `spec_extra` is
there for when that is not enough, and appends a file's KDL to the emitted document:

```rust
#[derive(usage::Cli)]
#[usage(bin = "mycli", spec_extra = "assets/mycli-extra.usage.kdl")]
struct Cli { /* … */ }
```

The path is relative to the crate that declares it, and the file is read at compile time. It joins
`to_kdl()` itself, so the endpoint, a checked-in artifact and your docs build all see one
document. Nothing parses the appended text while compiling — the [round-trip
test](#round-trip-guarantee) above is what catches a file with a mistake in it, so write that test
if you use this.

`min_usage_version = "…"` on the root is written first in the document, as the CLI's claim about
which usage consumers can read it.

Everything the spec says about the program rather than about one command is a root attribute
too. `author`, `license` and `repository` take an expression, so the `env!("CARGO_PKG_…")`
values stay the source of truth; `source_code_link_template` takes the [tera
template](/spec/reference/#source-code-link-template) that turns a command path into the
"view source" link on its markdown page:

```rust
#[derive(usage::Cli)]
#[usage(
    repository = env!("CARGO_PKG_REPOSITORY"),
    source_code_link_template = r#"https://github.com/me/mycli/blob/main/src/cli/{{path}}.rs"#,
)]
struct Cli;
```

A multi-line template is written unindented: a Rust raw string keeps every leading space.

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
    version_spec = "1.0.0"
)]
struct Cli;
```

The name and bin expressions return `&'static str`; a computed version implements `ToString`.
They are evaluated only when the process renders help, version output, diagnostics, or a
completion script. Successful argument parsing still reads the static tables directly and does
not allocate or build a command graph. `to_kdl()` keeps `mycli` and `1.0.0`, so generated
artifacts are deterministic and do not depend on the embedding process. `--version` formats the
computed version, while `version_spec` remains the static value exported to KDL.

`Cli::runtime_app()` returns the borrowed view with the computed name and bin applied; it does
not currently apply the computed version. For a caller that already has different identity
values, `Cli::app().name(...).bin(...).version(...)` provides the split explicitly.

`Cli::render_help` and `Cli::render_failure` are the same overlay for a `parse_from` caller that
handles help and diagnostics itself instead of using `parse()`. `help::render(Cli::spec(), …)`
keeps the portable literals.

## What the parser does with the spec

Nothing, at runtime. The derive compiles your declaration into static tables that usage-argv
parses and renders help from directly — no KDL is parsed when your CLI runs, and usage-lib is
not a dependency of your binary. The spec is the _export_ format. The two implementations are
held to identical behavior by a shared conformance corpus and by rendering all 211 of mise's
help pages through both.

## What can't be expressed from the derive

Nearly nothing, as of `example`: every node a command declares now has a derive attribute. That is
the rule this project holds itself to — a typed declaration must lower into the spec losslessly,
and where it cannot, the derive gains the vocabulary rather than the spec losing it.

The one property that does not carry over is an example's `lang`, which picks syntax highlighting
for a KDL-authored example and has nothing to choose between in Rust.

[`spec_extra`](#appending-raw-kdl) is the escape hatch if that ever stops being true for something
you need. Nothing in this repository uses it.
