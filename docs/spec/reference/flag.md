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
flag "--out <path>" requires="--format"  // giving --out means --format must be given too
flag "--config <file>" {
  requires_if "special.toml" "--key" // this value also needs --key
}
flag "--dump" exclusive=#true            // --dump has to be given on its own
flag "--tags <tag>" var=#true delimiter="," // --tags a,b,c is three values
flag "--args <ARGS>" allow_hyphen_values=#true // --args -destroy binds "-destroy"
flag "--inspect <PORT>" require_equals=#true   // --inspect=9229 yes, --inspect 9229 no
flag "--color <WHEN>" default_missing="always" // --color is always; --color=never is never
flag "--bin-names" {
  default_if "--json" "true" // --json implies --bin-names
}

flag "--stdin" {
  conflicts "--file" "--url" // several, one per argument
}

flag "--sign" {
  requires "--key" "--identity" // several, and all of them are needed
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

For three or more flags that exclude each other, or for "one of these is required", a
[`group`](/spec/reference/group) says in one node what `conflicts` says once per pair.

## `requires`

The positive form: giving this flag means the flags it names must be given too. Every
selector has to be satisfied, so `requires "--key" "--identity"` means both. Nothing
happens when the declaring flag is absent — a requirement is a consequence of using the
flag, not a rule about the command line as a whole.

`required_if` says the same thing from the other end, and both exist because they are
written in different places. `--out` needing `--format` is `requires="--format"` on
`--out`, or `required_if="--out"` on `--format`; the first keeps the rule on the flag it
is about, which is usually where a reader looks for it.

A value from the environment or a default satisfies a requirement, on the same principle
`conflicts` follows: the question is whether the other flag ended up with a value, not
how it got one.

`requires_if VALUE FLAG` makes the requirement conditional on the declaring flag's
value. The node may be repeated when different values require different flags:

```kdl
flag "--config <file>" {
  requires_if "special.toml" "--key"
  requires_if "remote.toml" "--token"
}
```

Command-line and environment values activate a conditional requirement; a default does
not. If a flag collects several values, any matching value activates its requirement.
This is the same source distinction clap's `requires_if` and `requires_ifs` make.

::: warning
A spec generated from a clap command never carries this. clap has `Arg::requires` as a
setter with no getter, so [the clap integration](/spec/integrations/clap) cannot read it
back out — a CLI that wants the constraint in its spec has to declare it here.
:::

## `default_if`

A default that depends on another flag. Lives on the _target_ — the flag that
gets the value — which is the inverse of [`requires_if`](#requires_if):

```kdl
flag "--bin-names" {
  default_if "--json" "true"            // --json is enough
  default_if "--output" "json" "pretty" // --output json binds pretty
}
```

Two arguments are clap's `ArgPredicate::IsPresent`; three are `Equals`. The node
may be repeated; the first matching condition wins.

Only considered when this flag was not on the command line and has no environment
value. An applied `default_if` is a default, not an explicit value: it satisfies
[`requires`](#requires) and does not activate `requires_if`.

::: warning
A spec generated from a clap command never carries this. clap has
`Arg::default_value_if` as a setter with no getter, so
[the clap integration](/spec/integrations/clap) cannot read it back out — the
same hole as `requires`.
:::

## `exclusive`

An exclusive flag has to be given on its own: everything else the command declares is
refused alongside it, **including its positional arguments**, which is what makes this
more than [`conflicts`](#conflicts-and-overrides) with every other flag — a conflict has
nowhere to name an argument.

`--version` and `--dump-config` are the shape it is for: asking for one means the rest of
the command line has nothing to act on.

Only what was supplied counts, the same rule `conflicts` follows. A flag with a
[`default`](/spec/reference/flag) standing beside an exclusive one is nobody saying
anything, and counting it would make the exclusive flag unusable on any command that has
a default.

## `delimiter`

One word, several values: `--tags a,b,c` is three. clap spells this `value_delimiter`,
and a spec generated from a clap command carries it.

The split happens during the parse, before anything judges what it produced, so
[`choices`](/spec/reference/arg) is asked about each value rather than about the word
that carried them, and `var_min`/`var_max` count the values the user meant rather than
the words they typed.

A delimiter needs somewhere to put what it splits, so it goes with `var=#true` — on a
single-value flag everything after the first separator would be dropped, and that is
refused where it is written rather than at a prompt.

## `allow_hyphen_values`

A flag's detached value may look like a flag: `--args -destroy` binds `-destroy`
instead of reading `-d` as a short. clap spells this `allow_hyphen_values`, and a spec
generated from a clap command carries it.

The attached form already binds a dash-prefixed value (`--args=-destroy`), so this is
only about the following word. A `--` is flag-like too, so a flag declared this way
takes the separator as its value rather than ending flag interpretation —
`ex -a -- -x` binds `--` and leaves `-x` for whatever follows. A variadic occurrence
still stops collecting at a later flag-like token, so a second occurrence of the same
flag is not eaten as a value.

A flag that takes no value cannot declare it: there is nothing to take.

## `require_equals`

The value must be attached with `=`: `--inspect=9229` binds and `--inspect 9229`
is a missing value. clap spells this `require_equals`, and a spec generated from
a clap command carries it — aube's `--inspect` / `--inspect-brk` are the fleet
case.

A short's attached form still binds (`-i9229`, `-i=9229`); only the following
word is refused. Combined with [`allow_hyphen_values`](#allow_hyphen_values),
the attached form can still pass a dash-prefixed value (`--args=--force`);
the detached form stays refused.

A flag that takes no value cannot declare it.

## `default_missing`

The value used when the flag is given with none: `--color` binds `always` if the
spec says `default_missing="always"`. `--color=never` and (unless
[`require_equals`](#require_equals)) `--color never` still bind the word that was
typed. A following flag-like token is not taken as the value.

clap spells this `default_missing_value`. clap 4 has the setter and no getter, so
a spec generated from a clap command never carries it — the same hole as
[`requires`](#requires). A rewrite that declares in usage keeps it. Combined with
`require_equals`, a following word is a positional rather than the value, which
is aube's `--color` / `--inspect` shape.

A flag that takes no value cannot declare it.

## `global`

A `global` flag is recognized by the command that declares it and by everything below it, so
`mycli --verbose run task` and `mycli run task --verbose` both work. It is also passed to any
[`mount`](/spec/reference/cmd#mounting-dynamic-commands) reached after it, so the mount command
can take it into account, and it is not offered inside a mounted command — see
[Global flags and mounted commands](/spec/reference/cmd#global-flags-and-mounted-commands).

A non-global flag belongs to the command that declares it, but may still appear before one of
that command's subcommands (`mycli run --force task`): it is parsed there, just not inherited
and not passed to mounts.
