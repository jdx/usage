# The completion corpus

> The corpus two directories up is about what a command line _binds_;
> [`render/`](../render/README.md) is about what a spec _reads as_; [`config/`](../config/README.md)
> is about resolving a CLI's settings. This one is about what could go where the cursor is.

Test vectors for completion. Each pairs a spec with a partially typed command line and the
candidates an implementation must offer.

Plain JSON, for the same reason the others are: an implementation in any language can run these
without reimplementing a test format. If you are answering `complete` from a usage spec — in Go,
in JavaScript, or as a second Rust implementation — this directory is the definition of correct.

## Why this exists

Completion is the one area with three implementations and no shared fixture. Parsing has the
corpus above; rendering got one in `render/`; completion has `argv/src/complete.rs` tested by its
own unit tests, `cli/src/cli/complete_word.rs` tested by its own, and a Go implementation landed
in #984 tested by a third set. Three sets of tests written against three readings of the same
rules is the arrangement that produced every drift this project has had to chase — the help
renderers agreed on mise and differed on five of the other six CLIs until #972 held them to one
fixture.

It also closes the two easier thirds of the corpus gaps that were still open: completion
parsing, which is `parse_partial` over deliberately incomplete input, and restart tokens, which
only matter at a cursor. Mounts remain uncovered, and deliberately — resolving one _runs a
command_, which a corpus cannot do hermetically. The differential fuzzer learned that the
expensive way: its first draft spawned real `mise` processes that fetched vfox metadata and
shelled out to `apt-cache`.

## What a vector says

```jsonc
{
  "id": "flags-after-a-dash",
  "doc": "A word beginning with `-` offers flags rather than subcommands or values.",
  "spec": "name \"ex\"\nbin \"ex\"\nflag \"--force\"\ncmd \"go\" {}\n",
  "line": "ex -",
  "expect": { "candidates": ["--force"] },
}
```

`line` is the command line as typed, and the cursor sits at its end — which is where a shell asks
from. A vector needing the cursor elsewhere says so with `cursor`, a byte offset into `line`.

`candidates` is the set an implementation must offer, order-insensitive: the order a shell shows
them in is the shell's business, and two implementations sorting differently is not a
disagreement about what completes. A vector that _does_ mean to pin order says
`"ordered": true`.

## Candidates that cannot be a fixed list

Some answers are not a list of words. `run=` shells out; a `complete` callback asks the binary
itself; `files` and `dirs` ask the filesystem. A vector cannot state those without becoming a
test of the machine it runs on, so it states the _kind_ instead:

```jsonc
"expect": { "files": true }
```

which asserts that the implementation defers to the shell's own file completion rather than
offering words of its own. What the filesystem then contains is not the corpus's business.

## Vectors that expect nothing

An empty expectation is almost always a spec that did not say what its author thought, so the
corpus refuses one unless the vector says the silence is deliberate:

```jsonc
"expect": { "nothing": true }
```

The case that needs it is [`03-external-boundary.json`](03-external-boundary.json): past an
`external_subcommand` catch-all the words belong to a program the spec does not describe, and
offering the catch-all's own subcommands, flags or the working directory would answer about the
wrong CLI. Offering nothing is the claim, path fallback included.

## Keeping it honest

Every vector carries a `reference` label, defaulting to `agrees`, saying whether `usage-cli` —
the reference implementation — produces exactly this. A vector the reference disagrees with must
say so and why, so a divergence is a recorded decision rather than a mystery. That is the same
rule the argv corpus runs on, and `conformance/tests/complete.rs` is what enforces it.
