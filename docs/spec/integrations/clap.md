# clap (Rust)

[`clap_usage`](https://crates.io/crates/clap_usage) generates a usage spec from a CLI built with [clap](https://crates.io/crates/clap).

## Installation

```toml
[dependencies]
clap_usage = "5"
```

## Quick Start

```rust
use clap::Command;

let mut cmd = Command::new("mycli")
    .version("1.0")
    .arg(clap::Arg::new("input"));

let mut buf = Vec::new();
clap_usage::generate(&mut cmd, "mycli", &mut buf);
println!("{}", String::from_utf8(buf).unwrap());
```

For migrations, use `spec_with_report` (or `generate_with_report`) and require a
clean report before trusting the generated spec:

```rust
let (spec, report) = clap_usage::spec_with_report(&mut cmd, "mycli");
for loss in report.losses() {
    eprintln!("{}: {:?}", loss.command.join(" "), loss);
}
assert!(report.is_lossless());
println!("{spec}");
```

The report includes the command path, clap argument ID, feature, and source detail
for each detectable loss. `is_lossless()` therefore means lossless for behavior
visible through clap's public getters, not for every setter clap exposes. Before
treating the generated spec as fully compatible, audit the declaration against the
[clap migration guide](/rust/migrating-from-clap#compatibility-gaps).

## Integration Pattern

A common approach is to add a hidden `--usage-spec` flag that outputs the spec:

```rust
use clap::{Arg, Command};
use std::io;

let mut cmd = Command::new("mycli")
    .arg(Arg::new("usage-spec")
        .long("usage-spec")
        .hide(true)
        .action(clap::ArgAction::SetTrue));

let matches = cmd.clone().get_matches();

if matches.get_flag("usage-spec") {
    clap_usage::generate(&mut cmd, "mycli", &mut io::stdout());
    return;
}
```

Then pipe the output to `usage`, passing `-f -` to read the spec from stdin:

```bash
mycli --usage-spec | usage generate completion bash mycli -f -
mycli --usage-spec | usage generate md -f - --out-file docs.md
mycli --usage-spec | usage generate manpage -f - --out-file mycli.1
```

For completions, `--usage-cmd 'mycli --usage-spec'` can replace the pipe and
`-f -`, letting the completion script fetch a fresh spec itself at runtime.

## What a generated spec cannot carry

The spec is produced by reading a `clap::Command` back, so it can only carry what clap
exposes a getter for. [`requires`](/spec/reference/flag#requires) is the notable one it
does not: `Arg::requires`, `requires_if`, `requires_ifs` and `requires_all` are setters
with no reader, so a flag declared with them arrives in the spec with no requirement on
it, and everything downstream — help, docs, completions — describes a CLI without that
constraint. `Arg::default_value_if` is the same hole: a generated spec never carries
[`default_if`](/spec/reference/flag#default_if).

[`multicall`](/spec/reference/#multicall) is one clap _does_ expose:
`Command::is_multicall_set` reaches the spec as `multicall #true`.

## Links

- [Migrating from clap](/rust/migrating-from-clap)
- [crate on crates.io](https://crates.io/crates/clap_usage)
- [source code](https://github.com/jdx/usage/tree/main/clap_usage)
