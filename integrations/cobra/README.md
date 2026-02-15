# cobra_usage

Generate [usage](https://usage.jdx.dev) specs from [Cobra](https://github.com/spf13/cobra) command definitions.

This enables shell completions (bash, zsh, fish, PowerShell, nushell), markdown documentation, and man pages from your existing Cobra CLI.

## Installation

```bash
go get github.com/jdx/usage/integrations/cobra
```

## Quick Start

```go
import (
    cobra_usage "github.com/jdx/usage/integrations/cobra"
)

func main() {
    root := &cobra.Command{
        Use:     "mycli",
        Short:   "My CLI tool",
        Version: "1.0.0",
    }
    // ... add subcommands, flags, args ...

    // Print usage spec as KDL
    fmt.Print(cobra_usage.Generate(root))
}
```

## Integration Pattern

The recommended pattern is to check for a `--usage-spec` flag before `Execute()`:

```go
for _, arg := range os.Args[1:] {
    if arg == "--usage-spec" {
        fmt.Print(cobra_usage.Generate(rootCmd))
        return
    }
}
if err := rootCmd.Execute(); err != nil {
    os.Exit(1)
}
```

Then pipe the output to `usage` to generate completions, docs, or man pages:

```bash
mycli --usage-spec | usage generate completion bash
mycli --usage-spec | usage generate md -f -
mycli --usage-spec | usage generate man -f -
```

## API

| Function | Description |
|---|---|
| `Generate(cmd) string` | Returns the usage spec as a KDL string |
| `GenerateJSON(cmd) ([]byte, error)` | Returns the usage spec as JSON |
| `GenerateToFile(cmd, path) error` | Writes the KDL spec to a file |
| `GenerateJSONToFile(cmd, path) error` | Writes the JSON spec to a file |

## Feature Mapping

| Cobra | Usage Spec |
|---|---|
| `cmd.Name()` | `name`, `bin` |
| `cmd.Short` | `about` (root), `help` (subcommand) |
| `cmd.Long` | `long_about` (root), `long_help` (subcommand) |
| `cmd.Version` | `version` |
| `cmd.Aliases` | `alias` |
| `cmd.Hidden` | `hide=#true` |
| `cmd.Deprecated` | `deprecated="message"` |
| `cmd.Use` args (`<required>`, `[optional]`, `...`) | `arg` nodes |
| `cmd.ValidArgs` | `choices` on first arg |
| Persistent flags | `global=#true` |
| `flag.Shorthand` | `-s` in flag name |
| `flag.Name` | `--long` in flag name |
| `flag.Usage` | `help="..."` |
| `flag.Hidden` | `hide=#true` |
| `flag.Deprecated` | `deprecated="..."` |
| `flag.DefValue` | `default="value"` |
| Bool flags | No arg child |
| Count flags (`CountP`) | `count=#true var=#true` |
| Other flags | `arg <UPPER_NAME>` child |
| `MarkFlagRequired` | `required=#true` |

## Example

See [`example/main.go`](example/main.go) for a complete example CLI.

```bash
cd example
go run . --usage-spec
```
