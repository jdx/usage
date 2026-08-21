# `flagset`

A named set of flag declarations that any command can pull in with `use`.

```kdl
name "demo"
bin "demo"

flagset "output" {
  flag "-v --verbose" help="Print more"
  flag "--json" help="Machine-readable output"
}

cmd "build" {
  use "output"
  flag "--release"
}

cmd "test" {
  use "output"
  flag "--filter <pattern>"
}
```

`build` and `test` each end up with three ordinary flags. A `flagset` is resolved while
the spec is read, so nothing downstream — help, completions, docs, manpages, the
generated parsers — sees a new concept. It is the spec's answer to the same repetition
[`flatten`](/rust/subcommands#sharing-declarations-with-flatten) removes on the typed side.

## Where each node goes

`flagset` is declared at the top level of a spec, never inside a command: a set one
command can see and its sibling cannot is a scoping rule to explain for no benefit.

`use` goes wherever the flags themselves would go — at the top level, inside any `cmd` at
any depth, or inside another `flagset`. It takes one or more set names:

```kdl
name "demo"
bin "demo"

flagset "logging" {
  flag "-v --verbose"
}
flagset "output" {
  flag "--json"
}

// the root's own flags
use "logging"

cmd "build" {
  use "logging" "output"
}
```

Order does not matter: the whole file is read before any `use` is resolved, so a set may
be declared below the command that uses it.

## Expansion happens in place

A `use` between two flags expands between them. Help order is spec order, so a set that
always appended would reorder the command that used it:

```kdl
name "demo"
bin "demo"

flagset "output" {
  flag "--json"
}

cmd "build" {
  flag "--release"
  use "output"
  flag "--target <triple>"
}
```

`build`'s flags are `--release`, `--json`, `--target`, in that order.

## The nearer declaration wins

A command that declares a flag the set also declares keeps its own, matching what happens
when a subcommand [redeclares a global](/spec/reference/flag#global):

```kdl
name "demo"
bin "demo"

flagset "output" {
  flag "--json" help="Machine-readable output"
  flag "-q --quiet"
}

cmd "build" {
  use "output"
  flag "--json" help="JSON build report, with a schema"
}
```

`build` gets its own `--json` and the set's `-q --quiet`. Overlap is measured per form,
the way it is everywhere else in a spec: a command declaring `-j --job-count` takes the
set's `-j --jobs` out of the expansion, because `-j` is already spoken for.

## Sets compose

A `flagset` may `use` another, so a large set can be assembled from small ones:

```kdl
name "demo"
bin "demo"

flagset "common" {
  flag "-v --verbose"
}
flagset "output" {
  use "common"
  flag "--json"
}
flagset "input" {
  use "common"
  flag "--stdin"
}

cmd "run" {
  use "output" "input"
}
```

`run` gets `-v --verbose` once — a set reachable twice through composition contributes
once, by the same per-form rule above. A cycle is an error, reported as the path that
closes it (`flagset cycle: a -> b -> a`), and so is a `use` naming a set that does not
exist.

## Shared sets in their own file

Sets travel through [`include`](/spec/reference/), which is how a CLI
with several specs keeps its shared declarations in one place:

```kdl
// common.usage.kdl
flagset "common" {
  flag "-v --verbose"
  flag "--config <FILE>"
}
```

```kdl
// demo.usage.kdl
name "demo"
bin "demo"
include file="./common.usage.kdl"

cmd "build" {
  use "common"
}
```

Each file resolves its own `use` nodes against the sets it declares and the ones it
includes. A file cannot use a set declared only by a file that includes _it_ — that would
make the meaning of a spec depend on who read it — so an unresolved name is an error in
the file that wrote it.

Every set is resolved while its file is read, whether or not anything uses it, so a `use`
naming a set that does not exist is reported even inside a set nothing has reached for yet.

A name may still be declared only once, and an `include` does not make that a choice: two
files declaring `common` is an error wherever the `include` stands, rather than one of them
quietly taking the name from the other. What is counted is declarations rather than
arrivals, so the shared file above may be included by every file that uses its sets and by
the spec that includes those — one declaration reaching a spec by several routes is what
this feature is for.

A `use` belongs to the flags it stands among, so an included file that declares flags of its
own for the same command replaces the `use` along with them — the same rule
[`group`](/spec/reference/group) follows, and for the same reason: whoever owns the flag list
owns what is said about it.

## What a flagset is not

**Not `global`.** A [global flag](/spec/reference/flag#global) is declared once and
inherited by every subcommand automatically. Reach for a flagset when inheritance is the
wrong shape: a set that belongs to some commands and not others, or one whose flags should
be answered by each command rather than by the root.

**Not a [`group`](/spec/reference/group).** A group states a relationship between flags —
at most one, exactly one — and is enforced when a command line is parsed. A flagset states
nothing about the flags it holds; it is only a way to declare them once. A set's flags may
of course be named by a group on the command that used them.

**Not for arguments.** A flag is identified by its spelling wherever it lands, so the same
declaration means the same thing under any command. A positional is identified by its
position, so the same set spliced into two commands with different arguments would mean
two different things. Declaring `arg` inside a `flagset` is an error rather than a guess.

## One-way

Expansion is part of reading the file. A spec that is parsed and re-emitted — by
`usage g json`, by a generator, by anything that prints a spec back out — contains the
flags, not the `flagset` and `use` nodes that produced them, the same way `include` does
not survive a round trip. What a flagset saves is authoring and review, not bytes on the
wire.

Because `flagset` and `use` are spec nodes, a spec using them needs a `usage` new enough
to know them. Say so with `min_usage_version` if your spec is read by tooling you do not
control.
