# `arg`

```kdl
arg "<file>"                             // positional arg, completed as a filename
arg "<dir>"                              // positional arg, completed as a directory
arg "[file]"                             // optional positional arg
arg "<file>" default="file.txt"          // default value for arg
arg "<file>" env="MY_FILE"               // arg can be backed by an env var
arg "<file>" parse="mycli parse-file {}" // parse arg value with external command
arg "<port>" validate="int(value) >= 1 && int(value) <= 65535" validate_error="must be a valid port"

arg "[file]" var=#true // multiple args can be passed (e.g. mycli file1 file2 file3) (0 or more)
arg "<file>" var=#true // multiple args can be passed (e.g. mycli file1 file2 file3) (1 or more)
arg "<file>..."        // shorthand for var=#true (trailing ellipsis)
arg "<file>" var=#true var_min=3 // at least 3 args must be passed
arg "<file>" var=#true var_max=3 // up to 3 args can be passed
```

`validate` is an [expr](https://expr-lang.org/) expression evaluated once for each
value after defaults and environment fallbacks are applied. The only variable is
`value`, always a string. The expression must return a boolean; `false` reports
`validate_error`, or a generic validation error when it is omitted. Because the
expression is stored in the spec, generated Rust and Go parsers enforce the same rule.

## Using Variadic Args in Bash

When using variadic arguments (`var=#true`), the values are passed as a shell-escaped
string via the `usage_<name>` environment variable. To properly handle arguments
containing spaces as a bash array, wrap the variable in parentheses:

```bash
# Given: usage_files="arg1 'arg with space' arg3"

# Convert to bash array:
eval "files=($usage_files)"

# Now use as array:
for f in "${files[@]}"; do
  echo "Processing: $f"
done

# Or pass to commands:
touch "${files[@]}"
```

This pattern ensures arguments with spaces are handled correctly as separate elements.

```kdl
arg "<shell>" {
  choices "bash" "zsh" "fish" // <shell> must be one of the choices
}

arg "<env>" {
  choices env="DEPLOY_ENVS" // values from $DEPLOY_ENVS, split on commas and/or whitespace
}

// Rich choices keep clap PossibleValue metadata portable in KDL.
arg "<color>" {
  choices ignore_case=#true {
    choice "always" help="Always use color" {
      alias "yes"
      alias "on" hide=#true
    }
    choice "never" hide=#true
  }
}

arg "<file>" help_heading="Input" // group this arg under a heading in help output

arg "<file>" long_help="longer help for --help (as oppoosed to -h)"

// double-dash behavior
arg "<file>" double_dash="required" // arg only accepts values after a double dash; `mycli file.txt` is an error, `mycli -- file.txt` is not
arg "<-- file>"                     // shorthand for double_dash="required" (also `arg "[-- file]"`, `arg "<-- files>..."`)
arg "<file>" double_dash="optional" // arg may be passed after a double dash (e.g. mycli -- file.txt or mycli file.txt) — the default
arg "<file>..." double_dash="automatic" // once arg is passed, behave as if a double dash was passed (e.g. mycli file.txt --filewithdash)
arg "<args>..." double_dash="preserve" // preserve double dashes as args (e.g. mycli arg1 -- arg2 -- arg3)
```

## Double-Dash Behavior

`double_dash="required"` is enforced while parsing, so the three points below are
what a caller of your CLI actually sees.

**The arg is unreachable until a `--` is typed.** A word offered to it beforehand is
rejected with ``Argument <file> can only be set after a `--` separator``, and the value
is not assigned. A variadic arg reports this once, not once per word, and an arg that
is both `required` and `double_dash="required"` reports only this — not also
`Missing required arg`.

**Everything after the `--` is routed to that arg**, jumping past earlier args —
including a greedy variadic that would otherwise swallow the rest. This matches
clap's [`Arg::last(true)`](https://docs.rs/clap/latest/clap/struct.Arg.html#method.last),
which is what specs generated from clap map to `double_dash="required"`.

```kdl
arg "[tool]..."
arg "[-- command]..."
// mycli node@20 -- node app.js  =>  tool=[node@20], command=[node, app.js]
// mycli -- ls                   =>  tool=[],        command=[ls]
```

**Flag parsing stops at the `--`**, as usual, so `mycli -- --verbose` gives the arg the
literal string `--verbose` rather than setting a `--verbose` flag.

Two interactions are worth calling out:

- A `--` that `double_dash="preserve"` keeps as a value is a _value_, not a separator.
  It does not unlock a `double_dash="required"` arg — one token cannot be both.
- A command's `restart_token` starts a fresh invocation, which resets the separator.
  Each invocation after the token needs its own `--`.
