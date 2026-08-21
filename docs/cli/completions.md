# Generating Completion Scripts

## Auto-completion for shebang scripts (bash)

If you have shell scripts that use the `usage` shebang
(e.g. `#!/usr/bin/env -S usage bash`) and live on `$PATH`, you can enable
tab-completion for all of them at once with a single init line — no per-script
generation required.

Add this to your `~/.bashrc`:

```bash
source <(usage g completion-init bash)
```

For zsh:

```bash
# Add this to your ~/.zshrc
source <(usage g completion-init zsh)
```

For fish:

```bash
# Add this to ~/.config/fish/conf.d/usage.fish
usage g completion-init fish | source
```

After restarting your shell, `<Tab>` will work on any script whose first line
is a `usage` shebang. Mechanism per shell:

- **bash**: registers a `complete -D` default handler that dispatches to
  `usage complete-word` for usage shebangs. Source this **after**
  bash-completion so the existing default handler is chained to for non-usage
  commands.
- **zsh**: registers a `compdef -default-` fallback. Falls back to `_files`
  for non-usage commands.
- **fish**: scans `$PATH` once at shell startup (fish has no default-completer
  fallback) and registers `complete -c <name>` per usage-shebang script.

This is the simplest setup if your CLIs are written as `usage`-shebang scripts.
For `.usage.kdl` specs or binaries with `--usage`, generate per-binary
completion scripts as shown below.

## Per-binary completion scripts

Usage can generate completion scripts for any shell, and put them where that shell looks.
`--install` does the second part:

```bash
usage g completion bash mycli -f ./mycli.usage.kdl --install
mycli --<TAB>
```

It writes the script file and nothing else: no shell rc file and no PowerShell profile is
edited. Where a shell needs a one-time line of its own — zsh's `fpath+=`, PowerShell's
dot-source — the line is printed for you to add. Running it again when nothing has changed
writes nothing, and a file usage did not write is reported rather than replaced; pass
`--force` to replace one anyway.

Without `--install` the script goes to stdout, which is what to use when you want to choose the
path yourself. For bash:

```bash
usage g completion bash mycli -f ./mycli.usage.kdl > ~/.bash_completions/mycli.bash
source ~/.bash_completions/mycli.bash
mycli --<TAB>
```

Generated bash completions call
[bash-completion](https://github.com/scop/bash-completion)'s helper functions, so it
must be installed and sourced before the generated script. Install it from your
package manager (`apt install bash-completion`, `brew install bash-completion@2`, …);
a generated completion that cannot find it says so instead of failing silently.
Version 2.11 and newer are supported, which covers every current distribution
release.

zsh:

```bash
usage g completion zsh mycli -f ./mycli.usage.kdl > ~/.zsh_completions/_mycli
source ~/.zsh_completions/_mycli
mycli --<TAB>
```

fish:

```bash
usage g completion fish mycli -f ./mycli.usage.kdl > ~/.config/fish/completions/mycli.fish
mycli --<TAB>
```

fig/Amazon Q:

```bash
usage g fig -f ./mycli.usage.kdl > ./mycli.fig.ts
mycli --<TAB>
```

nushell:

```nushell
usage g completion nu mycli -f ./mycli.usage.kdl > ~/.config/nushell/autoload/mycli.nu
source ~/.config/nushell/autoload/mycli.nu
mycli --<TAB>
```

PowerShell:

```powershell
usage g completion powershell mycli -f ./mycli.usage.kdl > ./mycli.ps1
. ./mycli.ps1
mycli --<TAB>
```

The supported 6.x targets are bash, zsh, fish, PowerShell, and Nushell. The
first four are the clap-compatibility set; Nushell is a usage extension. Elvish
is not a 6.0 target.

::: info
Usage CLI is a runtime dependency for the generated completion scripts. Your users
will need to have `usage` installed in order for the completion scripts to work.
:::

New shells should be easy to add because the logic around completions is mostly handled by the Usage CLI.
Typically, completion scripts will call usage like this to fetch completion choices (cword is the index of
the current word):

```bash
$ usage complete-word --file ./mycli.usage.kdl -- mycli cmd1 cmd2 --f
--force
--file
```

## Completions for `usage` CLI itself

For yourself, the general command works — `usage` is another CLI that can describe itself, so
there is no special case:

```bash
usage g completion zsh usage --usage-cmd "command usage --usage-spec" --install
```

For a package, keep the redirects below. Those are system directories the package manager owns,
which is exactly why `--install` does not target them.

```bash
usage --completions bash > /etc/bash_completion.d/usage
usage --completions zsh > /usr/share/zsh/site-functions/_usage
usage --completions fish > ~/.config/fish/completions/usage.fish
```
