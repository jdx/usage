# Cobra (Go)

[`cobra_usage`](https://github.com/jdx/usage/tree/main/integrations/cobra) converts a [Cobra](https://github.com/spf13/cobra) command tree into a usage spec.

## Installation

```bash
go get github.com/jdx/usage/integrations/cobra
```

## Quick Start

```go
import cobra_usage "github.com/jdx/usage/integrations/cobra"

// Print usage spec as KDL
fmt.Print(cobra_usage.Generate(rootCmd))
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
rootCmd.Execute()
```

Then pipe the output to `usage`, passing `-f -` to read the spec from stdin:

```bash
mycli --usage-spec | usage generate completion bash mycli -f -
mycli --usage-spec | usage generate md -f - --out-file docs.md
mycli --usage-spec | usage generate manpage -f - --out-file mycli.1
```

For completions, `--usage-cmd 'mycli --usage-spec'` can replace the pipe and
`-f -`, letting the completion script fetch a fresh spec itself at runtime.

## API

| Function                              | Description                            |
| ------------------------------------- | -------------------------------------- |
| `Generate(cmd) string`                | Returns the usage spec as a KDL string |
| `GenerateJSON(cmd) ([]byte, error)`   | Returns the usage spec as JSON         |
| `GenerateToFile(cmd, path) error`     | Writes the KDL spec to a file          |
| `GenerateJSONToFile(cmd, path) error` | Writes the JSON spec to a file         |

## Feature Mapping

| Cobra                                              | Usage Spec                                    |
| -------------------------------------------------- | --------------------------------------------- |
| `cmd.Name()`                                       | `name`, `bin`                                 |
| `cmd.Short`                                        | `about` (root), `help` (subcommand)           |
| `cmd.Long`                                         | `long_about` (root), `long_help` (subcommand) |
| `cmd.Version`                                      | `version`                                     |
| `cmd.Aliases`                                      | `alias`                                       |
| `cmd.Hidden`                                       | `hide=#true`                                  |
| `cmd.Deprecated`                                   | `deprecated="message"`                        |
| `cmd.Example`                                      | `example` node (top-level for root)           |
| `cmd.Use` args (`<required>`, `[optional]`, `...`) | `arg` nodes                                   |
| `cmd.ValidArgs`                                    | `choices` on first arg                        |
| Persistent flags                                   | `global=#true`                                |
| `flag.Shorthand`                                   | `-s` in flag name                             |
| `flag.Name`                                        | `--long` in flag name                         |
| `flag.Usage`                                       | `help="..."`                                  |
| `flag.Hidden`                                      | `hide=#true`                                  |
| `flag.Deprecated`                                  | `deprecated="..."`                            |
| `flag.DefValue`                                    | `default="value"`                             |
| Bool flags                                         | No arg child                                  |
| Count flags (`CountP`)                             | `count=#true var=#true`                       |
| Other flags                                        | `arg <UPPER_NAME>` child                      |
| `MarkFlagRequired`                                 | `required=#true`                              |

Cobra's `Example` is a single free-form block rather than a list of structured
examples, so it maps to one `example` node holding the whole text, dedented.
Comment lines the author wrote stay inside the example code.

## Example

See [`example/main.go`](https://github.com/jdx/usage/tree/main/integrations/cobra/example) for a complete example CLI.

```bash
cd integrations/cobra/example
go run . --usage-spec
```
