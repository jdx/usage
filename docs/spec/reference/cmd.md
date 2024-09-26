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
# all help fields can be either inline params or separate nodes like
# below for the *_long_help fields. Typically when a lot of space is needed
# it's cleaner to use separate nodes.
cmd "config" {
  before_long_help "shown before the command"
  long_help "longer description"
  after_long_help "shown after the command"
}

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

## Mounting dynamic commands

A usage spec can define a command to run which emits extra usage spec which will be merged into the cmd.
For example, assume a CLI named `mycli` has a command `run` which executes a set of tasks, those tasks
are themselves commands which have their own sets of args/flags dynamically generated. To support this,
create a hidden command like `mycli mount-usage-tasks` which emits usage spec for the tasks. Then,
create a `mount` on the `run` command. Here is the static usage spec for the `mycli` CLI as described:

```sh
cmd "mount-usage-tasks" hide=true
cmd "run" {
	mount run="mycli mount-usage-tasks"
}
```

Calling `mycli mount-usage-tasks` would emit something like this:

```sh
cmd "task1" {
  arg "arg1" help="task1 arg1"
  flag "flag1" help="task1 flag1"
}
cmd "task2" {
  arg "arg1" help="task2 arg1"
  flag "flag1" help="task2 flag1"
}
```

Now when using completion with usage, if the user types `mycli run <tab><tab>`, usage will then
call `mycli mount-usage-tasks` and merge the emitted usage into the `run` command and display the
task commands as if they were statically defined in the usage spec.
