# Integrations

Integrations extract CLI definitions from framework internals and output a [usage spec](/spec/) in KDL format. This enables shell completions, markdown docs, and man pages from your existing CLI framework — no manual spec authoring needed.

## Cobra (Go)

[`cobra_usage`](https://github.com/jdx/usage/tree/main/integrations/cobra) converts a [Cobra](https://github.com/spf13/cobra) command tree into a usage spec.

### Installation

```bash
go get github.com/jdx/usage/integrations/cobra
```

### Quick Start

```go
import cobra_usage "github.com/jdx/usage/integrations/cobra"

// Print usage spec as KDL
fmt.Print(cobra_usage.Generate(rootCmd))
```

The recommended pattern is to check for a `--usage-spec` flag before `Execute()`:

```go
for _, arg := range os.Args[1:] {
    if arg == "--usage-spec" {
        fmt.Print(cobra_usage.Generate(rootCmd))
        return
    }
}
rootCmd.Execute()
```

Then pipe the output to `usage`:

```bash
mycli --usage-spec | usage generate completion bash
mycli --usage-spec | usage generate md --out-file docs.md
mycli --usage-spec | usage generate manpage --out-file mycli.1
```

See the [full documentation](https://github.com/jdx/usage/tree/main/integrations/cobra) for the complete API and feature mapping.

## clap (Rust)

[`clap_usage`](https://crates.io/crates/clap_usage) generates a usage spec from a CLI built with [clap](https://crates.io/crates/clap).

### Installation

```toml
[dependencies]
clap_usage = "2"
```

### Quick Start

```rust
use clap::Command;

let mut cmd = Command::new("mycli")
    .version("1.0")
    .arg(clap::Arg::new("input"));

let mut buf = Vec::new();
clap_usage::generate(&mut cmd, "mycli", &mut buf);
println!("{}", String::from_utf8(buf).unwrap());
```

## Community Integrations

We're looking to add integrations for more CLI frameworks. High-priority targets include:

- **Commander.js** (Node.js) — [tj/commander.js](https://github.com/tj/commander.js)
- **urfave/cli** (Go) — [urfave/cli](https://github.com/urfave/cli)
- **Typer** (Python) — [fastapi/typer](https://github.com/fastapi/typer)
- **Click** (Python) — [pallets/click](https://github.com/pallets/click)
- **argparse** (Python) — stdlib

See the [full tracker](https://github.com/jdx/usage/tree/main/integrations) for the complete list of planned integrations across 16+ languages. Contributions welcome!
