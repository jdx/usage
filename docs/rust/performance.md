# Parser performance

usage's compiled Rust parser is designed so ordinary argv parsing reads static
tables and writes directly into the result. Cold metadata for help, specs, and
completions is not constructed on a successful parse.

## Mise-scale result

The gate uses generated shadows of the same checked-in mise spec: 211 commands,
722 flags, 129 positional arguments, and four command levels. A third binary
does the same startup work without parsing, so instruction and wall-time
measurements subtract startup from two runs of the same binary.

| Framework | Instructions | vs usage | Wall time | Allocations, no values bound |
| --------- | -----------: | -------: | --------: | ---------------------------: |
| usage     |        7,377 |        — |    0.7 µs |                            0 |
| clap      |        6.31M |     855x |    544 µs |                        6,560 |
| bpaf      |        21.9M |   2,957x |  1,610 µs |                            — |

clap and bpaf construct and validate a parser at runtime before reading a
single word, so their floor is hundreds of microseconds. usage reads tables the
compiler already laid out, so its floor is hundreds of nanoseconds.

## Why it is fast

The derive does the expensive work while the application is compiled. It emits
the command tree as static `Command`, `Flag`, and `Arg` tables, so starting a
parse does not construct, validate, or allocate a parser. The hot path then
does only the work the argv asks for:

- It reads argv once, borrowing each value from its `OsStr` instead of copying it.
- A flag lookup scans the current command's flags and inherited globals, not all
  711 flags in the CLI. Once found, a generated integer key selects the
  destination field without another string lookup.
- The parser's command stack is a fixed-size array. A bare parse never reaches
  the heap; an owned value such as `String` accounts for one allocation only
  when that value is actually bound.
- Help text, spec and completion metadata, and human-readable diagnostics live
  outside the parse tables. A successful parse neither constructs nor reads them.
- Generated binding writes into one accumulator in place. Selecting a subcommand
  creates only that variant's accumulator, not storage for every command the
  user did not select.

The result scales with the command path and values that were typed, rather than
with the whole CLI.

## Compile time

That runtime speed moves work into compilation. On the same mise-scale generated
shadows, a debug rebuild of only the CLI crate, with each framework's dependencies
already built, took a median 10.2 seconds with usage-rs, 3.6 seconds with clap, and
1.6 seconds with bpaf over three runs. In a separate isolated clean-build pass,
the same ordering held at about 15, 6, and 4 seconds respectively. These
measurements used rustc 1.97.1 on x86_64 Linux.

The absolute times are specific to this unusually large command tree and the
benchmark host, but the tradeoff is not: usage-rs is doing substantially more
work at compile time. A project with a large CLI declaration and a tight edit-build
loop may be better served by clap or bpaf, especially when parser startup is not a
meaningful part of its runtime.

## What clap's number includes

For this generated command tree, clap's approximately 544 µs consists of about
343 µs constructing the tree, 178 µs validating it, and 23 µs parsing. usage's
tables are compiled, so its 0.7 µs path has no equivalent construction or
validation phase. Even compared only with clap's already-built parse phase,
usage is about 34x faster in this measurement.

## Binary size

The gate's parse-only binaries — each linking its framework's mise-scale
shadow, built by the same workspace release build, stripped:

| Framework                   |     Bytes | Decimal MB |
| --------------------------- | --------: | ---------: |
| argh                        | 1,010,392 |     1.0 MB |
| usage, argh vocabulary      | 1,189,312 |     1.2 MB |
| usage, clap vocabulary      | 1,311,096 |     1.3 MB |
| usage, full spec vocabulary | 1,319,424 |     1.3 MB |
| bpaf                        | 2,493,936 |     2.5 MB |
| clap                        | 3,102,696 |     3.1 MB |

The common-vocabulary usage shadows drop exactly the properties their comparison generator
drops and disable the spec endpoint neither framework has. The argh version also shortens every
description the same way and uses the same unboxed command enums. That leaves each pair operating
over the same expressible CLI instead of charging usage for richer metadata. The full usage
shadow remains the conformance fixture. On common vocabulary, usage is 58% smaller than clap
and 18% larger than argh. For what the spec endpoint itself weighs, see
[Spec output](/rust/spec#the-endpoint) — 65 KB on a small CLI, and
`#[usage(spec_endpoint = false)]` removes it.

### Where the size lives, and what removes it

The runtime crate itself compiles to about 16 KB; nearly all of the parser's
footprint is the per-command binding code the derive generates, plus the static
tables it reads (about 150 KB at mise scale). Size therefore tracks how many
commands and fields a CLI declares, not which usage features it turns on.

Two profile settings any CLI can apply cut further, independent of usage:

- `strip = true` removes symbols and debug info — the tables above already
  assume it.
- `panic = "abort"` removes unwinding landing pads and the backtrace machinery
  std otherwise links. On the mise-scale binary this is another 84 KB (−6%).

## Method and limits

- Instruction counts come from Cachegrind, whose run-to-run variation on the
  benchmark host is much lower than wall-clock timing.
- `tak` runs the release binaries repeatedly and reports the difference between
  the no-parse and parse paths.
- Every shadow is generated from the same spec, and each intentionally drops
  what its framework cannot express. `parse-n-usage-argh` and `parse-n-usage-clap` apply the
  comparison framework's exact drop set to usage for common-vocabulary binary-size comparisons.
- Binary sizes are the gate's `parse-n*` binaries from a full workspace release
  build, stripped, against rustc 1.97.1 on x86_64-unknown-linux-gnu. The
  workspace build matters: cargo unifies features, so clap gets the features
  (color, suggestions, help, derive, env) a real CLI of this size enables.
- This measures routing and parsing, not process startup, configuration loading,
  command execution, help rendering, or completion generation.
- Refreshing or growing the mise fixture moves the absolute counts. Compare
  parsers in the same commit, not one commit's count against another's.

The benchmark sources live in `benches/gate`, with generated shadows under
`benches/shadows`. Gate maintenance — including the `markdown` benchmark on
this repository's own spec — is documented in [Contributing](/contributing#performance-checks).
