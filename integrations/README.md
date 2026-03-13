# Usage Integrations

Integrations that generate usage specs from CLI framework definitions, enabling shell completions, docs, and man pages from a single source.

For the full list of available and planned integrations, see the [Integrations documentation](https://usage.jdx.dev/spec/integrations/).

## How Integrations Work

An integration extracts the CLI definition (commands, flags, args, completions) from a framework's internal representation and outputs a [usage spec](https://usage.jdx.dev/spec/) in KDL format. This spec can then drive:

- **Shell completions** for bash, zsh, fish, PowerShell, and nushell
- **Markdown documentation**
- **Man pages**
- **`--help` output**

See [`clap_usage`](../clap_usage/) for a reference implementation.
