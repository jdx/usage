# `cmd`

```sh
# aliases
cmd "config" help="Manage the CLI config" {
  alias "cfg" "cf" "cg"  # aliases for the command
  alias "conf" hide=true # hide alias from docs and completions
}

cmd "config" hide=true # hide command from docs and completions
cmd "config" subcommand_required=true # subcommand is not optional

# these are shown under -h
cmd "config" before_help="shown before the command"
cmd "config" help="short description"
cmd "config" after_help="shown after the command"

# these are shown under --help
cmd "config" before_long_help="shown before the command"
cmd "config" long_help="longer description"
cmd "config" after_long_help="shown after the command"

cmd "list" {
  example "Basic usage" r#"
    $ mycli list
    FRUIT  COLOR
    apple  red
    banana yellow
  "#
  example "JSON output" r#"
    $ mycli list --json
    [
      {"FRUIT": "apple", "COLOR": "red"},
      {"FRUIT": "banana", "COLOR": "yellow"}
    ]
  "#
}
```
