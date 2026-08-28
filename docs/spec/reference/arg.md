# `arg`

Arguments may also be classified by a prefix instead of position. See
[sigil arguments](./sigils.md) for matching, boundary, completion, and derive rules.

Positionals accept `hide`, `hide_default_value`, `hide_env`, `hide_env_values`,
`hide_possible_values`, `hide_short_help`, and `hide_long_help`. These affect
help presentation only; defaults, environment fallback, and validation remain
active.

```kdl
arg "<file>"                             // positional arg, completed as a filename
arg "<dir>"                              // positional arg, completed as a directory
arg "[file]"                             // optional positional arg
arg "<file>" default="file.txt"          // default value for arg
arg "<file>" env="MY_FILE"               // arg can be backed by an env var
arg "<file>" display_order=10             // explicit order in help; parse order is unchanged
arg "<port>" validate="int(value) >= 1 && int(value) <= 65535" validate_error="must be a valid port"
arg "<output>" effect="write"             // raises the command effect when supplied
arg "[tool]..." sigil="+"                  // +node@24 stores node@24 without advancing

arg "[file]" var=#true // multiple args can be passed (e.g. mycli file1 file2 file3) (0 or more)
arg "<file>" var=#true // multiple args can be passed (e.g. mycli file1 file2 file3) (1 or more)
arg "<file>..."        // shorthand for var=#true (trailing ellipsis)
arg "<file>" var=#true var_min=3 // at least 3 args must be passed
arg "<file>" var=#true var_max=3 // up to 3 args can be passed
arg "<tag>..." delimiter="," // one word such as a,b becomes two values
arg "<start> <end>" var_min=2 var_max=2 // exactly two values with distinct labels
arg "<number>" allow_negative_numbers=#true // -1 is a value, --force is still flag-like
arg "<item>..." value_terminator=";" // stop before ; without storing it
arg "[request]" {
  requires "--mode" "--scope" // when request is present, both flags are needed
}
arg "[token]" {
  required_if_eq "--mode" "remote"
}
arg "[checksum]" {
  required_unless_all "--stdin" "--file"
}
```

`allow_negative_numbers` accepts a leading-minus integer or decimal where a normal
dash-prefixed token would be treated as a flag. `value_terminator` is valid only on a
variadic argument; it ends that argument without binding the terminator token.

`delimiter` splits one word into several values before choices and validation
are applied, so it is valid only on a variadic argument.

Several placeholders in one argument declare fixed arity. Each label is retained in
help and generated Rust and Go tables; `var_min` and `var_max` must match the number
of placeholders.

The labels can also be spelled out with a `value_names` child node, which takes one
or more strings:

```kdl
arg "<range>" {
  value_names "START" "END" // same as arg "<START> <END>"
}

arg "<ITEM>..." var=#true var_min=2 var_max=2 {
  value_names "ITEM" // one label, shown as <ITEM> <ITEM>
}
```

The first name replaces the argument's own label. More than one name declares fixed
arity: `var_min` and `var_max` default to the number of names and must equal it when
given. A single name relabels the values of an exact-arity variadic.

Positionals accept the same post-parse relationship nodes as flags: `conflicts`,
`requires`, `required_if`, `required_if_eq`, `required_if_eq_all`, `required_unless`,
and `required_unless_all`. Bare selectors name positionals and dashed selectors name
flags.

`validate` is an [expr](https://expr-lang.org/) expression evaluated once for each
value after defaults and environment fallbacks are applied. The only variable is
`value`, always a string. The expression must return a boolean; `false` reports
`validate_error`, or a generic validation error when it is omitted. Because the
expression is stored in the spec, generated Rust and Go parsers enforce the same rule.

The Rust runtime compiles the operators, the string, array and numeric builtins, and
`matches`. The date, JSON and base64 builtins are left out — twelve crates for something a
`validate=` rule rarely reaches, since the only variable is a string. An expression that
uses one fails when a value reaches it, naming the feature to add; a CLI that wants them
adds it to its own manifest:

```toml
expr-lang = { version = "2.1", features = ["temporal"] }
```

## Environment sources

`env` is the canonical variable. `env_fallback` names additional variables in
precedence order, and `deprecated_env` names compatibility aliases consulted
last and reported as deprecated:

```kdl
arg "<profile>" env="MYCLI_PROFILE" {
  env_fallback "MYCLI_ENV" "PROFILE"
  deprecated_env "OLD_PROFILE"
}
```

Command-line values win over every environment source. A value from an
environment variable wins over `default`.

## Markdown help and effects

`help_md` supplies Markdown directly to generated Markdown documentation:

```kdl
arg "<output>" help="Output path" {
  help_md "Write the result to **this path**."
  note "Relative paths use the working directory."
  warning "An existing file will be replaced."
  effect "write"
}
```

Without `help_md`, generated Markdown falls back to `long_help` and then
`help`. `note` and `warning` render as labeled blocks in long terminal help and
as portable blockquotes in generated Markdown. An `effect` raises the selected command's declared effect when this
positional is supplied; see [command effects](/spec/#command-effects).

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
  // note: `choices env=` requires the `unstable_choices_env` cargo feature of
  // usage-lib; the usage CLI enables it, but library consumers must opt in
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

// Keep known values in help and completion while accepting others.
arg "<backend>" {
  choices strict=#false "core" "git"
}

arg "<file>" help_heading="Input" // group this arg under a heading in help output

arg "<file>" long_help="longer help for --help (as opposed to -h)"

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

Choices are strict by default. Set `strict=#false` on `choices` to keep the
declared values in help and completion while accepting values outside the list.
