# Configuration resolution

A [`config` block](/spec/reference/config) says what a CLI's settings are. This page
says how a value is arrived at: which place wins when several have something to say,
what a declared type does to a value on its way in, which places are allowed to set
which settings, and what a resolution has to report about the values it refused.

It exists because that behavior was previously written down nowhere. Every CLI in the
jdx fleet resolved its own settings by hand, and every copy differed — not through
carelessness, but because the declaration of a setting and the code that resolved it
were two separate things kept in step by hand. The rules here are normative, and
[the conformance corpus](#the-conformance-corpus) makes them executable.

## Terms

A **setting** is one entry in the registry: a key, a type, and what the spec says
about it. Keys are dotted paths — `task.output` — and dots are the only nesting.

A **layer** is one place values come from: the command line, the environment, a
config file, a git config, a `.npmrc`. A layer produces **entries**, each one a
setting and a value it supplies.

A **resolution** is the result of merging every layer: a value per setting, the
**origin** each value came from, and the **warnings** the merge produced.

An **origin** names the exact place, not the kind of place: `MISE_JOBS`, `--jobs`,
`./hk.toml`, `git hk.jobs`. "The environment" is not an origin, because a user who
wants to change the answer needs to know which variable to unset.

## Precedence

Layers are ordered, and the order is not negotiable:

1. the command line
2. the environment
3. config files, nearest first
4. anything else the CLI puts below them
5. the declared default

Which layers a CLI _has_ is its own business. Their relative order is not: a
resolution where a project's file outranks the flag the user just typed is a bug,
not a configuration choice.

A layer that supplies nothing for a setting is not a layer that clears it. A higher
layer with no value leaves the lower one alone.

A declared default is the **bottom layer**, not a floor applied afterwards. The
difference shows up in [merging](#merging): a `union` list with a default and one
file gets both, because the default took part in the merge rather than filling in
what nothing had set.

## Merging

Each setting declares how values from several layers combine.

| policy              | what happens                                                    |
| ------------------- | --------------------------------------------------------------- |
| `replace` (default) | the highest layer wins outright                                 |
| `union`             | the items from every layer, lowest first                        |
| `deep`              | tables merged key by key, the higher layer winning a shared key |

`union` concatenates: a `list` keeps duplicates, because that is what distinguishes
a list from a `set`, and a `set` drops them — within one layer's value as much as
across two. First occurrence keeps its position.

An empty value is a value. `HK_EXCLUDE=` is how a user turns a declared default
off, and a union then has nothing left to add to.

## Types

Every layer that reads text — an environment, a git config, an `.npmrc` — hands over
a string. The declared type is the only thing that says whether `1` is the number
one, the string `1`, or a list of one.

- A bare value for a `list` setting is a list of one, which is what `MISE_ENV=production`
  relies on. An empty string is _no_ items rather than one empty item.
- Booleans read `true`/`1`/`yes`/`y`/`on` and `false`/`0`/`no`/`n`/`off`/empty.
  Deliberately not "anything non-empty is true": `FOO=false` meaning true is the
  kind of surprise a config system exists to prevent.
- A number written where text is expected is text that happens to look like a
  number. A _collection_ written there is not: rendering one would produce a value
  nobody wrote.
- `duration` and `url` are text here. What makes a string a URL is what the CLI does
  with it, and the crate that owns the duration type owns its spelling.
- A union, or a type name usage does not know, coerces nothing at all: the spec has
  said usage cannot decide what belongs there.

A **named parser** splits one string into several values before the type reads it —
`list_by_comma`, `list_by_colon`, `list_by_os_path_separator`, `set_by_comma` — so no
layer has to know that a setting is comma-separated.

A value the declared type cannot read **costs its own key and nothing else**. A typo
in a system-wide file must not stop a CLI from starting for every user on the
machine.

## Choices

A setting may name the values it takes. Those reach the docs, the JSON schema and
the completions, and they reach resolution too: a CLI that documents three values
and accepts a fourth in silence has lied to its user.

Choices on a collection mean each **item** is one of them, which is the rule
`usage g json-schema` follows by putting the enum on every value position rather
than on the container.

A choice is read as the declared type before it is compared, so `choice "yes"` under
`type="bool"` is the same value as `#true`. The type is asked first: a value that is
not a boolean at all is reported as that, because there is nothing useful to say
about which choice it is not.

## Scope

Which places may set which settings. mise treats this as a security property rather
than a preference: a setting that decides whether code runs must not be settable by
a file a pull request can add.

| scope           | settable from                            |
| --------------- | ---------------------------------------- |
| `any` (default) | anywhere                                 |
| `global`        | not from anything a repository can carry |
| `env`           | never from a file at all                 |

The question the merge asks is how far a place is **trusted**, not what kind of place
it is. A pkl file or a git config inside a checkout is exactly as much a thing a
repository carries as a TOML file is, and a source usage does not recognize gets the
least trusting answer until its layer says otherwise — because a check every new
layer has to remember is one a new layer will forget.

A refusal costs that value alone, and is reported under the name the user wrote.

## Renames

An old name keeps working. An upgrade that silently changed what a machine's config
meant would be worse than the rename itself.

A value written under a renamed key lands on the setting that replaced it, at the
same precedence it was written at, and the user is told what it was read as. The old
name is not a second setting: it has no value of its own, and every read of it
answers about its replacement.

A deprecation notice may sit anywhere along a chain of renames — `a` renamed to `b`,
and `b` the one carrying the notice that says to use `c` — and the first one found is
the one reported.

## Warnings

A resolution reports rather than prints. A library that writes to stderr cannot be
used by anything with an opinion about output, and mise queues its deprecations until
its logging is up.

Each has a **kind**, because the wording is for a person and the kind is what a
program acts on:

| kind              | what happened                                      |
| ----------------- | -------------------------------------------------- |
| `unknown-setting` | a key no setting declares                          |
| `wrong-type`      | a value the declared type cannot read              |
| `not-allowed`     | a value the declared choices do not allow          |
| `out-of-scope`    | a place that may not set this setting              |
| `deprecated`      | a setting whose spec says not to use it            |
| `renamed`         | a value read as the setting that replaced its name |
| `not-read`        | a value passed over because another name won       |

An unknown key is a warning and never an error: a config file written for a newer
binary must still work with an older one.

## The conformance corpus

[`corpus/config/`](https://github.com/jdx/usage/tree/main/corpus/config) pairs a set
of settings with the layers that supply values and the resolution the two must
produce. A second implementation — in Go, in TypeScript, as a second Rust one — passes
them or does not.

Two rules on this page have none, both for the same reason: a vector says what a
resolution produces, and neither of these is one. `list_by_os_path_separator` splits on
a character that depends on the machine running the test, and a corpus whose answers
differ by platform stops being the definition of correct. `not-read` is a layer's choice
between two of its own variables, which a vector describing what a layer _supplies_
cannot express — [`EnvLayer`'s own tests](https://github.com/jdx/usage/blob/main/config/src/env.rs)
hold it instead.

Vectors describe a **registry** rather than a spec, because resolution is a question
about keys and types rather than about KDL, and warnings are pinned by kind rather
than by message: wording is expected to differ between implementations.

```sh
cargo test -p usage-conformance --test config
```

## Implementations

[`usage-config`](https://github.com/jdx/usage/tree/main/config) resolves; it carries
no spec parser, so a CLI ships a resolver rather than a KDL reader.
[`usage-config-build`](https://github.com/jdx/usage/tree/main/config-build) reads the
spec at build time and emits the registry as consts, along with the typed `Settings`
struct a CLI reads — so a setting that is declared is a setting that resolves, with
no second declaration to keep in step.
