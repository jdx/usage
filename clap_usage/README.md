# clap_usage

Export a [Usage spec](https://usage.jdx.dev/spec/) from a
[clap](https://crates.io/crates/clap) command definition. Use the spec to generate
shell completions, Markdown documentation, man pages, and SDKs.

## Add the integration

```toml
[dependencies]
clap = "4"
clap_usage = "5"
```

Expose a hidden flag that prints the spec:

```rust
use clap::{Arg, ArgAction, Command};

fn build_cli() -> Command {
    Command::new("example")
        .arg(Arg::new("file").long("file"))
        .arg(
            Arg::new("usage-spec")
                .long("usage-spec")
                .hide(true)
                .action(ArgAction::SetTrue),
        )
}

fn main() {
    let mut cmd = build_cli();
    let matches = cmd.clone().get_matches();
    if matches.get_flag("usage-spec") {
        clap_usage::generate(&mut cmd, "example", &mut std::io::stdout());
        return;
    }
    // Run the application using `matches`.
}
```

With the executable and the [Usage CLI](https://usage.jdx.dev/cli/#installation)
on `PATH`:

```sh
example --usage-spec > example.usage.kdl
usage generate markdown --file example.usage.kdl --out-file reference.md
usage generate completion zsh example --usage-cmd "example --usage-spec" --install
```

The exporter can only read behavior exposed by clap's public getters. Use
`spec_with_report` or `generate_with_report` to inspect detectable losses before
relying on the generated interface. See the
[integration guide](https://usage.jdx.dev/spec/integrations/clap) for reports,
compatibility limits, and completion setup.
