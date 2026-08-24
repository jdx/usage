# `flag`

Flags can be removed from every help page and completion with `hide`. Help
annotations and page variants can also be hidden independently without changing
parsing or fallback behavior. Flags accept `hide_default_value`, `hide_env`,
`hide_env_values`, `hide_possible_values`, `hide_short_help`, and
`hide_long_help`. Usage help prints an environment variable's name but never its
current value, so `hide_env_values` is preserved for clap compatibility without
changing today's rendered output.

```kdl
flag "-u --user <user>" // one way to define a flag
flag "--user" { // another way to define the same flag
  alias "-u"
  arg "<user>"
}
flag "--user" { alias "-u" hide=#true } // hide alias from docs and completions

flag "-f --force" global=#true          // global can be set on any subcommand
flag "--force" display_order=10         // explicit order within its help section
flag "--config <file>" required=#true   // invocation must provide a value
flag "--clear" effect="destructive"     // raises the command effect when supplied
flag "--file <file>" default="file.txt" // default value for flag
flag "-v --verbose" count=#true         // instead of true/false $usage_verbose is # of times
                                        // flag was used (e.g. -vvv = 3)
flag "--assist" action="help_short" help="Show concise help"
flag "--manual" action="help_long" help="Show full help"
flag "--help-all" action="help_all" help="Show help for every command"
flag "--release" action="version" help="Print version"
flag "-h --help" action="help" builtin=#true // parser-supplied, materialized by a generator

flag "--include <pattern>" var=#true            // flag can be repeated (--include a --include b)
flag "--include... <pattern>"                   // same as above, ellipsis on flag
flag "--include <pattern>..."                   // arg is variadic (--include a b c in one invocation)
flag "--include <pattern>" var=#true var_min=1  // at least 1 value required
flag "--include <pattern>" var=#true var_max=5  // up to 5 values allowed
flag "--range <start> <end>"                    // one occurrence takes exactly two values

flag "--color" negate="--no-color" default=#true // $usage_color=#true by default
                                                 // --no-color will set $usage_color=#false

flag "--color" env="MYCLI_COLOR" // flag can be backed by an env var

flag "--file <file>"  // args named "<file>" will be completed as files
flag "--dir <dir>"    // args named "<dir>" will be completed as directories

flag "--file <file>" required_if="--dir"     // if --dir is set, --file must also be set
flag "--file <file>" required_unless="--dir" // either --file or --dir must be present
flag "--token <token>" {
  required_if_eq "--mode" "remote"
}
flag "--approval <approval>" {
  required_if_eq_all "--mode" "remote" "--scope" "global"
}
flag "--checksum <checksum>" {
  required_unless_all "--stdin" "--file"
}
flag "--file <file>" overrides="--stdin" // --file and --stdin override each other; the last one wins
flag "--file <file>" conflicts="--stdin" // --file and --stdin cannot be given together
flag "--out <path>" requires="--format"  // giving --out means --format must be given too
flag "--config <file>" {
  requires_if "special.toml" "--key" // this value also needs --key
}
flag "--dump" exclusive=#true            // --dump has to be given on its own
flag "--tags <tag>" var=#true delimiter="," // --tags a,b,c is three values
flag "--args <ARGS>" allow_hyphen_values=#true // --args -destroy binds "-destroy"
flag "--jobs <N>" allow_negative_numbers=#true // --jobs -1 binds "-1"
flag "--item <ITEM>" var=#true value_terminator=";" // ; ends this occurrence
flag "--inspect <PORT>" require_equals=#true   // --inspect=9229 yes, --inspect 9229 no
flag "--color <WHEN>" default_missing="always" // --color is always; --color=never is never
flag "--bump [LEVEL]" value_optional=#true      // absent, bare, and valued are distinct
flag "--color" bool_value=#true                 // --color=false is explicit false
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

flag "--backend <backend>" {
  choices strict=#false "core" "git" // suggest these, but accept other values
}

flag "--port <port>" {
  arg "<port>" validate="int(value) >= 1 && int(value) <= 65535" validate_error="must be a valid port"
}

flag "--env <env>" {
  choices env="DEPLOY_ENVS" // values from $DEPLOY_ENVS, split on commas and/or whitespace
  // note: `choices env=` requires the `unstable_choices_env` cargo feature of
  // usage-lib; the usage CLI enables it, but library consumers must opt in
}

// argv wins, then APP_TOKEN, then the fallbacks from left to right, then the
// explicitly deprecated alias. The list nodes preserve declaration order.
flag "--token <token>" env="APP_TOKEN" {
  env_fallback "APP_AUTH_TOKEN" "TOOL_TOKEN"
  deprecated_env "OLD_APP_TOKEN"
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
  note "Paths are resolved relative to the working directory."
  warning "An existing file will be replaced."
}
```

