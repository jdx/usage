# clap (Rust)

[`clap_usage`](https://crates.io/crates/clap_usage) generates a usage spec from a CLI built with [clap](https://crates.io/crates/clap).

## Installation

```toml
[dependencies]
clap_usage = "2"
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

Then pipe the output to `usage`:

```bash
mycli --usage-spec | usage generate completion bash
mycli --usage-spec | usage generate md --out-file docs.md
mycli --usage-spec | usage generate manpage --out-file mycli.1
```

## Links

- [crate on crates.io](https://crates.io/crates/clap_usage)
- [source code](https://github.com/jdx/usage/tree/main/clap_usage)
