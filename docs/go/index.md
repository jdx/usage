# Go Framework

::: danger Not ready for testing
The Go framework is experimental and **not ready for any amount of testing yet** — do not build
against it. Unlike [usage-rs](/rust/), which is complete enough that `usage-cli` itself is built
with it, usage-go's APIs, generated output,
and behavior are all still in flux, and much of what these pages document only exists in open
pull requests. These docs are a draft published for review, not an invitation to try it.
:::

The Go framework builds your CLI from a usage spec — but unlike most Go CLI libraries, your
shipped binary never parses the spec. `usage generate go` lowers the KDL into plain Go tables,
typed structs, and a `Parse` function at build time. The result:

- **One small dependency.** The module is `github.com/jdx/usage/go`; beyond the standard
  library it imports only [expr](https://github.com/expr-lang/expr), which evaluates a spec's
  validation expressions.
- **Zero-allocation routing.** The low-level event parser allocates nothing on success or
  failure — roughly 57–110ns per parse on mise's real 211-command spec. Generated `Parse` adds
  typed binding, defaults, and validation on top.
- **Linker-friendly.** Parse tables, validation metadata, and help text are three separate
  tables; the linker drops the ones you don't reference. No `init` functions.
- **One source of truth.** The same KDL spec generates your completions, docs, and manpages.

## Parser overhead

<UsageBenches lang="go" embedded />

The generated package contains plain command and flag tables that the linker lays out before
`main`; there is no spec parser, reflection, command-tree builder, or `init` function in the
shipped program. The event parser keeps its state and 16-entry command stack inline, borrows
values from argv, and scans only the flags in scope. Those are the zero-allocation,
57–110ns measurements. Generated `Parse` does more work to produce the typed result shown
below, so the zero-allocation claim deliberately does not apply to that higher layer — and
it is `Parse` the chart above measures, at about 5.9µs on mise's spec, because binding to a
filled struct is the whole of what cobra, urfave/cli and kong each do in one call. The
[Go README](https://github.com/jdx/usage/blob/main/go/README.md) has both numbers, what a
whole process costs, and where `Parse` spends its time.

## Quick start

Write a spec:

```kdl
name "ex"
bin "ex"
version "1.0.0"
flag "-v --verbose" global=#true help="be loud"
flag "-j --jobs <n>" help="how many jobs"
arg "<file>" help="the file to process"
cmd "install" help="install a tool" {
    alias "i"
    flag "-f --force"
    arg "<pkg>"
}
```

Generate the Go code:

```go
//go:generate usage generate go -f ex.usage.kdl -o tables.go -p ex
```

Parse:

```go
package main

import (
	"fmt"
	"os"

	"github.com/jdx/usage/go/argv"
)

func main() {
	cli, err := ex.Parse(os.Args[1:])
	if err != nil {
		exit(err.(*argv.Error))
	}
	if cli.Install != nil {
		install(cli.Install.Pkg, cli.Install.Force, cli.Verbose)
		return
	}
	process(cli.File)
}
```

`Parse` returns a typed struct per command — `cli.Install` is `nil` unless `install` (or its
alias `i`) was invoked — with flags bound, `env`/`default` fallbacks applied, and `required`,
`choices`, `var_min`/`var_max`, and flag relations enforced.

## Handling help, version, and failures

Unlike the Rust framework's `parse()`, the generated Go `Parse` never prints or exits — help and
version requests come back as errors with `Code` set, and rendering is yours to invoke. The
standard exit function looks like this:

```go
func exit(e *argv.Error) {
	pos := argv.Walk(ex.Root, os.Args[1:])
	path := []string{"ex"}
	for _, c := range pos.Chain[1:] {
		path = append(path, c.Name)
	}
	switch e.Code {
	case argv.CodeHelp:
		if e.Long {
			fmt.Print(argv.LongHelp(ex.HelpMeta, path, pos.Chain, ex.HelpText))
		} else {
			fmt.Print(argv.ShortHelp(ex.HelpMeta, path, pos.Chain, ex.HelpText))
		}
		os.Exit(0)
	case argv.CodeVersion:
		fmt.Println("ex " + ex.Version)
		os.Exit(0)
	default:
		fmt.Fprint(os.Stderr, argv.Render(e, path, pos.Chain, ex.HelpText))
		os.Exit(2)
	}
}
```

The rendered pages match usage-lib's byte for byte, and the failure messages are clap-shaped —
see [Help and errors](/go/help).

## Why generation instead of a runtime spec?

The Go module has no KDL parser, on purpose. Lowering a spec is `usage-cli`'s job, done once at
build time; the shipped binary carries tables the linker can lay out as data. Building tables at
runtime is not supported — generation is the only path, and the point.

Everything is verified against the reference implementation: a shared JSON conformance corpus
(all vectors passing) covers the parsing grammar, and all 211 of mise's usage lines, `-h` pages,
and `--help` pages are compared byte-for-byte against usage-lib's rendering in CI.

## Where to go next

- [Generated code](/go/generated-code) — what `usage generate go` emits and what `Parse` does
- [The parser](/go/parser) — the low-level zero-allocation event API
- [Binding and values](/go/binding) — env/default resolution, validation, typed conversions
- [Help and errors](/go/help) — rendering `-h`/`--help` pages and failures
- [Completions](/go/completions) — answering shell completion requests

## Current limitations

Worth knowing before you commit:

- **`overrides` is not enforced by generated `Parse`.** `conflicts`, `required_if`, and
  `required_unless` are; a spec relying on last-one-wins `overrides` semantics needs to call
  `argv.ApplyOverrides` itself, and `argv.CheckDisplaced` on each key it reports — a flag that
  lost is out of the running for `required`, `env`, and `default`, but the word it was typed is
  still judged against its `choices`.
- **Fields are `string`, `bool`, `[]string`, or `int` (for counts).** A spec says what a value is
  called, never what type it is — convert with [`argv.Int`, `argv.Duration`, etc.](/go/binding#typed-values)
- **`complete` run-scripts, `config` nodes, `group`, `value_hint`, and `mount` are not carried
  into the generated tables.** Completions still offer `choices` and derive file, directory,
  executable, and command completion from `complete` types and value names — only `run=`
  shell-out completers are unsupported, since they mean running a subprocess on every Tab. See
  [Completions](/go/completions). Config resolution is not implemented.
- Command trees deeper than 16 levels are rejected (`CodeTooDeep`); short flags must be ASCII.
