# `mise env`

###### Aliases: `e`

Exports env vars to activate mise a single time

Use this if you don't want to permanently install mise. It's not necessary to
use this if you have `mise activate` in your shell rc file.

###### Arg `[TOOL@VERSION]...`

Tool(s) to use

##### Flag `-J --json`

Output in JSON format

##### Flag `-s --shell <SHELL>`

Shell type to generate environment variables forExamples:
  $ eval "$(mise env -s bash)"
  $ eval "$(mise env -s zsh)"
  $ mise env -s fish | source
  $ execx($(mise env -s xonsh))
