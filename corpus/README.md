# The argv conformance corpus

> Resolving a CLI's _configuration_ — layers, precedence, merge policies — has a
> corpus of its own in [`config/`](config/README.md). This file is about parsing
> a command line.

Test vectors for [the argv grammar](https://usage.jdx.dev/spec/argv). Each one
pairs a spec with a command line and the result parsing them must produce.

These are plain JSON so that an implementation in any language can run them
without reimplementing a test format. If you are writing a usage parser — for Go,
for JavaScript, for Python, or as a second Rust implementation — this directory is
the definition of correct, and passing it is what "compatible" means.

[usage-go](../go/README.md) is the first parser to take that offer up: it answers
the binding vectors from a Go test runner rather than a Rust one, which is the
claim this format exists to make checkable.

## Format

One file per area of the grammar. Each has a `section`, an `about` explaining what
the group establishes, and `vectors`:

```json
{
  "id": "long-value-attached",
  "doc": "`--flag=value` binds the text after the first `=`.",
  "spec": "name \"ex\"\nbin \"ex\"\nflag \"--jobs <n>\"\n",
  "argv": ["--jobs=8"],
  "expect": { "ok": { "flags": { "jobs": "8" } } }
}
```

| field       | meaning                                                                                                         |
| ----------- | --------------------------------------------------------------------------------------------------------------- |
| `id`        | unique across the corpus, and stable — reports quote it, so renaming one breaks anybody tracking known failures |
| `doc`       | what this vector pins down, in one sentence                                                                     |
| `spec`      | a complete spec, as KDL                                                                                         |
| `argv`      | the command line, excluding the program name                                                                    |
| `env`       | the environment the parse sees; absent means empty                                                              |
| `expect`    | `{"ok": {...}}` or `{"error": "code"}`                                                                          |
| `reference` | whether usage-lib agrees; see below                                                                             |

`expect.ok` holds `cmd` (the selected subcommand path, outermost first, omitted
for the root), `flags`, and `args`. Both maps are keyed by the name the spec gives
each flag or argument, never by the token that set it, so `-j`, `--jobs`, and an
`env` var all land under `jobs`. Anything unset is omitted rather than null.

Values are strings, lists of strings, booleans, or lists of booleans — a list of
booleans being how a `count` flag records its occurrences. The grammar decides
which token binds where, not what it means, so `"8"` stays a string and
converting it is the caller's business.

`expect.error` names a class of failure, never a message. Wording is a
quality-of-implementation concern; the class is what the corpus pins, so that a
strict parser and a lenient one differ visibly. The codes are listed in
[the grammar](https://usage.jdx.dev/spec/argv#errors).

Vectors that need an environment carry it. Nothing here reads the process
environment, so no vector's result depends on the machine running it.

## The `reference` field

The grammar is the intent; usage-lib is one implementation of it. Where they
differ, the vector says so:

```json
"reference": {
  "diverges": "usage-lib parses successfully and binds nothing, so `ex --jobs` silently does what `ex` does."
}
```

Absent means usage-lib agrees. `conformance/tests/reference.rs` checks every
label in both directions: a vector claiming agreement must agree, and a vector
claiming divergence must still diverge. So a divergence that gets fixed fails the
suite with an instruction to delete the label, and the list cannot quietly rot
into folklore.

If you are implementing the grammar elsewhere, treat `expect` as the target and
read the `diverges` notes as the compatibility risks you inherit if you copy
usage-lib's behavior instead.

## `layer`

Which layer of a parser the vector is a question for, and therefore whether your
implementation is expected to answer it.

`binding` — the default, and most of them — is about which token becomes which flag
or argument. Any parser that reads argv should answer these.

`post-binding` is decided once the last token has been read: `required`, `choices`,
`env` fallback, defaults, `var_min`/`var_max`, `overrides`. They need to know a
value's type, so a parser that only binds tokens can skip them and leave them to
whatever owns the target type.

It is stated per vector rather than left to be inferred from the spec. A vector
whose spec declares `choices` can still be an ordinary binding question — `--shell
zsh` binds `zsh` whatever the choice list says — and guessing from the declaration
exempted vectors nobody meant to exempt.

## Running them

In this repository:

```sh
cargo test -p usage-conformance
```

To see what usage-lib actually does with each vector, rather than only whether it
matches:

```sh
cargo run -p usage-conformance --bin oracle          # human-readable
cargo run -p usage-conformance --bin oracle -- --json
cargo run -p usage-conformance --bin oracle short-   # ids containing "short-"
```

## Adding a vector

Write the case with the result the grammar says it should have — not the result
you observe. Then run the oracle: if usage-lib agrees, leave `reference` out; if
it does not, record what it does instead. The note is the deliverable, since that
is what a migration reads.
