# usage-go

A CLI framework for Go, built the way [usage-argv](../argv) is built for Rust: the
command line is bound against **static tables** instead of a command tree
assembled at run time.

> **Status: the binder.** `argv` implements the [argv grammar](../docs/spec/argv.md)
> and passes every binding vector in the [conformance corpus](../corpus). The code
> generator that emits tables from a spec, and the layer above binding that fills a
> typed struct, are not written yet. See [what is missing](#what-is-missing).

## Why

Every Go CLI framework builds a model of the CLI at run time. cobra constructs a
`cobra.Command` per subcommand, each with its own flag set; kong walks a struct
with reflection. Both pay for the whole CLI on every invocation, including the two
hundred commands the user did not type.

Measured against a shadow of [mise](https://mise.jdx.dev)'s spec — 211 commands,
711 flags, 128 positionals — parsing `mise use -g node@20`:

|                         | instructions | wall, whole process |  binary |
| ----------------------- | -----------: | ------------------: | ------: |
| a do-nothing Go process |            — |             0.95 ms | 2.31 MB |
| **usage-go**, amortized |   **~1,600** |          **1.1 ms** | 2.37 MB |
| cobra, amortized        |    3,249,052 |              2.0 ms | 3.41 MB |
| urfave/cli v3, likewise |    5,591,321 |              1.7 ms | 5.74 MB |
| kong, likewise          |   57,889,084 |              6.1 ms | 5.34 MB |

Instruction counts are cachegrind, against mise's committed spec, on the argv the
Rust shadows use so the two tables describe the same work. The column is labelled
per row rather than once at the top, because the two kinds of figure are not the
same measurement: the three frameworks are one cold parse, and usage-go's is
amortized over a thousand binds — for the reason below, which is that a single one
of ours cannot be measured at all.

**usage-go's row is reproducible: `mise run perf:go`.** That harness
([`tasks/perf-go.sh`](../tasks/perf-go.sh)) reports the bind amortized over 1,000
binds and the runtime floor beside it, rather than subtracting the floor once and
forgetting it — because a single bind is _below the Go runtime's own startup
jitter_, ±50,000 instructions run to run, which is thirty times the whole bind.
Differencing `PARSE_N=1` against `PARSE_N=0` the way the Rust harness does gives a
number here that changes sign between runs.

cobra's row is reproducible too, and by the same command. `xtask gen-shadow
benches/mise.usage.kdl benches/go/cobra cobra` writes mise's CLI out as a cobra
program — 211 commands, each with its own flag set — which is checked in under
[`benches/go/cobra`](../benches/go/cobra) and measured beside usage-go. So the two
rows describe the same CLI rather than two people's transcriptions of it.

Its figure includes building the command tree, because that is what cobra does on
every process start. Hoisting that out of the loop would measure its parser
against a program that had already paid for its model, which no CLI gets to do.

That measurement replaced a hand-taken one of 2,008,880, which was lower because
the program it was taken against was written by hand and smaller than mise: the
generated one declares every command and flag the spec has. What cobra cannot
express is printed when the shadow is generated rather than passed over — 128
positionals, since cobra validates a count and not a name, 17 hidden aliases, 13
second long forms, and one short-only flag.

urfave/cli v3's and kong's rows are still hand-measured against programs that are
not in the repository, and until they are generated the same way those two numbers
should be read as an order of magnitude rather than as a measurement.

Two things are worth reading off that table honestly. The win against cobra is
real — about 40% of process startup — but it is bounded: 0.95 ms of usage-go's
1.1 ms is Go runtime startup that no parser can touch. And the framework that
gives Go the ergonomics people actually want, kong's struct tags, costs 29× cobra
to do it, because reflection is the only way to get them without a build step.
Generated tables are the way to have both.

## The design

Three properties, each of which is tested rather than claimed:

**Nothing is built before `main`.** The tables are package-level `var` holding
plain data, so the Go linker lays them out. `go tool nm` reports them as type `D`
and the package has no `init` function — a 211-command table costs 47 KB of
initialized data and zero instructions.

**A parse allocates nothing.** The parser holds its state, its ancestor chain and
its error inline; a bound value is a slice of the argv string rather than a copy.
`TestParseAllocatesNothing` measures this with `testing.AllocsPerRun`, on the
failure paths as well as the success ones. A mise-sized binding runs in 57 ns.

**Binding stays separate from judging.** The parser answers one question — which
token becomes which flag or argument — and reports each occurrence as an event.
The rules that need a value's declared type live in `post.go`, reading a second,
cold table the parser never touches: `required`, `choices`, the `env`-then-
`default` fallback, `var_min`, `var_max`. They are pure functions over what
binding produced rather than a framework, because the caller is the one that knows
how it accumulated — generated code assigns to a field, a harness with no target
type appends to a slice, and inventing a value model here would force both through
it.

The four rules that compare one entry against another — `conflicts`, `overrides`,
`required_if`, `required_unless` — live in `relationships.go`, because a name in
the declaration has to be resolved to the entry it refers to before any of them
can be checked. `overrides` is the odd one out and is applied first: it asks which
of two flags came _last_, which only the arriving tokens know.

## Using it

```go
p := argv.New(root, os.Args[1:])
for p.Next() {
    switch ev := p.Event(); ev.Kind {
    case argv.KindCommand:
        // ev.Command was selected
    case argv.KindFlag:
        // ev.Flag was given, with ev.Value if ev.HasValue
    case argv.KindArg:
        // ev.Value filled ev.Arg
    case argv.KindExternal:
        // ev.Values is the unmatched name, then the rest of argv
    }
}
if err := p.Err(); err != nil {
    // binding failed, or --help was asked for
}
```

Tables are generated from a spec by the usage CLI, which is what Go has instead of
a derive macro:

```go
//go:generate usage generate go -f mycli.usage.kdl -o tables.go
```

The generated file exports `Parse`, a struct per command, `Root` and the two cold
tables, and a key constant per entry:

```go
cli, err := mycli.Parse(os.Args[1:])
if err != nil {
    fmt.Fprint(os.Stderr, argv.Render(err.(*argv.Error), path, chain, mycli.HelpText))
    os.Exit(2)
}
if cli.Run != nil {
    fmt.Println(cli.Run.Task, cli.Run.Args)
}
```

`Parse` binds, applies the post-binding rules, and fills the structs — a missing
required flag or a value outside its choices comes back rather than reaching your
code, and `env` and `default` values reach the fields.

Fields are `string`, `bool` and `[]string`, because that is what a spec knows: it
says what a value is _called_ and never what type it is. Turning `"8"` into an
`int` is what the conversions above are for.

**Three tables, and you pay for the ones you use.** Go's linker drops an
unreferenced package-level table entirely, so the split is enforced by the linker
rather than by a feature flag:

| a CLI that…                    | carries          | mise-sized binary |
| ------------------------------ | ---------------- | ----------------: |
| only binds                     | the parse tables |           2.60 MB |
| applies the post-binding rules | `+ Meta`         |           2.82 MB |
| prints help                    | `+ HelpText`     |           2.82 MB |

None of them has an init function. That is what Rust gets from putting the cold
half behind a feature flag, except nobody has to remember the flag — which is also
why help text is a third table rather than more fields on `Meta`: folding them
together would make every CLI that applies a rule carry every help string in the
spec.

Dispatch on the key constants rather than on `Name`: it costs no string
comparison, and a flag renamed in the spec then fails to compile instead of
silently never matching.

Writing tables by hand is supported too, and is what
[`argv/parser_test.go`](argv/parser_test.go) does:

```go
var (
    force = &argv.Flag{Name: "force", Longs: []string{"force"}, Shorts: []byte{'f'}}
    root  = &argv.Command{Name: "ex", Flags: []*argv.Flag{force}}
)
```

## Help

`argv.UsageLine` renders the line a page prints after `Usage: `, from the parse
tables and `HelpText`:

```go
argv.UsageLine([]string{"mise"}, mise.Root, mise.HelpText)
// mise [FLAGS] [TASK] <SUBCOMMAND>
```

`argv.ShortHelp` renders the whole page `-h` prints — header, `Commands`,
`Arguments`, `Flags` and `Global flags`, with the columns lined up and the
inherited globals worked out the way the parser resolves them.

`argv.LongHelp` renders `--help`: the same content through a wider layout, with
help wrapped into a column, the long form of each description preferred, and each
annotation on its own line.

**All 211 usage lines, all 211 `-h` pages and all 211 `--help` pages match
usage-lib byte for byte**, which is the test that keeps them honest. usage-lib builds the line from a spec through a
template over a runtime model; this builds it from static tables. Reimplemented
rules drift, so both are run over mise's real spec and compared — the same check
`benches/gate/tests/help.rs` makes for usage-argv, against the same reference.

Two implementations checked against one oracle beats two checked against each
other.

## Typed values

Binding collects text, deliberately — the grammar decides which token becomes
which flag, not what it means. `argv.Int`, `Uint`, `Float`, `Bool`, `Duration`
and `Each` are where that text becomes a value, each failure carrying the word
that would not convert and the type it was going to.

Note `Bool` and `EnvTruth` are different widths on purpose: `Bool` takes Go's
spellings for a value somebody typed, `EnvTruth` is the narrower allow-list
usage-lib uses to decide whether a variable sets a value-less flag at all.

## Completions

`argv.Walk` reads the words before the cursor and reports what it is standing in;
`argv.Candidates` says what could go there — subcommands and their aliases, the
flags in scope, and the values a `choices` list allows.

Both ask the parser rather than re-deriving its rules, which is the point: a
completion advertising a flag the parser would refuse is worse than no completion.
So a global is offered inside a subcommand, a redeclared name shadows the
inherited one, a hidden flag binds without being advertised, and past a `--`
nothing is offered at all.

`argv.RenderAnswer` writes the result in the protocol each shell reads — bash
takes values, fish, nu and PowerShell take a description after a tab, and zsh
takes a third field with the text to insert, because what it displays and what it
types are not always the same string.

## Errors

`argv.Render` turns a failure into what a CLI should print to stderr:

```
error: unknown flag `--wat`

Usage: ex run [-f --force]

For more information, try `--help`.
```

The one part of this module with **no reference to match**. usage-lib prints a
one-line message inside miette's frame and usage-argv renders through miette too;
neither travels, because miette is a Rust library and a Go CLI drawing the same
ASCII art would be imitating a diagnostic format rather than sharing one. So this
is judged on whether the message says what went wrong, where, and what to try —
and tested by asserting those rather than by comparing bytes.

## Conformance

The [corpus](../corpus) is the definition of correct, and it is plain JSON so that
an implementation in any language can run it. `go/conformance` runs all of it:

```sh
mise run test:go
```

**All 154 vectors pass** — every one the corpus has, binding and post-binding
alike. The suite asserts that nothing was skipped, so that stays a measurement
rather than a claim.

The number is worth watching rather than quoting: 101 when this module landed, 122
once the corpus imported the argv questions clap's suite answers, 145 once the
post-binding rules arrived, 152 with the relationships between flags, and 154 as
the corpus kept growing underneath. Every one of those increases was answered
without a change to the Go side, which is the argument for a shared corpus in one
line. A vector's spec is KDL, and this module deliberately has no KDL parser —
`usage generate json` does the lowering, which is why the suite needs the CLI
built. That split is the same one an adopter gets: tables are generated once at
build time by a maintainer who has the usage CLI, and the shipped binary never
sees a spec.

`internal/shadow/mise` holds the tables generated from mise's committed spec — 211
commands, 711 flags — checked in so a reviewer sees the diff when the emitter
changes, and regenerated by `mise run gen-go`. It is where the zero-allocation
claim is measured at real scale rather than against a fixture with four flags:
110 ns per parse, 0 allocations.

## What is missing

- **A typed front door.** The conversions exist; what is missing is generated
  code that calls them, so a CLI author gets a struct rather than events.
- **Shadow programs for the other frameworks.** usage-go's own numbers are
  reproducible with `mise run perf:go`; urfave's and kong's are still
  hand-measured, because generating mise-sized programs for them from the spec is
  its own piece of work.
- **Running a spec's `complete` scripts.** A `run=` block shells out, which this
  package has no business doing on a Tab. Everything else about completion is
  here: the request, the answer, and the script that registers it with each of
  the five shells.
