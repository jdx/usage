# Generate shell completions

Generate a completion script for Bash, Zsh, Fish, PowerShell, or Nushell. When the
user presses Tab, the script calls `usage complete-word` to read the spec and
return candidates.

You need the [Usage CLI](/cli/#installation), a spec file, and the executable you
want to complete. For an end-to-end example, start with
[Write a spec](/guide/getting-started).

## Install for your shell

The two positional arguments are the shell and the executable's name as typed at
the prompt. Select your shell below:

::: code-group

```sh [Bash]
usage generate completion bash mycli --file ./mycli.usage.kdl --install
```

```sh [Zsh]
usage generate completion zsh mycli --file ./mycli.usage.kdl --install
```

```fish [Fish]
usage generate completion fish mycli --file ./mycli.usage.kdl --install
```

```powershell [PowerShell]
usage generate completion powershell mycli --file ./mycli.usage.kdl --install
```

```nushell [Nushell]
usage generate completion nu mycli --file ./mycli.usage.kdl --install
```

:::

Follow any setup instructions printed by the command, then open a new shell and
try completing `mycli --` with Tab.

`--install` writes the completion file. It does not edit shell startup files or
PowerShell profiles; when an extra line is needed, it prints that line for you.
An unchanged file is left alone. A file created outside Usage is not replaced
unless you pass `--force`.

::: info Runtime dependency
The generated scripts need `usage` on `PATH`. If you distribute them, include it
in your installation instructions. The [Rust framework's native
completions](/rust/completions) run through your binary instead and also support
Elvish.
:::

### Bash setup

Bash scripts need [bash-completion](https://github.com/scop/bash-completion)
version 2.11 or later, installed and sourced before the generated script. Install
it through your package manager, for example `apt install bash-completion` or
`brew install bash-completion@2`, and follow the package's shell setup instructions.

## Read the spec from the installed executable

Use `--usage-cmd` when a command can print its spec. This keeps completion aligned
with the installed binary rather than a separately maintained file:

```sh
usage generate completion zsh mycli --usage-cmd "mycli --usage-spec" --install
```

`--usage-spec` is an [integration pattern](/spec/integrations/clap), not a universal
flag. A CLI built with the [Rust framework](/rust/) exposes `mycli __usage_spec__`;
use that command instead.

## Write to a custom location

Without `--install`, the script goes to stdout. For example, create and load a
Bash completion in the current shell:

```sh
mkdir -p ~/.bash_completions
usage generate completion bash mycli --file ./mycli.usage.kdl > ~/.bash_completions/mycli.bash
source ~/.bash_completions/mycli.bash
```

To load it in future sessions, source the file from your shell's startup file
after initializing its completion system. Use `--install` for the shell's standard
completion directory and setup instructions.

Fig and Amazon Q use a different output format:

```sh
usage generate fig --file ./mycli.usage.kdl --out-file ./mycli.fig.ts
```

## What the script does

Each script calls `usage complete-word` with the spec and the words typed so far, and prints
what comes back. The same command works by hand, which is the quickest way to check what a spec
offers at a given point:

```bash
$ usage complete-word --file ./mycli.usage.kdl -- mycli cmd1 cmd2 --f
--file
--force
```

Because the shell-specific part is this thin, adding a shell is mostly a matter of writing the
shim.

## Shebang scripts

Scripts that use the `usage` shebang (`#!/usr/bin/env -S usage bash`, see
[Scripts](/cli/scripts)) need no per-script completion file. One line in the shell's rc enables
Tab on every such script on `$PATH`. bash, in `~/.bashrc`:

```bash
source <(usage g completion-init bash)
```

zsh, in `~/.zshrc`:

```bash
source <(usage g completion-init zsh)
```

fish, in `~/.config/fish/conf.d/usage.fish`:

```bash
usage g completion-init fish | source
```

After restarting the shell, `<Tab>` works on any executable whose first line is a `usage`
shebang. Each shell gets there its own way:

- **bash** registers a `complete -D` default handler that dispatches to `usage complete-word`
  for usage shebangs. Source it **after** bash-completion, so the existing default handler is
  chained to for everything else.
- **zsh** registers a `compdef -default-` fallback, which falls back to `_files` for
  everything else.
- **fish** has no default-completer fallback, so it scans `$PATH` once at shell startup and
  registers `complete -c <name>` for each usage-shebang script it finds.

## Completions for `usage` itself

`usage` is another CLI that can describe itself, so the general command applies:

```bash
usage g completion zsh usage --usage-cmd "command usage --usage-spec" --install
```

For distribution packages, write completion assets during packaging and install them
into the paths prescribed by that package manager. For example, these commands
produce the assets in the current directory:

```bash
usage --completions bash > usage.bash
usage --completions zsh > _usage
usage --completions fish > usage.fish
```
