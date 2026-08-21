# Parser performance

usage's compiled Rust parser is designed so ordinary argv parsing reads static
tables and writes directly into the result. Cold metadata for help, specs, and
completions is not constructed on a successful parse.

## Mise-scale result

The gate uses generated usage and clap shadows of the same checked-in mise spec:
211 commands, 711 flags, 128 positional arguments, and four command levels. A
third binary performs the same startup work without parsing, which lets the
instruction and wall-time measurements subtract startup from two runs of the
same binary.

| Measurement                                  | usage |  clap |                Result |
| -------------------------------------------- | ----: | ----: | --------------------: |
| Retired instructions, route and parse        | 7,377 | 6.31M |            855x fewer |
| Wall time, argv to parsed value              | 0.7us | 544us |           788x faster |
| Heap allocations, no values bound            |     0 | 6,560 | no parser allocations |
| Heap allocations, three or four values bound |   3-4 | 6,560 |   one per owned value |

The original launch targets were fewer than 100k retired instructions, less
than 50us, and zero allocations when no value is bound. All three pass.

### How the instruction count moved

The number above is not monotone, and the shape of the curve is the useful part.
It was 50.9k at launch, rose to 63.8k, and is now 7,377.

The rise looked like the price of vocabulary, and was not. Two costs scaled with
the size of the whole CLI rather than with what the user typed:

- `Partial` is the entire CLI's accumulator, every command's fields inlined
  recursively, which is 11KB at mise's scale. `read_argv`, `read` and
  `parse_from` each returned one by value, so a parse performed four copies of
  it and spent about 87% of its instructions copying. `read_argv_into` and
  `read_into` take `&mut` instead: 63.8k to 18.5k.
- `Subcommands::Partial` was a struct with a field per variant, so constructing
  it materialized all 211 commands' accumulators when 210 of them were
  unreachable by construction. It is an enum now, either `Unselected` or the one
  variant being filled: 18.5k to 4.2k, with the type itself falling from 11,000
  to 824 bytes and data references from 116,742 to 1,889.

Because `Partial` was copied four times per parse, every property the derive
learned to express widened a struct that was already being copied, so ordinary
vocabulary growth arrived multiplied. Removing the copies removed the
multiplier: metadata a parse does not read now costs a parse nothing.

The remainder of the movement is the fixture rather than the parser. Measuring
one parser against both fixtures separates them: the current parser scores 4,907
against the pre-refresh spec and 7,377 against the current one, which now comes
from mise's real typed command tree and carries more positional arguments and
per-command metadata. clap grows about 7% across the same change. usage grows
more in proportion only because its own cost is now small enough for the
fixture's shape to dominate it.

## What clap's number includes

For this generated command tree, clap's approximately 544us consists of about
343us constructing the tree, 178us validating it, and 23us parsing. usage's
tables are compiled, so its 0.7us path has no equivalent construction or
validation phase. Even compared only with clap's already-built parse phase, with
the tree constructed and paid for, usage is about 34x faster in this
measurement.

## argh and bpaf

clap is not the only comparison the gate builds. The same mise spec is shadowed in argh's and
bpaf's vocabulary, and the same differencing measures all four parsing `mise use -g node@20`
(one run of `tasks/perf-shadow.sh`; counts vary slightly run to run):

| Framework | Instructions, cold parse | vs usage | Wall time (min) |
| --------- | -----------------------: | -------: | --------------: |
| usage     |                    7,409 |        — |          350 ns |
| argh      |                    6,292 |     0.8x |          275 ns |
| clap      |                6,307,681 |     851x |          513 µs |
| bpaf      |               21,909,001 |   2,957x |         1.61 ms |

The table splits into two classes. clap and bpaf construct and validate a parser at runtime
before reading a single word, so their floor is hundreds of microseconds. argh and usage read
static tables, so their floor is hundreds of nanoseconds.

Within the static class, argh is slightly cheaper — and expresses far less. Each shadow
intentionally drops what its framework cannot say, and argh's drops the most: environment
fallback, declared defaults, hidden and global flags, `choices`, flag relationships, aliases,
and non-UTF-8 argv (argh parses `&[&str]`). usage's number carries the full grammar, the spec,
and clap-shaped help and errors. The claim is not that usage is the cheapest parser possible;
it is that carrying everything costs the same class as carrying almost nothing.

## Binary size

The gate's parse-only binaries — each linking its framework's mise-scale shadow, built by the
same workspace release build, stripped:

| Framework | Stripped binary |
| --------- | --------------: |
| argh      |          1.0 MB |
| usage     |          1.5 MB |
| bpaf      |          2.5 MB |
| clap      |          3.1 MB |

The ordering matches the dependency story: usage links no third-party crates, clap links eight.
The same expressiveness caveat applies to argh's number as to its instruction count. For what
the spec endpoint itself weighs, see [Spec output](/rust/spec#the-endpoint) — 65 KB on a small
CLI, and `#[usage(spec_endpoint = false)]` removes it.

## Method and limits

- Instruction counts come from Cachegrind, whose run-to-run variation on the
  benchmark host is much lower than wall-clock timing.
- `tak` runs the release binaries repeatedly and reports the difference between
  the no-parse and parse paths.
- Every shadow is generated from the same spec, and each intentionally drops what
  its framework cannot express — so a framework is measured on its own vocabulary,
  and the drops are themselves part of the comparison.
- Binary sizes are the gate's `parse-n*` binaries from a full workspace release
  build, stripped. The workspace build matters: cargo unifies features, so clap
  gets the features (color, suggestions, help, derive, env) a real CLI of this
  size enables.
- This measures routing and parsing, not process startup, configuration loading,
  command execution, help rendering, or completion generation.
- The mise fixture changes over time, both by growing and by being refreshed
  from mise's current command tree, so one commit's count is not directly
  comparable with another's. CI protects the absolute instruction target, and
  `tasks/perf-shadow.sh` warns when the parser-to-parser ratio falls below 80x,
  which is the measure that belongs to the parsers rather than to the fixture.

The benchmark sources live in `benches/gate`, with generated shadows under
`benches/shadows`.
