# `mise completion`

Generate shell completions

###### Arg `[SHELL]`

Shell type to generate completions for

##### Flag `-s --shell <SHELL_TYPE>`

Shell type to generate completions for

##### Flag `--usage`

Always use usage for completions.
Currently, usage is the default for fish and bash but not zsh since it has a few quirks
to work out first.

This requires the `usage` CLI to be installed.
https://usage.jdx.dev

Examples:

    $ mise completion bash > /etc/bash_completion.d/mise
    $ mise completion zsh  > /usr/local/share/zsh/site-functions/_mise
    $ mise completion fish > ~/.config/fish/completions/mise.fish
