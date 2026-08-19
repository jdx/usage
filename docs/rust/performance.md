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
| Retired instructions, route and parse        | 50.9k | 5.96M |            117x fewer |
| Wall time, argv to parsed value              | 2.1us | 490us |           238x faster |
| Heap allocations, no values bound            |     0 | 6,560 | no parser allocations |
| Heap allocations, three or four values bound |   3-4 | 6,560 |   one per owned value |

The original launch targets were fewer than 100k retired instructions, less
than 50us, and zero allocations when no value is bound. All three passed.

A later measurement moved usage to 60.4k instructions while clap remained
approximately flat at 5.90M, reducing the ratio from 117x to 97x. The parser is
still comfortably under the absolute gate, but the change is reported because
the ratio is useful evidence that vocabulary growth is not free.

## What clap's number includes

For this generated command tree, clap's approximately 490us consists of about
305us constructing the tree, 160us validating it, and 24us parsing. usage's
tables are compiled, so its 2.1us path has no equivalent construction or
validation phase. Even compared only with clap's already-built parse phase,
usage was about 12x faster in this measurement.

## Method and limits

- Instruction counts come from Cachegrind, whose run-to-run variation on the
  benchmark host is much lower than wall-clock timing.
- `tak` runs the release binaries repeatedly and reports the difference between
  the no-parse and parse paths.
- Both shadows are generated from the same spec and intentionally drop the same
  unsupported properties.
- This measures routing and parsing, not process startup, configuration loading,
  command execution, help rendering, or completion generation.
- The mise fixture grows over time. CI protects the absolute instruction target;
  this report also records the parser-to-parser ratio so regressions remain
  visible when the fixture changes.

The benchmark sources live in `benches/gate`, with generated shadows under
`benches/shadows`.
