# Generating Completion Scripts

Usage can generate completion scripts for any shell. Here is an example for bash:

```sh-session
usage g completion bash mycli -f ./mycli.usage.kdl > ~/.bash_completions/mycli.bash
source ~/.bash_completions/mycli.bash
mycli --<TAB>
```

zsh:

```sh-session
usage g completion zsh mycli -f ./mycli.usage.kdl > ~/.zsh_completions/_mycli
source ~/.zsh_completions/_mycli
mycli --<TAB>
```

fish:

```sh-session
usage g completion fish mycli -f ./mycli.usage.kdl > ~/.config/fish/completions/mycli.fish
mycli --<TAB>
```

::: info
Usage CLI is a runtime dependency for the generated completion scripts. Your users
will need to have `usage` installed in order for the completion scripts to work.
:::

New shells should be easy to add because the logic around completions is mostly handled by the Usage CLI.
Typically, completion scripts will call usage like this to fetch completion choices (cword is the index of
the current word):

```sh-session
$ usage complete-word --file ./mycli.usage.kdl -- mycli cmd1 cmd2 --f
--force
--file
```
