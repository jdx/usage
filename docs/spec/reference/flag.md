# `flag`

```kdl
flag "-u --user <user>" // one way to define a flag
flag "--user" { // another way to define the same flag
  alias "-u"
  arg "<user>"
}
flag "--user" { alias "-u" hide=#true } // hide alias from docs and completions

flag "-f --force" global=#true          // global can be set on any subcommand
flag "--file <file>" default="file.txt" // default value for flag
flag "-v --verbose" count=#true         // instead of true/false $usage_verbose is # of times
                                        // flag was used (e.g. -vvv = 3)

flag "--include <pattern>" var=#true            // flag can be repeated (--include a --include b)
flag "--include... <pattern>"                   // same as above, ellipsis on flag
flag "--include <pattern>..."                   // arg is variadic (--include a b c in one invocation)
flag "--include <pattern>" var=#true var_min=1  // at least 1 value required
flag "--include <pattern>" var=#true var_max=5  // up to 5 values allowed

flag "--color" negate="--no-color" default=#true // $usage_color=#true by default
                                                 // --no-color will set $usage_color=#false

flag "--color" env="MYCLI_COLOR" // flag can be backed by an env var

flag "--file <file>"  // args named "<file>" will be completed as files
flag "--dir <dir>"    // args named "<dir>" will be completed as directories

flag "--file <file>" required_if="--dir"     // if --dir is set, --file must also be set
flag "--file <file>" required_unless="--dir" // either --file or --dir must be present
flag "--file <file>" overrides="--stdin" // --file and --stdin override each other; the last one wins
flag "--file <file>" conflicts="--stdin" // --file and --stdin cannot be given together

flag "--stdin" {
  conflicts "--file" "--url" // several, one per argument
}

flag "--shell <shell>" {
  choices "bash" "zsh" "fish" // <shell> must be one of the choices
}

flag "--env <env>" {
  choices env="DEPLOY_ENVS" // values from $DEPLOY_ENVS, split on commas and/or whitespace
}

flag "--filter <pattern>" help_heading="Filtering" // group this flag under a heading in help output

flag "--file <file>" long_help="longer help for --help (as opposed to -h)"
// this is equivalent to the above but preferred when a lot of space is needed
flag "--file <file>" {
  long_help #"""
    longer help for --help (as opposed to -h)
    even
    more
    text
    """#
}
```

## `conflicts` and `overrides`

Both describe a pair of flags that should not be in effect at once, and they differ in
what to do about it. `overrides` resolves the collision — the last one given wins, which
is what you want for `--color`/`--no-color`, where a later flag is a correction. `conflicts`
reports it, for flags whose combination has no sensible meaning at all: giving both is a
mistake, and silently honouring one of them hides it.

A conflict holds in either direction, so declaring it once is enough; it applies to flags
that were actually given, not to defaults.

## `global`

A `global` flag is recognized by the command that declares it and by everything below it, so
`mycli --verbose run task` and `mycli run task --verbose` both work. It is also passed to any
[`mount`](/spec/reference/cmd#mounting-dynamic-commands) reached after it, so the mount command
can take it into account, and it is not offered inside a mounted command — see
[Global flags and mounted commands](/spec/reference/cmd#global-flags-and-mounted-commands).

A non-global flag belongs to the command that declares it, but may still appear before one of
that command's subcommands (`mycli run --force task`): it is parsed there, just not inherited
and not passed to mounts.
