# `flag`

```sh
flag "-u --user <user>" # one way to define a flag
flag "--user" { # another way to define the same flag
  alias "-u"
  arg "<user>"
}
flag "--user" { alias "-u" hide=#true } # hide alias from docs and completions

flag "-f --force" global=#true           # global can be set on any subcommand
flag "--file <file>" default="file.txt" # default value for flag
flag "-v --verbose" count=#true          # instead of true/false $usage_verbose is # of times
                                        # flag was used (e.g. -vvv = 3)

flag "--include <pattern>" var=#true            # flag can be repeated (--include a --include b)
flag "--include... <pattern>"                   # same as above, ellipsis on flag
flag "--include <pattern>..."                   # arg is variadic (--include a b c in one invocation)
flag "--include <pattern>" var=#true var_min=1  # at least 1 value required
flag "--include <pattern>" var=#true var_max=5  # up to 5 values allowed

flag "--color" negate="--no-color" default=#true  # $usage_color=#true by default
                                                 # --no-color will set $usage_color=#false

flag "--color" env="MYCLI_COLOR" # flag can be backed by an env var
flag "--color" config="ui.color" # flag can be backed by a config file

flag "--file <file>"  # args named "<file>" will be completed as files
flag "--dir <dir>"    # args named "<dir>" will be completed as directories

flag "--file <file>" required_if="--dir"     # if --dir is set, --file must also be set
flag "--file <file>" required_unless="--dir" # either --file or --dir must be present
flag "--file <file>" overrides="--stdin"     # if --file is set, previous --stdin will be ignored

flag "--shell <shell>" {
  choices "bash" "zsh" "fish" # <shell> must be one of the choices
}

flag "--file <file>" long_help="longer help for --help (as oppoosed to -h)"
# this is equivalent to the above but preferred when a lot of space is needed
flag "--file <file>" {
  long_help r#"longer help for --help (as oppoosed to -h)
   even
   more
   text
   "#
}
```