`note` and `warning` are semantic callouts. Long terminal help renders labeled
indented blocks, while generated Markdown renders portable labeled blockquotes.

## `deprecated`

`deprecated="use --out"` says a flag still works and should not be used any more. It shows
in help and in completion descriptions, and a parse reports it when the flag is actually
given — see [Warnings](/spec/argv#warnings) for what counts as a use and what a CLI does
with the report.

`deprecated_warn_at` and `deprecated_remove_at` name releases. The first withholds the
warning until the CLI's own `version` reaches it, which is how a deprecation is declared
before it starts nagging; the second is only ever mentioned in the message.

```kdl
flag "--output <output>" deprecated="use --out" deprecated_remove_at="3.0.0"
flag "--legacy" deprecated="use --modern" deprecated_warn_at="2027.1.0"
```

A `deprecated_env` alias is the same idea for one of the names a value may arrive through:
the value is still read, and the variable that supplied it is reported along with the
current name to use instead.

## `conflicts` and `overrides`

Both describe a pair of flags that should not be in effect at once, and they differ in
what to do about it. `overrides` resolves the collision — the last one given wins, which
is what you want for `--color`/`--no-color`, where a later flag is a correction. `conflicts`
reports it, for flags whose combination has no sensible meaning at all: giving both is a
mistake, and silently honouring one of them hides it.

A conflict holds in either direction, so declaring it once is enough; it applies to flags
that were actually given, not to defaults.

Losing an override unsets a flag; it does not unsay the word it was given. `--log-level=v
--trace` is still refused when `v` is outside `--log-level`'s
[`choices`](/spec/reference/arg) — which
of the two is in effect is what `overrides` settles, and only a later occurrence of
`--log-level` itself replaces the word it was handed.

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

`required_if_eq SELECTOR VALUE` makes the target required when an explicitly supplied
value matches. Repeat the node for “any”; `required_if_eq_all` takes selector/value pairs
and activates only when every pair matches. `required_unless` is waived by any named
argument, while `required_unless_all` is waived only when all of them are present.

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

Which condition fired, or why none did, is what
[`usage explain`](/cli/reference/explain) reports — along with the same answer for
`env`, `default` and [`default_missing`](#default_missing).

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

## `allow_negative_numbers`

The next token may be a negative integer or decimal without making every dash-prefixed
word a value. `--jobs -1` binds `-1`, while `--jobs --force` still leaves `--force` to
normal flag parsing. clap spells this `allow_negative_numbers`, and generated specs
preserve both its argument-level and command-level forms.

## `value_terminator`

A variadic flag stops collecting when this exact token appears, and the token itself is
not stored. In `--item one two ; target`, the flag gets `one` and `two`; parsing resumes
with `target`. clap's `value_terminator` getter is preserved by generated specs.

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

## `bool_value`

A boolean switch can opt into explicit attached values. With
`flag "--color" bool_value=#true`, `--color`, `--color=true`, and
`--color=false` bind true, true, and false respectively. Only the long `=` form
is added: `--color false` leaves `false` for a positional, so no following word
changes role and existing switches remain unchanged unless they opt in.

If the flag also has `negate="--no-color"`, the explicit value applies to the
spelling: `--no-color=false` negates false and therefore binds true. Values other
than the exact words `true` and `false` are rejected. Value-taking and count
flags cannot declare `bool_value`.

## `value_optional`

A value-taking flag may be present without a value. This is executable parser
policy: `flag "--bump [LEVEL]" value_optional=#true` distinguishes an absent
flag, a bare `--bump`, and `--bump=major`. The nested argument's square brackets
remain presentational on their own, so a spec can render `[LEVEL]` without
silently changing what argv accepts.

A flag that takes no value cannot declare it.

Detached optional values are easy to misread because the next word may remain a
positional. Prefer a concrete `default_missing` together with `require_equals`
when a bare flag should mean a default and explicit values can use
`--flag=value`.

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

## Markdown help and effects

`help_md` supplies Markdown directly to generated Markdown documentation:

```kdl
flag "--output <path>" help="Output path" {
  help_md "Write the result to **this path**."
  warning "An existing file will be replaced."
  effect "write"
}
```

Without `help_md`, generated Markdown falls back to `long_help` and then
`help`. An `effect` raises the selected command's declared effect when this flag
is supplied; see [command effects](/spec/#command-effects).
