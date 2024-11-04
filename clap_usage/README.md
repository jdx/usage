# clap_usage

Generates [usage spec](https://usage.jdx.dev) for CLIs written with [clap](https://crates.io/crates/clap).

## Usage

```rust
use clap::{arg, Command, ValueHint};
use clap_usage::generate;
use std::io::BufWriter;

fn build_cli() -> Command {
    Command::new("example")
        .arg(arg!(--file <FILE> "some input file").value_hint(ValueHint::AnyPath))
        .arg(arg!(--usage))
}

fn main() {
    let matches = build_cli().get_matches();

    if matches.get_flag("usage") {
        let mut cmd = build_cli();
        eprintln!("Generating usage spec...");
        clap_usage::generate(&mut cmd, "example", &mut std::io::stderr()).unwrap();
        return;
    }

    // Your CLI code here...
}
```
