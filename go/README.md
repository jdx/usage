# usage-go

> [!CAUTION]
> **usage-go is not ready for any amount of testing.** Unlike the Rust framework
> (`usage-rs`), which is complete enough that `usage-cli` is built with it, usage-go's
> APIs, generated output, and behavior are all still in flux, and parts of what is
> described below only exist in open pull requests. Do not build against it yet.

A CLI framework for Go, built the way [usage-argv](../argv) is built for Rust: the
command line is bound against **static tables** instead of a command tree
assembled at run time.

> **Status: parsing, binding, help, and completions.** `argv` implements the
> [argv grammar](../docs/spec/argv.md) and passes every binding vector in the
> [conformance corpus](../corpus). `usage generate go` emits the tables from a spec,
> along with a typed `Parse` that fills a struct per command. See
> [what is missing](#what-is-missing).

## Why

Every Go CLI framework builds a model of the CLI at run time. cobra constructs a
`cobra.Command` per subcommand, each with its own flag set; urfave/cli assembles the
same shape out of `cli.Command` values; kong walks a struct with reflection. All
three pay for the whole CLI on every invocation, including the two hundred commands
the user did not type.

Measured against a shadow of [mise](https://mise.jdx.dev)'s spec — 211 commands,
711 flags, 128 positionals — parsing `mise use -g node@20`:

|                                 |  one parse | vs usage-go | median |
| ------------------------------- | ---------: | ----------: | -----: |
| **usage-go**, argv → struct     | **5.9 µs** |             | 6.5 µs |
| cobra, build tree + resolve     |     110 µs |        ~18x | 120 µs |
| urfave/cli v3, build tree + run |     200 µs |        ~34x | 220 µs |
| kong, reflect + parse           |     3.0 ms |       ~500x | 3.3 ms |
| usage-go, argv → events         |      73 ns |             |  79 ns |

**In-process parse throughput**: each parser run repeatedly in one process, the
fastest of many short rounds reported. This is the measurement
[`benches/gate/src/bin/time-sweep.rs`](../benches/gate/src/bin/time-sweep.rs) takes
for usage-rs against clap and bpaf, and it is here for the same reason: what a parse
costs is a question about parsing, and a whole Go process is about a millisecond of
runtime startup before `main` — two orders of magnitude larger than the thing being
compared, and varying run to run by more than most of these rows cost. Reproduce with
`mise run perf:go`, which runs
[`benches/go/cmd/sweep`](../benches/go/cmd/sweep/main.go).

The four rows are the same work: from argv to a value the program can use. usage-go's
is `Parse` — bind, apply the post-binding rules, fill the typed structs — because that
is the whole of what the other three do, and comparing our cheapest half against their
whole is how a benchmark flatters its author. The binder alone is the last row, kept out
of the comparison: no other framework here has a stage that answers "which token is
which" and stops.

Each framework's row includes building its model, because that is what it does on every
process start. Hoisting that out would measure a parser against a program that had
already paid for its model, which no CLI gets to do. The collector is switched off
during a round and asked to run between rounds — a process that parses one command line
and gets on with it usually exits before the collector would have run at all, and left
on it lands unevenly enough to move a minimum by 2x.

**All four programs are generated from that one spec**, by `mise run gen-shadow`, and
checked in under [`benches/go`](../benches/go) so a reviewer sees the diff when an
emitter changes. That is what makes this a comparison between parsers rather than
between transcriptions of mise: the hand-written cobra program these replaced was a
third of mise's size, and read as though cobra were three times faster than it is.

What a framework cannot express is printed when its shadow is generated rather than
passed over, since a shadow that quietly dropped half the spec would measure a smaller
CLI. cobra takes every positional as a count rather than as a name, and has no vocabulary
for a short-only flag or a second long form. urfave has no arity check for a single
positional. kong is the one that loses most: 222 flags that a subcommand redeclares
cannot be said at all, because a kong flag reaches every command below the one that
declares it and redeclaring it is a duplicate `kong.New` refuses; and seven commands'
positionals go, because kong cannot mix positionals with subcommands on one node.

### What a whole process costs

The number an adopter feels, and mostly not the parser:

|                                 | instructions | whole process |  binary |
| ------------------------------- | -----------: | ------------: | ------: |
| the Go runtime, parsing nothing |   ~1,010,000 |       1.14 ms |       — |
| **usage-go**, argv → struct     |  **123,293** |   **1.16 ms** | 6.83 MB |
| usage-go, argv → events         |        1,955 |       1.09 ms | 4.80 MB |
| cobra                           |    2,807,995 |       1.62 ms | 3.42 MB |
| urfave/cli v3                   |    5,763,828 |       1.69 ms | 5.48 MB |
| kong                            |   66,688,959 |       5.62 ms | 5.38 MB |

Instruction counts are cachegrind, amortized over 1,000 parses for usage-go and fewer
for the frameworks that cost three to five orders of magnitude more — 20 each for cobra
and urfave, 2 for kong, since a thousand kong parses under cachegrind would take
minutes. Amortized rather than differenced from a single parse because Go's startup
varies run to run by ±50,000 instructions, which is twenty-five times what usage-go's
binder costs.
Taken with `GOMAXPROCS=1`, which is what makes them a measurement at all: valgrind
serializes every thread onto one core, and a Go runtime with more than one to schedule
spends the wait spinning, so unpinned counts pick up instructions proportional to wall
time — twenty cobra resolves read 56M on one run and 5,002M on the next.

The binary column is each row's harness — `go/internal/bench/parse-n` and `bind-n` for
the two usage-go rows, `benches/go/cmd/parse-n-cobra` and its siblings for the other
three — built with a plain `go build`, no `-ldflags` and nothing stripped. A harness is a
`main` that parses N times and prints whether it arrived, so what the column compares is
what each framework drags in, plus a few lines.

The wall column is the fastest of 200 whole processes, and the floor is reported beside
it rather than subtracted from it. The floor and the row under it are one binary asked
for nought parses and for one, and they differ by less than a process launch varies: a
usage-go parse is _below the resolution of that column_, which is the honest thing for
it to say rather than an ordering to read. The same caution applies to cobra's row
against urfave's: they are 0.1 ms apart on a clock whose runs vary by more than that,
and which of them comes out ahead changes between runs — the parse table above is where
those two are separated. A millisecond of every row here is the Go runtime coming up,
and no parser wins that back.

Three things are worth reading off these two tables honestly. The win over cobra is
real and it is a factor of eighteen, but on the clock a user feels it is 0.46 ms of a
1.6 ms process. The framework that gives Go the ergonomics people actually want —
kong's struct tags — costs 27x cobra to do it on the clock, and 24x by instruction
count, because reflection is the only way to get them without a build step; generated
tables are how to have both. And usage-go's
typed front door costs about eighty times its own binder on the clock, and sixty-three
times by instruction count, most of it in two maps the generated `Parse` allocates per
call: the binder is as fast as this repository claims, and the layer above it has not
had the same attention.

## The design

Three properties, each of which is tested rather than claimed:

**Nothing is built before `main`.** The tables are package-level `var` holding
plain data, so the Go linker lays them out. `go tool nm` reports them as type `D`
and the package has no `init` function — a 211-command table costs 47 KB of
initialized data and zero instructions.

**A parse allocates nothing.** The parser holds its state, its ancestor chain and
its error inline; a bound value is a slice of the argv string rather than a copy.
`TestParseAllocatesNothing` measures this with `testing.AllocsPerRun`, on the
failure paths as well as the success ones. `argv`'s own `BenchmarkParse` binds a
four-flag fixture in 57 ns.

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

Fields are `string`, `bool`, `[]string`, and `int` for a `count` flag — which is what a
spec knows, since it says what a value is _called_ and never what type it is. Turning
`--jobs 8`'s `"8"` into an `int` is what the conversions above are for.

**Three tables, and you pay for the ones you use.** Go's linker drops an
unreferenced package-level table entirely, so the split is enforced by the linker
rather than by a feature flag:

| a CLI that…                    | carries             | mise-sized binary |
| ------------------------------ | ------------------- | ----------------: |
| only binds                     | the parse tables    |           4.65 MB |
| applies the post-binding rules | `+ Meta`            |           5.18 MB |
| prints help                    | `+ HelpText`        |           5.77 MB |
| takes the typed front door     | `+ Meta`, `+ Parse` |           6.68 MB |

Four `main` packages over `internal/shadow/mise`, each referencing one more table than
the last and each built with a plain `go build` — the same toolchain and flags as the
harnesses above, so the two tables' megabytes are comparable. They are throwaways rather
than checked in: what is being measured is which table the linker keeps, and a reference
is all it takes to make it keep one.

None of them has an init function. That is what Rust gets from putting the cold
half behind a feature flag, except nobody has to remember the flag — which is also
why help text is a third table rather than more fields on `Meta`: folding them
together would make every CLI that applies a rule carry every help string in the
spec.

The last row is the one to be uncomfortable about: `Parse` is a generated function
with a case per entry in the spec, and at mise's scale that is two megabytes of code
on top of the tables it reads. A CLI with two hundred commands pays it; the split
above is what a CLI that wants less can reach for instead.

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

**Every vector passes** — binding and post-binding alike. The suite asserts that
nothing was skipped, so that stays a measurement rather than a claim.

The count is worth watching rather than quoting: 101 when this module landed, 122
once the corpus imported the argv questions clap's suite answers, 145 once the
post-binding rules arrived, 152 with the relationships between flags, and it
kept growing underneath. Every one of those increases was answered without a
change to the Go side, which is the argument for a shared corpus in one line. A vector's spec is KDL, and this module deliberately has no KDL parser —
`usage generate json` does the lowering, which is why the suite needs the CLI
built. That split is the same one an adopter gets: tables are generated once at
build time by a maintainer who has the usage CLI, and the shipped binary never
sees a spec.

`internal/shadow/mise` holds the tables generated from mise's committed spec — 211
commands, 711 flags — checked in so a reviewer sees the diff when the emitter
changes, and regenerated by `mise run gen-go`. It is where the zero-allocation claim is
measured at real scale rather than against a fixture with four flags: 110 ns per parse,
0 allocations.

The same tables are generated a second time into
[`benches/go/mise`](../benches/go/mise). The sweep that times four frameworks in one
process has to live in the module that depends on the other three, and Go's `internal`
rule means that module cannot import this copy however the two are laid out.

Three binder numbers appear in this file, from three harnesses, so the labels matter.
**73 ns** is the canonical one: the sweep's minimum for `mise use -g node@20`, which
every ratio above uses and `mise run perf:go` reprints. 110 ns is this package's
`BenchmarkParse` on that same argv — `go test -bench` reports a mean over one long run
where the sweep reports a minimum over short ones, and on the machine that took the
73 ns that benchmark reads about 80 ns, so read the two as one measurement taken two
ways on two machines rather than as a change. 57 ns is `argv`'s own `BenchmarkParse`,
which binds a four-flag fixture rather than mise's spec.

## What is missing

- **Typed fields.** `Parse` fills a struct, but only with the types a spec knows —
  `string`, `bool`, `[]string`, and `int` for a `count`. The conversions in
  [typed values](#typed-values) exist and generated code does not call them, so
  `--jobs 8` reaches your program as `"8"`.
- **A front door as fast as the binder.** `Parse` costs about eighty times the bind
  it wraps, most of it in two maps it allocates per call to collect what arrived
  before the post-binding rules judge it. Nothing about that is inherent — a spec's
  entries are known at generation time and could be slots in an array — and until it
  is done the number in the table above is the honest one to quote.
- **Running a spec's `complete` scripts.** A `run=` block shells out, which this
  package has no business doing on a Tab. Everything else about completion is
  here: the request, the answer, and the script that registers it with each of
  the five shells.
