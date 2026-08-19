# `cmd`

```kdl
// aliases
cmd "config" help="Manage the CLI config" {
  alias "cfg" "cf" "cg"  // aliases for the command
  alias "conf" hide=#true // hide alias from docs and completions
}

cmd "config" hide=#true // hide command from docs and completions
cmd "config" subcommand_required=#true // subcommand is not optional

// these are shown under -h
cmd "config" before_help="shown before the command"
cmd "config" help="short description"
cmd "config" after_help="shown after the command"

// these are shown under --help
// all help fields can be either inline params or separate nodes like
// below for the *_long_help fields. Typically when a lot of space is needed
// it's cleaner to use separate nodes.
cmd "config" {
  before_long_help "shown before the command"
  long_help "longer description"
  after_long_help "shown after the command"
}

cmd "list" {
  example "Basic usage" #"""
    $ mycli list
    FRUIT  COLOR
    apple  red
    banana yellow
  """#
  example "JSON output" #"""
    $ mycli list --json
    [
      {"FRUIT": "apple", "COLOR": "red"},
      {"FRUIT": "banana", "COLOR": "yellow"}
    ]
  """#
}
```

## Mounting dynamic commands

A usage spec can define a command to run which emits extra usage spec which will be merged into the
cmd.
For example, assume a CLI named `mycli` has a command `run` which executes a set of tasks, those
tasks
are themselves commands which have their own sets of args/flags dynamically generated. To support
this,
create a hidden command like `mycli mount-usage-tasks` which emits usage spec for the tasks. Then,
create a `mount` on the `run` command. Here is the static usage spec for the `mycli` CLI as
described:

```kdl
cmd "mount-usage-tasks" hide=#true
cmd "run" {
	mount run="mycli mount-usage-tasks"
}
```

Calling `mycli mount-usage-tasks` would emit something like this:

```kdl
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

`mount run` is executed the same way as [`complete`'s `run`](./complete.md#which-shell-runs-run):
`sh -c`, falling back to `cmd /c` on Windows when there is no `sh` on `PATH`. A mount pointing at
a shebang script therefore needs a POSIX shell to be available; one that invokes a program
directly, like `mycli mount-usage-tasks` above, works either way.

### Mounting at the top level

A `mount` also works as a top-level node, for a CLI whose _own_ commands are
discovered rather than declared — one whose subcommands come from plugins, say:

```kdl
name "mycli"
bin "mycli"
cmd "install"
mount run="mycli plugin-commands"
```

Resolving a mount runs a process, so when the root's mount runs depends on what is
asking for it:

| asking                          | when it runs                                                                                                                                          |
| ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| a completion, or rendering help | up front — both need the whole command list, and `mycli <tab>` has no word to go on                                                                   |
| a parse                         | only when a word matches nothing already declared, so `mycli install` costs nothing extra and only `mycli something-from-a-plugin` pays for discovery |
| a parse, for a flag             | never                                                                                                                                                 |

Declaring the commands you know about therefore keeps ordinary invocations free,
while completions still see everything.

A `default_subcommand` outranks discovery, because it already says what an unmatched
word means and costs nothing to consult. Without that, a CLI that routes unknown
words to a task runner would spawn its discovery process once per task. A mount can
ask to win anyway:

```kdl
mount run="mycli plugin-commands" overrides_default=#true
```

That setting applies to completions as much as to parses, and deliberately: a
completion offering a command that running would hand to the default subcommand
instead is worse than not offering it. So a root mount under a `default_subcommand`
contributes nothing anywhere until it asks to outrank it.

### Unknown flags

A flag-like token that names no flag becomes a word, offered to the positionals like
any other — because a spec often parses a command line whose flags belong to
something else. A command that owns all of its flags can refuse them instead:

```kdl
unknown_flags "error"               // for the whole CLI
cmd "exec" unknown_flags="value"    // except here, which forwards a command line
```

The nearest command that states a preference wins, then the spec, then `value`.
Unlike [`effect`](#effect), this is inherited: it describes how a command line is
read rather than what a command does. See
[the argv grammar](../argv.md#unrecognized-flags) for the reasoning and the cost.

### External subcommands

An unmatched word that names no subcommand can be forwarded as an external
command plus the rest of argv. This is clap's `allow_external_subcommands`, not
`unknown_flags=value`: known subcommands still win, a `default_subcommand` still
catches first, and a flag-like token on the parent is still an unknown flag.

```kdl
unknown_flags "error"
external_subcommand #true            // the whole CLI
cmd "exec" external_subcommand=#true // or one command
```

`ex git --help` forwards `git --help`. `ex --wat` is still an error. Once the
unmatched word is taken, remaining tokens — including `--help` — are not parsed
as this command's flags.

See [the argv grammar](../argv.md#external-subcommands).

### Inferred prefixes

A command can accept an unambiguous prefix of a subcommand name or alias, a long
flag, or a long flag alias:

```kdl
infer_subcommands #true
infer_long_args #true

cmd "install" {
  alias "add"
}
flag "--verbose"
```

Here `mycli insta`, `mycli a`, and `mycli --verb` resolve to their full
declarations. A prefix matching two different commands or flags is not accepted;
an exact spelling always wins. Both settings are inherited by nested commands and
may also be enabled for one subtree with `infer_subcommands=#true` or
`infer_long_args=#true` on its `cmd` node.

### Global flags and mounted commands

A mounted command describes a different program, so the flags of the commands it is mounted under
are not part of it. Once a mounted command is reached, `global` flags declared above it are no
longer offered in completions, and a flag the mounted command declares itself takes precedence over
a global of the same name:

```kdl
flag "-E --env <ENV>" global=#true
cmd "run" {
	mount run="mycli mount-usage-tasks"
}
```

```kdl
# emitted by the mount
cmd "task1" {
  flag "--env <name>" {
    choices "dev" "stage" "prod"
  }
}
```

`mycli run task1 --<tab>` offers only `task1`'s own flags, and `mycli run task1 --env <tab>` offers
`dev stage prod` rather than the global's `<ENV>` value. Global flags are still recognized _before_
the mounted command (`mycli --env prod run task1`), where they also propagate to the mount command
itself.
