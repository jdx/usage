# Plan: a compiled parser, and a config layer, for usage

Working plan for two related pieces of work. Boxes get ticked as things land, so
this file is also the status: if a box is unchecked, that thing does not exist.

1. **A compiled argv parser** — a Rust parser that reads static tables instead of
   building a command tree, with the usage spec as its source of truth.
2. **A config layer** — one implementation of the settings model that mise, hk,
   pitchfork, and fnox have each rebuilt separately, declared once and lowered
   into the spec.

The first is underway. The second is designed but not started, and deliberately
waits for the first to prove itself.

## Why

Derive-based CLI frameworks describe a command tree at runtime: the macro expands
into code that _builds_ an object graph — every subcommand, flag, help string,
alias, constraint — which a generic engine then interprets in order to parse one
command line. The construction is paid on every invocation, whether or not the
command that ran needed any of it.

At [mise](https://mise.jdx.dev)'s size — 210 commands, 711 flags, 339
positionals — that is roughly **3.1M instructions** for tree construction and
about half of the **~1.1ms** spent getting from `argv` to a parsed struct. mise
already maintains two hand-written argv scanners to avoid building the tree on
its hottest paths, and a comment in its source explains that deriving one of them
from clap "is what made every mise command ~6.3M instructions more expensive".
That workaround existing is the argument for this project.

serde faced the same fork and took the other branch: its derive emits a
monomorphic function specialized to your type, with no runtime model of the type
at all. Nobody asks serde for a schema compiler. This applies that shape to
`argv`.

**Runtime is the goal.** Compile time is a bonus: a derive emitting a small
monomorphic function should generate less IR than one emitting builder calls into
a generic engine, and if that holds it gets measured and advertised. If it does
not, the runtime case stands alone.

## How it is arranged

```
usage spec (KDL)  ←──────────────  the canonical model
      │                                    ↑
      │ reference                          │ emitted losslessly
      ↓                                    │
usage-lib ── interprets a spec       usage-derive ── reads a Rust type
      │      at runtime                     │
      │                                     ├─→ static tables ─→ usage-argv (hot)
      └─→ the oracle the corpus             └─→ static metadata ─→ help,
          measures everyone against              completions, spec output (cold)
```

Four rules hold this together.

**Code authors, the spec defines.** A Rust type is the authoring surface; the
spec is the semantic model. Anything the derive can express must lower losslessly
into the spec — and if the spec cannot express it, the spec gets extended first.
That keeps the emitted KDL a definition rather than a lossy summary, which is
what everything downstream (docs, manpages, completions, SDKs, agents) reads.

**usage-lib is the reference implementation.** It interprets a spec at runtime,
which is exactly what [the grammar](https://usage.jdx.dev/spec/argv) describes,
so it is the oracle. The corpus measures every implementation against it and
records disagreements per case rather than assuming they do not exist.

**The hot path stays small.** Binding only: which token becomes which flag or
argument. Help text, error rendering, spec emission, and every check that needs a
value's type live on cold paths in separate tables, so a successful parse never
touches them.

**End users never need another binary.** Help and completions are served from the
CLI itself. `usage` the CLI stays a maintainer's build-time tool for docs,
manpages, and SDKs — never a runtime dependency of somebody else's program.

## Milestones

### Done

- [x] **The grammar, written down** (#797) — `docs/spec/argv.md`. Token
      classification, the single left-to-right pass, flag forms, positional
      filling, subcommand routing and scope, `--`, and the error classes.
- [x] **A conformance corpus** (#797) — JSON vectors, language-neutral so any
      implementation can run them. Each records whether usage-lib agrees, as a
      measurement; the suite fails if a label is wrong in _either_ direction, so a
      divergence that gets fixed reports itself instead of rotting.
- [x] **`usage-argv`** (#798) — the binding parser. No tree, one pass, zero
      allocations on success _and_ failure, proved by a counting allocator rather
      than asserted. Answers the binding vectors, including every one where
      usage-lib diverges from the grammar.
- [x] **Released on the shared version** (#798, #803) — `usage-argv` and
      `usage-derive` are ordinary members of the workspace at the shared version
      rather than 0.0.0 curiosities outside the release cycle.
- [x] **Repeatable vs. variadic flags, and `double_dash_seen`** (#799) — a
      repeatable flag was greedy enough to eat a positional, and `automatic` mode
      reported a separator nobody typed.

### Next: the derive

- [x] **Static metadata tables** — a second, cold tree holding what the hot one
      deliberately omits: help and long help, about text, hidden-ness, visible and
      hidden aliases, value names, `choices`, defaults, `env`, effects, mounts,
      restart tokens, examples. Behind the `spec` feature, and each entry borrows
      the parse-table entry it describes, so names have one definition and cannot
      drift.
- [x] **KDL emission** — `Spec::to_kdl`, written by hand so the crate keeps having
      no dependencies. Verified by parsing the output back with usage-lib and
      checking the resulting spec field by field, then rendering it through the
      markdown and manpage generators an adopter's docs build actually uses.
- [x] **`usage-derive` v0** — flags, positionals, doc-comment help (first
      paragraph short, whole block long), and spec emission, from usage-native
      attributes. Unsupported field types are a compile error rather than a
      surprise, and the messages point at the offending field.
- [x] **Subcommands in the derive** — an enum of variants, each holding the struct
      that declares its flags, nested to any depth. A command is selected by its
      position in the parent's table, found from the address the parser hands back.
- [ ] **Typed values** — fields are text today (`String`, `Option<String>`,
      `Vec<String>`, `bool`, counting integers). Parsing into other types needs an
      error type for value conversion, which is also where `env`, required-ness,
      and `choices` get enforced — see the post-binding layer below.
- [ ] **`usage-derive` v1** — everything mise needs: constraints
      (`requires`/`conflicts`/`overrides`/`required_unless`), `var`, `count`,
      `env`, defaults, delimiters, the `double_dash` modes, global flags, flatten,
      boxed subcommand variants, headings, `cfg`-gated variants.
- [x] **The post-binding layer** — `required`, `choices`, `env` fallback, defaults,
      `var_min`/`var_max`, `conflicts`, `required_if`, `required_unless`, and
      `overrides`. All of them need a value's type, so they belong with the derive
      rather than in the parser. `overrides` is the one that happens _during_ the
      parse instead of after it: it asks which of two flags came last, which only the
      arriving token knows.

### Spec gaps found on the way

Each of these is a thing a CLI wants to say that the spec has no way to record.
Per the canonicality rule the spec gets extended first, so these block the derive
carrying them rather than being worked around.

- [x] **`help_heading`** — grouping flags under a heading in help output. Now a
      spec field on both flags and arguments, and the clap bridge no longer drops
      it, which it had been doing for every CLI that groups its flags.
- [x] **Rendering headings** — help output and generated markdown both group by
      heading now. Unheaded entries keep the default section and come first, and a
      heading with nothing visible in it produces no section.
- [x] **`conflicts`** — the spec could say `overrides`, `required_if` and
      `required_unless`, but not that two flags must not be given together, so the
      forty `conflicts_with` relationships mise declares in clap were being dropped
      by the bridge. Now a flag property, enforced by usage-lib, and a value from
      the environment counts on both sides of the check as clap does.
- [x] **An argument after a variadic, when it needs a `--`** — the derive refused the
      shape mise uses on `run`, `exec` and `git`: `[ARGS]…` for the words before the
      separator and `[-- ARGS_LAST]…` for the ones after. Both parsers already bound it;
      only the derive's validation disagreed. Found by compiling the mise shadow.
- [ ] **A mount on the root command** — the spec accepts `mount` only inside a
      `cmd` block, so a CLI whose _top-level_ subcommands are discovered by running
      something cannot say so. Worth deciding whether that is a gap or a deliberate
      restriction.

### Then: what a CLI framework has to have

- [ ] **Help rendering** — `--help` and `-h` from the static metadata, with
      headings and hidden-item filtering.
- [ ] **Completions, self-contained** — `<bin> completion <shell>` emits the
      script; a hidden `<bin> complete-word` serves requests from the binary's own
      embedded spec. Same dispatch shape usage-cli uses today, without requiring
      `usage` on the end user's machine. bash, zsh, fish first.
- [ ] **Docs and manpages** — no new code: confirm the emitted KDL feeds
      `usage g markdown|manpage` exactly as a clap-derived spec does today, so an
      adopter's docs pipeline does not change.
- [ ] **Diagnostics** — rich errors behind a feature. The hot path returns a
      compact code; rendering re-examines the command line only once it has
      already failed.

### The gate

Everything above is speculative until this passes. Baseline is mise's current
clap parser at mise's real scale, using a shadow CLI generated from mise's
checked-in `mise.usage.kdl` for both parsers.

- [x] **Shadow generation** — `xtask gen-shadow` turns any `.usage.kdl` into a crate
      of derived types. mise's committed 5,592-line spec compiles: 211 commands, 711
      flags, 128 arguments, four levels deep, in 2.6s. Seventeen things are dropped and
      the generator names them — 13 secondary flag aliases, 3 `double_dash="automatic"`,
      1 default on a collecting flag.
- [ ] **Bench harness** — the clap-equivalent shadow to measure against, `tak` gating in
      CI, and criterion for wall clock. A first measurement of the usage side alone, at
      mise's full scale, is 100k–106k instructions per parse for the parse itself
      (process total minus a null binary that does everything but parse), near-constant
      across invocation shapes — as static tables should be, with no tree to build.
- [ ] **Differential fuzzing** — proptest over argv against usage-lib on the mise
      spec, to find disagreements the corpus did not think of.
- [ ] **Perf report** — published honestly, whichever way it goes.

Runtime targets, which gate:

| measurement                        | clap baseline                     | target |
| ---------------------------------- | --------------------------------- | ------ |
| instructions, route + parse        | ~3.1M for tree construction alone | < 100k |
| wall time, argv to parsed struct   | ~1.1ms                            | < 50µs |
| heap allocations, successful parse | thousands                         | 0      |

Secondary, measured and reported but not gated: compile time (full and
incremental) and binary-size contribution against an equivalent clap-derived
shadow.

If the runtime targets miss by a wide margin, the honest outcome is to write that
down and stop. Nothing gets integrated into mise before this point.

### After the gate

- [ ] **mise** — the largest and least forgiving adopter. Likely a router first
      (which also retires mise's two hand-maintained flag tables), then commands
      lowered a few at a time, with mise's e2e argv corpus replayed against both
      parsers.
- [ ] **hk, pitchfork, fnox** — smaller, and all three already generate their
      spec from clap, so they are the natural second adopters.
- [ ] **Other languages** — the grammar and corpus are language-neutral on
      purpose. A Go, JavaScript, or Python implementation is verified by running
      the corpus, not by reading this repository's Rust.

## Not covered by the corpus yet

- [ ] **Restart tokens** — `restart_token` (mise's `:::`) makes one command line
      describe several invocations. `expect` holds a single result, so this needs a
      multi-invocation vector shape first.
- [ ] **Mounts** — `mount` resolves a sub-spec by running a command mid-parse,
      which makes a vector depend on an external process. Needs stubbing.
- [ ] **Completion parsing** — `parse_partial` accepts deliberately incomplete
      input. Different contract, different expectations, its own corpus.

## Known usage-lib divergences

The corpus records these; they are bugs to fix or decisions to revisit, not
settled behavior. Each is a small change to `lib/src/parse.rs`, and the corpus is
how a fix gets verified — including telling you to delete the label afterwards.

- [ ] Unrecognized flags fall through to positionals, so `ex --wat` binds `--wat`
      to an argument, or reports `unexpected_arg` when there is none. This is the
      root of most of the recorded divergences. **Needs a decision**: mise parses
      task arguments with this parser at run time, so rejecting an undeclared flag
      would change what a task accepts, not just what a completion offers.
- [x] A flag missing its value is dropped silently — now an error, in `parse` but
      not `parse_partial`, since a half-typed flag is exactly what a completion is
      asked about.
- [x] `=` is kept in attached short values, so `-j=8` binds `=8`.
- [ ] A repeated `--` is eaten, so a forwarded command line containing its own
      separator is altered in transit. **Needs a decision**: an existing test
      asserts this, and `double_dash="preserve"` is the declared way to keep
      separators, which may make it intentional.
- [x] `--jobs=` binds nothing rather than the empty string.
- [ ] A flag with a variadic argument rejects its second value, though
      [the flag reference](https://usage.jdx.dev/spec/reference/flag) documents the
      form.
- [ ] `double_dash="automatic"` is not enforced, which
      [the arg reference](https://usage.jdx.dev/spec/reference/arg) says outright.

## Config

Not started. Design first, implementation after the parser gate.

### What the four CLIs already do

mise, hk, pitchfork, and fnox have each independently built the same thing, and
the agreement is strong enough to standardize:

- A **TOML registry** (`settings.toml`) drives codegen — at the repo root in
  mise, hk, and pitchfork, and in the crate root in fnox.
- **`build.rs` generates, `include!(OUT_DIR)` delivers.** All four emit a typed
  `Settings` struct plus a `SETTINGS_META` map for introspection; three also
  generate the merge logic, while mise delegates that to confique.
- **The field vocabulary is ~80% shared**: `type`, `default`, `description`/`docs`,
  `examples`, `deprecated`, `since`, `env`.
- **The layering agrees** wherever a given layer exists: project (found upward)
  over user-global over defaults, with a `*.local.*` sibling outranking its base.

Where they differ is instructive, because it is mostly _drift_:

- **Every one of them hand-writes the CLI-to-settings binding**, and every one has
  a hole in it. hk declares `sources.cli` for flags nothing reads. pitchfork
  documents a CLI layer it does not have — the precedence list in its
  `--help` is copied by hand and lands verbatim in its committed spec. fnox
  resolves `age_key_file` through a hardcoded five-way chain in `providers/age.rs`
  because its settings and its config files are two disconnected systems.
- **Only hk can answer "where did this value come from"** (`hk config explain`),
  and it needed a second parallel merge function to do it.
- **Only hk validates its own registry file** (a JSON schema wired through taplo).
- **Docs and JSON-schema generation is three separate reimplementations**, and
  fnox has none.
- **hk's project config is pkl**, resolved by a subprocess and cached as JSON, with
  settings living as top-level keys of the same file as its hook config. Any shared
  design has to treat "project file" as a pluggable loader producing a value tree,
  not as "parse TOML at a path".

### The shape

- [ ] **Declare props in code**, `#[derive(usage::Config)]`, lowered into the
      spec's `config { prop ... }` block so settings documentation flows through
      the same pipeline as command documentation. Same canonicality rule as the
      parser: code authors, the spec defines.
- [ ] **A prop vocabulary that is the union of the four registries** — `type`
      (bool, int, string, path, duration, list, map, plus a Rust-type escape
      hatch), `default`, `env` and `deprecated_env`, `docs`, `deprecated` with
      warn/remove versions, `enum`, `optional`, `aliases`, `merge`
      (`replace`/`union` — hk needs union for its list settings), `scope`
      (mise strips `global_only` settings out of project files, which is a
      security property, not a preference), and per-source bindings in hk's
      `sources.{cli,env,git,...}` shape.
- [ ] **Named, ordered, pluggable layers.** The order is CLI flags, then
      environment, then env-files, then the project file (found upward, with
      `.local` variants outranking their base), then user-global, then system, then
      defaults. That already matches all four CLIs wherever a layer is present;
      which layers exist stays per-CLI, so hk's git-config layer and mise's `/etc`
      and `conf.d` layers slot in without being universal.
- [ ] **Generate the CLI binding** rather than hand-writing it. This is the single
      highest-value piece: it is what all four wrote by hand and all four got
      subtly wrong.
- [ ] **Provenance through one merge path**, so `<bin> config explain` comes free
      everywhere instead of needing a parallel implementation.
- [ ] **Extend `SpecConfigProp` first.** The spec's `config` block exists and
      **no CLI emits or consumes it today**; it is also missing `deprecated`,
      `enum`, `optional`, `aliases`, `merge`, scope, and the per-source lists that
      are load-bearing in at least one of the four. Spec-first, per the
      canonicality rule.
- [ ] **A registry JSON schema**, so the declaration format validates itself the
      way hk's does.

### Open questions

- [ ] Whether config lives in this repository or beside the parser crates. It is
      not argv parsing, but it shares the spec, the codegen, and the docs
      pipeline.
- [ ] Whether the four CLIs migrate incrementally (one layer at a time, keeping
      their generated `Settings`) or by regenerating from a converted registry.
      Incremental looks far more likely to actually happen.
- [ ] fnox's model, where config files are not a settings source at all, is the
      one real behavior change rather than a consolidation. Worth confirming that
      is a fix and not a deliberate choice.
