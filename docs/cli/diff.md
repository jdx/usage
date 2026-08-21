# Comparing two specs

A CLI is a public API, and a spec is the only machine-readable statement of what that API is.
`usage diff` reads two of them and says what changed:

```sh
usage diff released.usage.kdl current.usage.kdl
```

```text
breaking [flag-spelling-removed] at ex: flag '--jobs' no longer answers to '-j'
breaking [choice-removed] at ex: flag '--color' no longer accepts 'never'
breaking [cmd-removed] at ex: command 'old-thing' was removed
compatible [flag-added] at ex: flag '--quiet' was added
compatible [cmd-added] at ex: command 'new-thing' was added
metadata [help-changed] at ex: flag '--force' help text changed

Found 3 breaking, 2 compatible, 1 metadata change(s)
```

It exits `1` when there is a breaking change, so a release job can gate on it. Nothing else
in the CLI ecosystem can answer this question, for a structural reason:
[clap#918](https://github.com/clap-rs/clap/issues/918) has been open since 2017 asking for the
export it would need.

## The three categories

One rule draws the lines.

**breaking** — a command line that worked against the old spec now fails, binds differently, or
resolves to a different value. A removed flag, a lost short spelling, a narrowed `choices` set,
a positional that became required, a new `conflicts`, a default that moved.

**compatible** — the interface gained something, or relaxed a rule. Every command line that
worked still works and still means the same thing. A new optional flag, a widened `choices` set,
a requirement dropped, a new subcommand.

**metadata** — nothing about parsing moved: help text, `help_heading`, `display_order`,
hidden-ness, `effect`, deprecation notices, a renamed positional, an internal flag rename that
kept every spelling.

The interesting cases are the ones where the same edit lands in different categories depending
on context:

| edit                                          | category   | why                                                       |
| --------------------------------------------- | ---------- | --------------------------------------------------------- |
| dropping a value from a strict `choices`      | breaking   | the value is now rejected                                 |
| dropping a value from `choices strict=#false` | metadata   | the value is still accepted, just no longer offered       |
| listing a value in a `strict=#false` set      | metadata   | it was already accepted; the list decides what is offered |
| appending an optional positional              | compatible | no word that used to bind moves                           |
| appending a required positional               | breaking   | an invocation without it now fails                        |
| adding a `default`                            | compatible | nothing was resolved there before                         |
| changing or removing a `default`              | breaking   | it moves ground the caller was already standing on        |
| gaining a `conflicts`                         | breaking   | a combination that was valid is now rejected              |
| gaining an `overrides`                        | compatible | a collision that was an error now resolves                |
| gaining a group member, `multiple=#true`      | metadata   | membership only decides what satisfies `required`         |
| gaining a group member, exclusive group       | breaking   | the new member conflicts with the rest                    |
| renaming a command that keeps an alias        | metadata   | the old word still selects it                             |
| renaming a command with no alias              | breaking   | the old word selects nothing                              |
| adding a command nothing else answered to     | compatible | the word meant nothing before                             |
| adding a command over `external_subcommand`   | breaking   | the word used to reach an external command                |
| adding a command where a positional bound     | breaking   | the word used to be that argument's value                 |
| declaring `required_unless` on a flag         | breaking   | the flag is now required unless one of them is present    |
| adding to a non-empty `required_unless`       | compatible | one more way to be excused from the requirement           |
| dropping `subcommand_precedence_over_arg`     | breaking   | a word that selected a command now fills an argument      |
| a `renamed_to` config key that is not there   | breaking   | the promise about where the value went is not kept        |

A rename is not the only thing that can happen to a command in one release, so a renamed
command is compared against what it became. Those findings are located under the **old** name —
what a reader wants to know is what typing the old word does now, and the `cmd-renamed` line
above says which command it reaches.

## Two deliberate silences

**`version` is never reported.** A release bumps it, and a compatibility check that fires on
every release is one nobody leaves switched on. `long_version` is silent for the same reason.

**Derived strings are never reported.** `usage`, `full_cmd` and `help_first_line` restate what
the declarations already say, so a change in one of them is reported at its source or not at all.

A `mount` is compared as a declaration — added or removed — and not by what it discovers.
Resolving one means running the command it names, which reading two files should not do.

## In CI

The shape most releases want is the published spec against what the binary being built says
about itself. One of the two specs may be `-`:

```sh
mycli --usage-spec | usage diff mycli.usage.kdl -
```

As a release gate:

```yaml
- name: the CLI contract still holds
  run: |
    git show "$(git describe --tags --abbrev=0)":mycli.usage.kdl > released.usage.kdl
    mycli --usage-spec | usage diff released.usage.kdl - --breaking
```

`--breaking` drops the compatible and metadata findings, which is what a gate wants to read.
`--exit-zero` reports without failing, for a job that comments on a pull request rather than
blocking it. `--format json` gives the same findings as a list of `{category, code, message,
location}` objects, so a script can act on a specific `code`.

## Compare specs from the same generator

A spec generated from a typed CLI says what the generator of the day could see. Comparing
one emitted by an older `clap_usage` against one emitted by a newer one reports everything
the newer emitter learned to express as an interface change — relationships that were always
enforced but never written down read as newly added constraints.

Refreshing hk's checked-in fixture is this exactly: 329 `constraint-added` findings, none of
them a change to hk. The findings are a true reading of the two files, so the fix is not to
soften them but to compare like with like — the released spec against a spec emitted by the
same generator version, which is what a release job does anyway.

## What `deprecated` is for

A deprecation is metadata: the flag still parses, so it costs nobody anything today. What it
buys is that the removal it promises shows up here as `breaking` later, against a spec that
announced it first. `usage diff` is where a deprecation window is observed rather than
remembered.
