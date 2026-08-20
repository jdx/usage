# `cmd`

```kdl
// aliases
cmd "config" help="Manage the CLI config" {
  alias "cfg" "cf" "cg"  // aliases for the command
  alias "conf" hide=#true // hide alias from docs and completions
}

cmd "config" hide=#true // hide command from docs and completions
cmd "config" subcommand_required=#true // subcommand is not optional
cmd "config" subcommand_help_heading="Actions" subcommand_value_name="ACTION"
cmd "config" next_line_help=#true // put descriptions below each entry
cmd "config" flatten_help=#true // expand visible subcommands into this page
cmd "config" display_order=10 // present before commands with a greater order
cmd "config" help_heading="Configuration" // group under this heading in its parent's help

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

Tests and deterministic generators can opt out of process execution with
`usage::parse::Parser::with_mount_outputs`. The supplied map is keyed by the exact
`run` declaration and must contain every mount the parse reaches; a missing entry
is an error rather than permission to fall back to the host shell.

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

### Require an argument or show help

`arg_required_else_help` makes a bare invocation request the command's short help:

```kdl
arg_required_else_help #true             // for the whole CLI
cmd "run" arg_required_else_help=#true // or one command
```

The policy observes argv belonging to the selected command. A global flag before a
subcommand does not count as that subcommand's argument, and values supplied later by an
environment variable or default do not suppress help.

### Control built-in help and version entry points

Each synthesized entry can be removed independently:

```kdl
disable_help_flag #true
disable_help_subcommand #true
disable_version_flag #true
```

The same properties are accepted on a `cmd` node. A declared flag can then relocate the
behavior with `action="help"`, `help_short`, `help_long`, `help_all`, or `version`; its ordinary `help`
text is what the help page displays.

### Let a subcommand satisfy parent requirements

`subcommand_negates_reqs` makes required arguments, flags, groups, and conditional
requirements on a parent optional when a child is selected:

```kdl
subcommand_negates_reqs #true
arg "<input>"
cmd "inspect"
```

`ex inspect` is valid, while bare `ex` still requires `input`. Conflicts continue to apply,
and the selected child must still satisfy its own requirements.

### Make parent arguments conflict with subcommands

`args_conflicts_with_subcommands` makes arguments on a command and its child
subcommands mutually exclusive. Once that command binds a flag or positional, a
later child name is rejected; arguments after a selected child belong to the
child as usual:

```kdl
args_conflicts_with_subcommands #true
```

### Prefer a known subcommand to a variadic value

`subcommand_precedence_over_arg` lets a known child name end a variadic flag
or positional that would otherwise consume it.

```kdl
subcommand_precedence_over_arg #true
```

### Allow a missing optional positional

`allow_missing_positional` lets a later required positional claim the last
available word while an earlier optional positional remains empty. Without the
opt-in, positional binding remains strictly left to right.

```kdl
allow_missing_positional #true
arg "[optional]"
arg "<required>"
```

### Preserve delimiters in trailing values

`dont_delimit_trailing_values` keeps a positional token whole after `--`, or once a
`double_dash="automatic"` positional begins. The same argument still applies its
`delimiter` before that boundary:

```kdl
dont_delimit_trailing_values #true
arg "<values>..." delimiter=","
```

`ex a,b -- c,d` binds `a`, `b`, and `c,d`. The policy is inherited by
subcommands, matching clap's command-wide setting.

### Repeated scalar flags

Later occurrences of a single-valued flag replace earlier ones by default. Set
`args_override_self` to false on commands that require strict duplicate checking:

```kdl
args_override_self #false
cmd "publish" args_override_self=#false
```

Repeatable, variadic, and count flags continue collecting or counting regardless of this setting.

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
