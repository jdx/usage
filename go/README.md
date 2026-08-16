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

|                         | instructions, cold | wall, whole process |  binary |
| ----------------------- | -----------------: | ------------------: | ------: |
| a do-nothing Go process |                  — |             0.95 ms | 2.31 MB |
| **usage-go**            |         **~2,700** |          **1.1 ms** | 2.37 MB |
| cobra                   |          2,008,880 |              1.8 ms | 3.87 MB |
| urfave/cli v3           |          5,591,321 |              1.7 ms | 5.74 MB |
| kong                    |         57,889,084 |              6.1 ms | 5.34 MB |

Instruction counts are cachegrind, one cold construct-and-parse in a fresh process
(`PARSE_N=1` minus `PARSE_N=0`), the same method [`tasks/perf-shadow.sh`](../tasks/perf-shadow.sh)
uses for the Rust shadows. usage-go's figure is amortized over 1,000 parses because
a single one is _below the Go runtime's own startup jitter_ — ±80,000 instructions
run to run, which is thirty times the whole parse.

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

**Binding only.** The parser answers one question — which token becomes which flag
or argument — and reports each occurrence as an event. Everything that needs to
know a value's _type_ (`required`, `choices`, `env` fallback, defaults, `var_min`,
`overrides`) belongs to the layer that owns the target struct, exactly as it does
in Rust. That is why the corpus's 22 post-binding vectors are skipped here rather
than failed.

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
    }
}
if err := p.Err(); err != nil {
    // binding failed, or --help was asked for
}
```

Tables are meant to be generated. Writing them by hand is supported, and is what
[`argv/parser_test.go`](argv/parser_test.go) does:

```go
var (
    force = &argv.Flag{Name: "force", Longs: []string{"force"}, Shorts: []byte{'f'}}
    root  = &argv.Command{Name: "ex", Flags: []*argv.Flag{force}}
)
```

Dispatch on `Key` rather than `Name` in generated code: it is what the field
identifiers are for, and it costs no string comparison.

## Conformance

The [corpus](../corpus) is the definition of correct, and it is plain JSON so that
an implementation in any language can run it. `go/conformance` runs all of it:

```sh
mise run test:go
```

101 binding vectors pass; the 22 post-binding ones are skipped with the reason
recorded. A vector's spec is KDL, and this module deliberately has no KDL parser —
`usage generate json` does the lowering, which is why the suite needs the CLI
built. That split is the same one an adopter gets: tables are generated once at
build time by a maintainer who has the usage CLI, and the shipped binary never
sees a spec.

## What is missing

- **The generator.** `usage generate go`, emitting the tables above from a spec.
  Until it exists, tables are written by hand or built at run time from a lowered
  spec via `internal/spec`.
- **The typed layer.** Binding produces events; something has to turn them into a
  struct with `int` and `time.Duration` fields, and apply the post-binding rules.
  This is where the corpus's 22 skipped vectors get answered.
- **Help and errors.** A cold table of help text, and rendering worth reading.
- **Completions.** The Rust side serves these from the parser's own scope rules so
  that what is offered and what is accepted cannot disagree; the hooks for it
  (`Collecting`, `PendingArg`, `FlagsInScope`, `CommandStart`) are already here.
