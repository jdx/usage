# The rendering corpus

> The corpus one directory up is about parsing a command line; [`config/`](../config/README.md)
> is about resolving a CLI's configuration. This one is about what a spec _reads as_: the usage
> line, `-h`, and `--help`.

Test vectors for help rendering. Each one pairs a spec with the text rendering it must produce.

Plain JSON, for the same reason the argv corpus is: an implementation in any language can run
these without reimplementing a test format. If you are rendering help from a usage spec — in
Go, in JavaScript, or as a second Rust implementation — this directory is the definition of
correct.

## Why this exists beside the fleet gate

`benches/gate/tests/help.rs` renders all 211 of mise's commands with `usage-argv` and compares
each page against usage-lib byte for byte; `fleet.rs` beside it does the same for the other six
jdx CLIs. Together they are the check that decides whether an adopter's help output changes, and
no hand-written corpus will match them for scale.

What they cannot do is cover a shape no CLI in the fleet uses. The fleet was one CLI until #972,
and widening it found three bugs in a week — the version banner, an optional flag value, a
description ending in a break. That is the argument for this corpus rather than against it:
seven real CLIs still leave gaps, and the gaps are not exotic.

Flag values are the worked example. There are four pairings of "must the flag be given" against
"must its value be given", and across all 809 value-taking flags in mise and the fleet:

| pairing                                                  | in the fleet                 |
| -------------------------------------------------------- | ---------------------------- |
| `[--tool <TOOL>]` optional flag, required value          | 796                          |
| `<--v <n>>` both required                                | 8                            |
| `[--opt [n]]` both optional                              | 5, all in pitchfork and aube |
| `<--jobs [n]>` required flag, optional value             | **0**                        |
| a value carrying a `default`, which relaxes its brackets | **0**                        |

The third row is what #969 fixed, and pitchfork is the only reason it was visible. The last two
rows are shapes a spec can declare that nothing in the fleet does — so nothing but a written-down
case will hold them.

So the two are complements:

|                | asks                                                 | covers                             |
| -------------- | ---------------------------------------------------- | ---------------------------------- |
| the fleet gate | do seven real CLIs still render the same?            | those CLIs, exhaustively           |
| this corpus    | does every shape a spec can declare render the same? | every shape, one command at a time |

A rule that only one of them can catch belongs in whichever one catches it. In practice: if you
fix a rendering bug, the regression test goes _here_ unless a fleet CLI already exercises the
shape — and if one does, add it there instead, where it is checked at scale.

## Format

One file per area of rendering. Each has a `section`, an `about` explaining what the group
establishes, and `vectors`:

```json
{
  "id": "flag-optional-value-optional",
  "doc": "A value declared `[n]` is square-bracketed, independently of the flag's own brackets.",
  "spec": "name \"ex\"\nbin \"ex\"\nflag \"--opt [n]\"\n",
  "expect": { "usage": "ex [--opt [n]]" }
}
```

| field       | meaning                                                                                                         |
| ----------- | --------------------------------------------------------------------------------------------------------------- |
| `id`        | unique across the corpus, and stable — reports quote it, so renaming one breaks anybody tracking known failures |
| `doc`       | what this vector pins down, in one sentence                                                                     |
| `spec`      | a complete spec, as KDL                                                                                         |
| `cmd`       | which command's page, as the path below the root; absent means the root's own                                   |
| `expect`    | the rendering; see below                                                                                        |
| `reference` | whether usage-lib agrees; see below                                                                             |

### `expect`

`usage` is required and is the `Usage:` line's body **including the binary** — `ex go <TOOL>`,
not `go <TOOL>`. It is the shortest thing that can carry a shape, and every vector has an
opinion about it.

`short_help` and `long_help` are optional, and are the whole page as an array of lines. Lines
rather than one escaped string on purpose: a JSON file holding `"\n\nFlags:\n      --opt [n]\n"`
is not something a reviewer can read, and a diff over it cannot say which line moved. A page is
pinned in full or not at all — a truncated expectation would pass against a page that carries
on past it.

Pin a page when the vector is about a _section_: which entries appear, what column they sit in,
where an annotation goes. Pin only the usage line when it is about how one entry is written,
since the line contains that too and a whole page would bury it.

`long_help` is rendered at 80 columns, which is what every implementation falls back to when
`COLUMNS` is unset. Nothing here reads the real environment.

### What usage-argv is not asked

A few words reach usage-lib and cannot reach usage-argv at all. `disable_help` is the one so
far: it turns the parser's answer to `-h` off, and it is KDL-only — there is no derive spelling,
so no spec a `#[derive(Cli)]` binary carries can declare one, and the two renderers cannot
disagree about it in the wild.

This corpus breaks that premise, because it builds usage-argv's tables from KDL rather than from
a Rust type. So a vector declaring one is answered by the reference alone and skipped for
usage-argv, rather than being recorded as a divergence: nothing usage-argv rendered would be
right, because the question never reaches it.

Nothing marks these in the JSON — the harness works it out from the spec, since the exemption is
a property of the word rather than of the vector. What is asserted is the _count_, in
`conformance/tests/render.rs`. An exemption is a claim that a question does not reach an
implementation, and a set that can grow without anybody noticing is a set that will.

### The `reference` field

Same contract as the argv corpus. usage-lib is one implementation of these rules; where it
differs from what a vector expects, the vector says so:

```json
"reference": {
  "diverges": "usage-lib pads the supplied `help` entry into the commands column and usage-argv does not."
}
```

Absent means usage-lib agrees. `conformance/tests/render.rs` checks every label in both
directions, so a divergence that gets fixed fails the suite with an instruction to delete the
label, and the list cannot quietly rot into folklore.

## Running them

```sh
cargo test -p usage-conformance --test render
```

To see what each implementation actually renders, rather than only whether it matches:

```sh
cargo run -p usage-conformance --bin render-oracle
```

```sh
cargo run -p usage-conformance --bin render-oracle -- --json
```

```sh
cargo run -p usage-conformance --bin render-oracle -- flag-value
```

## Adding a vector

Write the spec and the `doc` first, with the rendering the rules say you should get. Then run
the oracle: `--json` prints what both implementations produced, in the shape `expect` takes, so
a page goes in as a measurement rather than a transcription.

If the two agree and match what you expected, you are done. If they agree and you expected
something else, you have found a rule you had wrong — or a rule worth changing, in which case
change it and keep the vector. If they _disagree_, that is the case this corpus exists for:
decide which is right, fix the other, and leave the vector behind as the thing that would have
caught it.
