### `completion`

#### Args

* `[SHELL]` – Shell type to generate completions for

#### Flags

* `-s,--shell <SHELL_TYPE>` – Shell type to generate completions for
Generate shell completions
Examples:
  $ mise completion bash > /etc/bash_completion.d/mise
  $ mise completion zsh  > /usr/local/share/zsh/site-functions/_mise
  $ mise completion fish > ~/.config/fish/completions/mise.fish
