# The argv grammar

A usage spec says what a CLI accepts. This page says how a command line is
matched against it: which token binds to which flag or argument, when a word
selects a subcommand, and what counts as an error.

It exists because that behavior was previously defined only by
[usage-lib](https://github.com/jdx/usage/tree/main/lib)'s implementation. A
second implementation — in Rust, in Go, in a shell completion — had no way to
know whether it agreed, and no way to prove it. The grammar here is normative,
and the [conformance corpus](#the-conformance-corpus) makes it executable.

::: warning Being written down changes nothing on its own

usage-lib does not implement every rule below, and the differences are recorded
per case in the corpus rather than smoothed over. See
[Where the reference implementation differs](#where-the-reference-implementation-differs).

:::

## Terms

A **token** is one element of `argv`, after the shell has finished with it. The
grammar never re-splits a token on whitespace: quoting is the shell's job and is
already done.

A **command line** is the tokens after the program name. `mycli install -f x`
has three.

A token is **flag-like** when it begins with `-`, is longer than one character,
and is not a negative number. So `--force`, `-f`, and `-abc` are flag-like; `-`,
`-1`, `-2.5`, and `-1e5` are not.

A number here means digits, at most one `.`, and optionally an exponent — `e` or
`E`, an optional `+` or `-`, then at least one digit. So `-1`, `-2.5`, `-1e5`, and
`-1.5e-3` are values. It is deliberately narrower than what a float parser
accepts: `-inf` and `-NaN` parse as floats but are far likelier to be misspelled
flags than numbers somebody meant to pass. `-1x` and `-1e` are not numbers either,
and so name flags that do not exist.

## Reading a command line

Tokens are read once, left to right. There is no backtracking, no reordering, and
no second pass: what a token binds to is decided when it is read, from the
command in scope at that moment. This is what makes the grammar implementable as
a single loop, and it is also why a `--` or a subcommand word changes the meaning
of everything after it but nothing before it.

At each token, in order:

1. If flag interpretation has stopped (a `--` was consumed), the token is a
   value.
2. If the token is exactly `--`, flag interpretation stops. The token is consumed
   and is not itself a value.
3. If the token is flag-like, it is matched as a flag ([long](#long-flags) or
   [short](#short-flags)). If nothing matches, see
   [unrecognized flags](#unrecognized-flags).
4. Otherwise the token is a word: it selects a [subcommand](#subcommands) if one
   matches, and is otherwise offered to the command's
   [positional arguments](#positional-arguments).

## Long flags

A token beginning with `--` is a long flag. The name is the text up to the first
`=`, or the whole token if there is none.

**Names match exactly.** `--for` does not match `--force`. Abbreviation
inference is deliberately absent: it makes adding a flag a breaking change for
anyone who typed a prefix that was unique until the new flag arrived. A prefix
that matches nothing is then an [unrecognized flag](#unrecognized-flags).

For a flag that takes a value, the value comes from one of two forms:

| form       | value                                                 |
| ---------- | ----------------------------------------------------- |
| `--jobs=8` | the text after the first `=`, so `--set=a=b` is `a=b` |
| `--jobs 8` | the following token                                   |

`--jobs=` binds the empty string. Present-but-empty is a distinct state from
absent, and the attached form is the only way to express it.

A detached value **must not be flag-like**. `--jobs --force` is a missing value,
not a `jobs` of `"--force"`, because the overwhelmingly likely reading is that
the value was forgotten. To pass a value that begins with a dash, attach it:
`--jobs=--force`. The negative-number exception means `--offset -1` still works.

A flag needing a value that ends the command line is an error.

### Flags that take several values

A flag whose argument is variadic (`--include <pattern>...`) keeps taking values
from one occurrence: it consumes following tokens until one is flag-like, or a
`--` arrives, or the command line ends. So `--include a b` gives it both, while
`--include a --force` gives it only `a`.

This is greedy, and a command that declares both a variadic flag and positionals
will find the flag eating them. That is inherent to the feature rather than a
quirk of this grammar; the way to end the run explicitly is `--`.

A flag declared `var` without a variadic argument is the other shape: it takes
one value per occurrence and may be repeated, so `--include a --include b`
collects two. The distinction matters — `--include a b` gives a _repeatable_ flag
only `a`, leaving `b` to a positional, while a _variadic_ one takes both.

## Short flags

A token beginning with a single `-` is one or more short flags. Letters are read
left to right.

A letter whose flag takes no value simply sets it, and reading continues with the
next letter — so `-ab` sets both `a` and `b`.

A letter whose flag takes a value ends the token. Its value is:

| form   | value                                       |
| ------ | ------------------------------------------- |
| `-j8`  | the rest of the token                       |
| `-j=8` | the rest of the token after one leading `=` |
| `-j 8` | the following token                         |

So `-aj8` sets `a` and gives `jobs` the value `8`. The attached form is what
makes `-C/tmp` and `-Edev` work.

One `=` immediately after the letter is a separator, matching the long form. Only
one: `-j==8` is a value of `=8`.

A token containing an unrecognized letter is not a bundle at all, so none of its
letters are applied: `-az` does not set `-a` on the way to discovering that `z`
names nothing. What happens to the token instead is described under
[unrecognized flags](#unrecognized-flags).

`-` alone is not a flag. It is a value, conventionally meaning stdin.

## Unrecognized flags

A flag-like token that names no flag in scope **becomes a word**, and is offered to
the positional arguments like any other. With nothing left to hold it, that is an
`unexpected_arg` — the same error an extra word produces, rather than a special one
about flags.

This is where the grammar parts company with every comparable parser. clap,
argparse, commander, oclif v2+, and POSIX `getopt` all reject the token. They are
right for what they do, which is parse _their own_ argv, where a dash-word can only
be a flag or a typo. A usage spec is also used to parse command lines whose flags it
does not own:

- a shell script run through `usage exec`, forwarding options to a tool it wraps
- a task's arguments, where the task script is the authority on what it accepts
- a completion, asked about a line that is still half-typed

In all three, a token the spec has not heard of is far more likely to be data in
transit than a mistake, and refusing it would break the wrapper for everyone who
did not enumerate the flags of the program behind it.

The cost is real and worth stating plainly: a misspelled `--hekp` becomes an
argument instead of an error, and whether it does depends on whether a positional
is free to take it. A CLI that owns all of its flags can have the stricter reading
by asking:

```kdl
unknown_flags "error"     // for the whole CLI
cmd "exec" unknown_flags="value"   // except here, which forwards a command line
```

Unlike `effect`, this **is** inherited: the nearest enclosing command that states a
preference wins, then the spec, then `value`. It describes how a command line is
read rather than what a command does, and a CLI that forwards options tends to
forward them at every level.

Even when refusing, a lone `-` and a negative number stay values — neither is a
misspelled flag, and without the second `--offset -1` could not be written. oclif
made exactly this mistake when it switched to refusing unknown flags, and had to
add the number case back afterward.

## Positional arguments

A word that does not select a subcommand is offered to the command's arguments in
declaration order. Each argument takes one word, except a variadic (`var=#true`,
or a trailing `...`), which collects every word still available.

- A word offered when no argument can hold it is an error, not silently dropped.
- `var_min` and `var_max` are enforced. They are checks on how many words an argument
  ended up with, not limits that stop it collecting: a bounded variadic still takes
  every word available, so an argument after one is as unreachable as an argument after
  an unbounded variadic.
- An unfilled required argument is an error. An unfilled optional one is absent.

Flags and words may interleave freely: `ex from -f to` fills the same arguments
as `ex -f from to`. A flag between two words does not affect which argument each
word fills.

## Subcommands

A word is matched against the subcommands of the command in scope, by name and by
alias. A match descends: parsing continues against the subcommand, and the
selected path records the canonical name even when an alias was typed.

**A subcommand name wins over a positional value.** A CLI that declares both
cannot receive a positional whose text equals one of its subcommand names — mise
documents exactly this hazard for tasks that share a name with a command.

**Only the descent position routes.** Once a word has been consumed by a
positional argument, a later word matching a subcommand name is just a value.
`ex other install` does not run `install`.

### Flag scope

A flag belongs to the command that declares it and may appear anywhere that
command is in scope, which includes before one of its own subcommand words:
`ex --quiet install` works for a root `--quiet`.

A flag declared `global=#true` is additionally inherited by every command
beneath it, so it may appear after any subcommand word at any depth.

Scope only ever runs downward. A flag declared on a subcommand is not accepted
before that subcommand is reached, `global` or not. A subcommand may redeclare a
name it would otherwise inherit, and below that point its own declaration is the
one that binds — mise does this deliberately, redeclaring several root globals on
`run` with different shorts.

## The `--` separator

A bare `--` stops flag interpretation. Every token after it is a value, however
flag-like it looks. The separator is consumed and is not itself a value.

Only the first `--` is a separator. A later one is an ordinary value, since flag
interpretation has already stopped — which is what lets a CLI forward a command
line that itself contains `--`.

An argument may say more about its relationship to the separator, with
`double_dash`:

| mode        | meaning                                                             |
| ----------- | ------------------------------------------------------------------- |
| `optional`  | the default: values may appear on either side                       |
| `required`  | values are accepted only after a `--`; a word before it is an error |
| `preserve`  | the separator is kept as a value instead of being consumed          |
| `automatic` | once the argument takes a value, behave as if `--` had been given   |

## Values not from argv

When the command line does not supply a value, it is taken from the environment
if the flag or argument declares `env`, and otherwise from its `default`. In
short: **command line, then environment, then default.**

An environment variable set to the empty string is set. Treating empty as unset
would make `EX_JOBS=` mean something no other empty value in the grammar means.

None of this changes which token binds where. It only fills what the command line
left empty, which is why it is described last: an implementation can do all of it
after the single pass is over.

## Errors

The grammar distinguishes these classes of failure. Wording is not specified —
diagnostics are a quality-of-implementation concern and should be much better
than these names — but the class is, so a strict parser and a lenient one can be
told apart mechanically.

| code                       | when                                                      |
| -------------------------- | --------------------------------------------------------- |
| `unknown_flag`             | a flag-like token matched no flag in scope                |
| `missing_flag_value`       | a flag needing a value did not get one                    |
| `missing_required_flag`    | a required flag never appeared                            |
| `missing_required_arg`     | a required argument was never filled                      |
| `unexpected_arg`           | more words than the command can hold                      |
| `invalid_choice`           | a value outside the declared `choices`                    |
| `arg_requires_double_dash` | a `double_dash="required"` argument got a value too early |
| `var_too_few`              | fewer values than `var_min`                               |
| `var_too_many`             | more values than `var_max`                                |
| `conflicting_flags`        | two flags declared to `conflict` were both given          |

Choices match exactly; case-insensitive matching would have to be declared rather
than assumed.

## The conformance corpus

[`corpus/`](https://github.com/jdx/usage/tree/main/corpus) holds the executable
form of this page: JSON vectors pairing a spec and a command line with the
expected result.

```json
{
  "id": "long-value-attached",
  "doc": "`--flag=value` binds the text after the first `=`.",
  "spec": "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\"\n",
  "argv": ["--jobs=8"],
  "expect": { "ok": { "flags": { "jobs": "8" } } }
}
```

Bindings are keyed by the name the spec gives each flag and argument, never by
the token that set them, so `-j`, `--jobs`, and `EX_JOBS` all land under `jobs`.
Values are recorded as strings: the grammar decides which token binds where, not
what it means, so turning `"8"` into a number is the caller's business. Failures
record only the error code.

Vectors that set `env` carry their own environment. The harness never reads the
process environment, so no vector's result can depend on the machine running it.

Any implementation in any language can run these. In this repository,
`cargo test -p usage-conformance` runs them against both usage-lib and
[usage-argv](https://github.com/jdx/usage/tree/main/argv).

Each vector says which layer of a parser it is a question for. Most are
`binding` — which token becomes which flag or argument — and a parser that reads
argv is expected to answer all of those. The rest are `post-binding`: `required`,
`choices`, `env` fallback, defaults, `var_min`/`var_max`, `overrides`, and
`conflicts` are
decided once the last token has been read, and need to know a value's type, so a
binding-only parser leaves them to the layer that owns the target struct.

That is declared per vector rather than worked out from the spec, because an
implementation should be told which vectors apply to it. usage-lib answers every
vector; usage-argv answers the binding ones.

## Where the reference implementation differs

Each vector records whether usage-lib agrees with it, as a measurement rather
than an assumption, and
[`conformance/tests/reference.rs`](https://github.com/jdx/usage/blob/main/conformance/tests/reference.rs)
fails if a label is wrong in either direction. A recorded divergence that gets
fixed shows up as a test failure telling you to delete the label, so the list
cannot rot.

Today usage-lib diverges on 11 of 87 vectors, from three causes:

**Unrecognized flags fall through to positionals.** `ex --wat` binds `--wat` to
an argument if one is free, and reports `unexpected_arg` if not. This accounts
for most of the divergences, including the ones where the grammar and usage-lib
agree a flag is out of scope but disagree about which error to report.

**Repeated `--` is eaten.** A second separator is dropped rather than kept as a
value, so a forwarded command line containing `--` is altered in transit. Note
that `double_dash="preserve"` exists precisely to keep separators, which makes
this arguably a deliberate design choice rather than a bug — see
[Open questions](#open-questions).

Three smaller gaps: `--jobs=` binds nothing rather than the empty string; a flag
with a variadic argument (`--include <pattern>...`, which the
[flag reference](/spec/reference/flag) documents) rejects its second value; and
`double_dash="automatic"` is not enforced, which the
[arg reference](/spec/reference/arg) already says outright.

Where the grammar and usage-lib disagree, the grammar is the intent and the
divergence is a bug to fix or a decision to revisit — not a description of
settled behavior. Each one is a small, self-contained change to
`lib/src/parse.rs`, and the corpus is how a fix gets verified.

## Not yet covered

- **Restart tokens.** `restart_token` (mise's `:::`) makes one command line
  describe several invocations, which the vector format cannot express: `expect`
  holds a single result. Supporting it needs a multi-invocation shape, and until
  then usage-lib's behavior — rewind, and let the last invocation's bindings
  stand — is untested here.
- **Mounts.** `mount` resolves a sub-spec by running a command during the parse.
  That makes a vector depend on an external process, so it needs a stubbing
  mechanism first.
- **Completion parsing.** `parse_partial` deliberately accepts incomplete input
  to drive completions. It is a different contract with different expectations,
  and deserves its own corpus.
