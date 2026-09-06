---
description: Choose a Usage workflow, write your first KDL spec, and generate completions and documentation.
---

# Get started

Usage connects a CLI definition to its parser, completions, help, and documentation.
Choose the starting point that matches your project:

| You want to…                        | Start with…                                   |
| ----------------------------------- | --------------------------------------------- |
| Build a Rust CLI                    | The [Rust quickstart](/rust/quickstart)       |
| Add flags and help to a script      | The [script guide](/cli/scripts)              |
| Export an existing CLI's definition | A [framework integration](/spec/integrations) |
| Write a spec and generate artifacts | The walkthrough below                         |

## Install the CLI

Choose one method:

::: code-group

```sh [mise]
mise use -g usage
```

```sh [Homebrew]
brew install usage
```

```sh [Cargo]
cargo install usage-cli --locked
```

:::

Check that it is available with `usage --version`. See
[installation](/cli/#installation) for other options.

## Write a spec

Save this as `mycli.usage.kdl`:

```kdl
name "My CLI"
bin "mycli"
about "Deploy services from your terminal"

cmd "deploy" help="Deploy a service" {
    arg "<service>" help="Service to deploy"
    flag "-e --env <env>" help="Target environment" default="staging" {
        choices "staging" "production"
    }
}
```

This describes `mycli deploy <service> --env <env>`. The service is required; the
environment defaults to `staging` and accepts either listed choice.

KDL nodes start with a name, properties use `key=value`, and braces contain child
nodes. Strings are quoted here for readability. Use `#true` and `#false` for
booleans. The [spec guide](/spec/) covers more syntax.

::: info The spec describes the interface
This file does not implement or install `mycli`. Your application performs the
actual deployment. You can try the following validation and generation commands
without that application installed.
:::

## Validate and inspect it

```sh
usage lint mycli.usage.kdl
usage complete-word --file mycli.usage.kdl -- mycli deploy api --env prod
```

The completion command returns `production`. You can also see how Usage interprets
a complete command line:

```sh
usage explain --file mycli.usage.kdl -- mycli deploy api --env production
```

[`usage explain`](/cli/reference/explain) shows how words bind to arguments and
flags, including where fallback values come from.

## Generate documentation

```sh
usage generate markdown --file mycli.usage.kdl --out-file reference.md
usage generate manpage --file mycli.usage.kdl --out-file mycli.1
```

Open `reference.md` to see the command, argument, flag, choices, and default.
Preview the man page with `man ./mycli.1`.

For a documentation website, generate one page per command:

```sh
usage generate markdown --file mycli.usage.kdl --multi \
  --out-dir docs/commands --url-prefix /commands
```

Regenerate these files when your CLI changes. See [Markdown generation](/cli/markdown)
for templates and formatting options.

## Add shell completions

Once your `mycli` executable is on `PATH`, generate a script for your shell:

::: code-group

```sh [Bash]
usage generate completion bash mycli --file mycli.usage.kdl --install
```

```sh [Zsh]
usage generate completion zsh mycli --file mycli.usage.kdl --install
```

```sh [Fish]
usage generate completion fish mycli --file mycli.usage.kdl --install
```

```powershell [PowerShell]
usage generate completion powershell mycli --file mycli.usage.kdl --install
```

```nushell [Nushell]
usage generate completion nu mycli --file mycli.usage.kdl --install
```

:::

Follow any startup-file instruction printed by `--install`, then start a new
shell. Type `mycli deploy api --env ` and press Tab.

Bash also needs `bash-completion` installed and sourced. All five generated scripts
call `usage` at runtime. See [shell completions](/cli/completions) for setup and
troubleshooting.

## Keep the definition current

If your CLI can print its own spec, generate from that output:

```sh
mycli __usage_spec__ | usage generate markdown --file - --out-file reference.md
```

`__usage_spec__` is the Rust framework's built-in endpoint. Other integrations
commonly expose `--usage-spec`; use the command your application provides.

Before a release, [compare the old and new specs](/cli/diff) to identify breaking
interface changes. Use the [spec reference](/spec/reference/) to add global flags,
configuration, dynamic completion, and other features.
