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

After restarting your shell, `<Tab>` will work on any script whose first line
is a `usage` shebang. The init script registers a `complete -D` default handler
that detects the shebang at completion time and dispatches to
`usage complete-word`. Non-`usage` commands fall through to bash-completion's
loader if it's installed.

This is the simplest setup if your CLIs are written as `usage`-shebang scripts.
For `.usage.kdl` specs or binaries with `--usage`, generate per-binary
completion scripts as shown below.

::: info
Currently only `bash` is supported for the init flow. zsh / fish use different
completion mechanisms and are tracked as follow-ups.
:::

## Per-binary completion scripts

Usage can generate completion scripts for any shell. Here is an example for bash:

```bash
usage g completion bash mycli -f ./mycli.usage.kdl > ~/.bash_completions/mycli.bash
source ~/.bash_completions/mycli.bash
mycli --<TAB>
```

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

Completions for the `usage` CLI itself can be generated with one of the following commands:

```bash
usage --completions bash > /etc/bash_completion.d/usage
usage --completions zsh > /usr/share/zsh/site-functions/_usage
usage --completions fish > ~/.config/fish/completions/usage.fish
```
