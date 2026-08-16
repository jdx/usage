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
- [x] **Typed values** — a field is any `T: FromStr`, plus `Option<T>`, `Vec<T>` and
      `Option<Vec<T>>`, which is what lets a command struct hold the `PathBuf` mise names
      227 times and the tool-version type it names 83 times. Binding still collects text;
      the conversion happens where the struct is built, and a value that will not convert
      reports the text and the type's own message. `Error` grew one boxed variant and lost
      `Copy`, and stayed 40 bytes, so nothing on the hot path grew.
- [x] **Enumerated values** — `#[derive(usage::ValueEnum)]` on an enum of bare variants
      gives the words a value may be, and a field says `value_enum` to use them. mise has
      nine of these. The list is declared once, on the type: the spec, the help, the
      completions and the check that rejects a wrong word all read it from there, so none of
      them can drift from the type the way a second list on the field would.
- [x] **Values that are not valid UTF-8 are reported, not mangled** — the partial holds the
      bytes a word arrived as, and the conversion happens once where the struct is built, so
      `--out /tmp/\xff` says so instead of handing a `PathBuf` a path with `U+FFFD` in it —
      a different file, silently. Costs +656 instructions (1.6%) and one allocation, which
      is what not corrupting a value is worth. It also retired the hazard of recognising
      `String` by its spelling, since there is no identity case left.
- [x] **Accepting a value that is not valid UTF-8** — reporting it was the safe half; accepting
      it is the whole fix, because the operating system does accept `/tmp/\xff` as a filename and a
      CLI that cannot receive one cannot open the file. A `PathBuf` or `OsString` field takes the
      bytes exactly, through `usage_argv::os_string_from_bytes`. **And with no `unsafe` anywhere.** On
      Unix an `OsString` is an arbitrary byte sequence, so this is the safe `OsString::from_vec` and
      every byte survives — which is the case that matters, since non-UTF-8 filenames are ordinary
      there. Windows was going to need `from_encoded_bytes_unchecked`, and jdx approved that, but a
      _safe_ function taking a `Vec<u8>` cannot enforce its precondition: there is no way to know the
      bytes came from `as_encoded_bytes` rather than from anywhere else, and a safe function whose
      precondition a caller can violate is unsound however carefully today's callers behave. Greptile
      flagged exactly that on #844. So Windows goes through UTF-8 and reports what will not convert,
      which gives up only an unpaired-surrogate argument there. Cheaper than the text path, not
      dearer: a `PathBuf` field costs **553 instructions per parse against a `String` field's 674**,
      since it skips the UTF-8 validation pass, and both allocate once. The gate fixture cannot show
      this — a spec carries no Rust types, so every shadow field is a `String` — which is why it is
      measured directly.
- [x] **`flatten`** — one struct's declarations belonging to another command, which mise does
      ten times: `ConfigLs` is written once and given to both `config` and `config ls`. The tables are
      joined at compile time by `concat_flags`/`concat_args`, so the parser walks one flat slice and
      flatten costs nothing at run time; a flattened group lands at the field's position, which
      positional arguments require. Everything else is delegation through `CommandArgs` — partial,
      `start`, `apply`, `check`, `build` — which is also why nesting works without anything extra.
      Nothing new in the spec: the emitted KDL lists the flags inline, exactly as a hand-written
      command would. It turned up a bug worth more than the feature. `Subcommands::apply` offered
      every event to _every_ variant and took the first that claimed it, which was harmless only
      because keys were unique per command — and flatten breaks that, since two commands sharing a
      declaration share its key. So `config --no-header` bound the flag on the unselected `config ls`.
      Now only the _selected_ variant is asked, which is both correct and much cheaper: at mise's
      scale the root had ~100 variants, each asked per event. **29,850 instructions and 822 ns, from
      40,179 and 1,893 ns** — 26% fewer instructions and 57% less wall time, measured against the
      parent branch on one machine. 176× clap's instruction count. Allocations unchanged: 0 bare, 4
      bound. `duplicate_key` had to change with it: it asserted no key appeared twice in the whole
      tree, and sharing one across commands is now ordinary rather than suspect. Checked per command
      instead, which is the level a key actually decides anything at. The collision flatten _can_
      still cause — a parent and the struct it flattens both declaring `--quiet` — is invisible to
      both expansions, so `Spec::to_kdl` grew a duplicate-form check beside the key one, where the
      whole tree is visible. `Option<T>` flatten is refused rather than guessed at: it needs a rule
      for when the group counts as given, and nothing in the fleet asks for one. Two more checks live
      in `Spec::to_kdl` for the same reason as the duplicate-form one: an argument no word can reach —
      an unbounded variadic on one side of a flatten and a later positional on the other — and the
      flag-form collision. Both are invisible to either expansion and visible where the tables are
      joined.
- [ ] **`usage-derive` v1** — everything mise needs. `conflicts`, `overrides`,
      `required_if`, `required_unless`, `var`, `count`, `env`, defaults, the four
      `double_dash` modes, global flags, flatten, boxed subcommand variants,
      headings and `cfg`-gated variants have all landed since this was written.
      What is left of the original list is **`requires`** — with `requires_if` and
      `requires_ifs`, the conditional forms, which are their own work rather than
      a detail of the first — none of which any part of this repository can
      express: not the derive, not the spec, not the clap bridge. And
      **delimiters**, where a value's `,` is split at parse time rather than only
      in a clap default. Both are in the clap-parity list below.
- [x] **The post-binding layer** — `required`, `choices`, `env` fallback, defaults,
      `var_min`, `conflicts`, `required_if`, `required_unless`, and `overrides` —
      `var_max` moved to the binder, see the decision below. All of them need a value's type, so they belong with the derive
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
- [x] **Is `var_max` a limit or a check?** — usage-argv let a variadic take every word and
      judged the count afterwards; usage-lib stopped the variadic at the bound and gave the
      rest to the next argument. Two coherent readings, and nothing had recorded that the
      two implementations disagreed. Settled as a **limit**, matching usage-lib and clap's
      `num_args`, since every spec in the fleet is generated from a clap command and it is
      the only reading under which `[a]… [b]` can be filled. `var_max` therefore moved into
      the table binding reads; `var_min` stays a check, because no single word tells you a
      variadic will end up short. Costs 55 instructions per parse, or 0.1%.
- [x] **The command-level properties mise patches in by hand** — `default_subcommand`,
      `restart_token` and `mount`, declared with `#[usage(...)]` instead of edited into the
      spec afterwards by `src/cli/usage.rs`. None of the three changes how a word binds: a
      mount costs a subprocess and belongs to completions, a restart token is read by
      whoever splits an invocation into several, and a default subcommand tells a completion
      engine what a bare word means. So they are emission-only, and the test asserts both
      that they reach the KDL and that binding is unaffected. `default_subcommand` is
      checked at compile time to require a subcommand field, since it names one.
- [x] **Routing on `default_subcommand`** — the parser reads it now, rather than the property
      being emitted and ignored. A word naming no subcommand descends into the named command and is
      examined again there, so it can be that command's argument or one of _its_ subcommands. The
      declaring command's own positional does not win, which is what makes the property more than a
      synonym for an argument — and is mise's shape exactly. Taken at most once per parse, so a chain
      of defaults cannot walk a CLI deeper than the user typed. No measurable cost: 40,370
      instructions against 40,472 before, the branch being reached only by a word that matched no
      subcommand. The ±100 is code layout rather than work — cachegrind is deterministic, and gave the
      same figure twice. Allocations unchanged: 0 bare, 4 bound. Only a _word_ routes: a dash-prefixed
      token naming no flag arrives as a value and was never a candidate to select anything, so it
      binds where it was typed — as usage-lib does, which stops looking for subcommands at an
      unrecognised flag. The name may also be an alias, since usage-lib resolves it against names,
      aliases and hidden aliases alike. The name is resolved by `find_subcommand` during **const
      evaluation**, so a `default_subcommand` that no subcommand answers to is a compile error. That
      retires the claim in the entry above that the name could not be checked: the variants are indeed
      another expansion, but a `const fn` can search the list the parent already holds. **What it does
      not buy on its own:** `mise build` still does not work end to end in the shadow, because mise's
      spec gives `run` no positional at all — `src/cli/usage.rs` clears them and adds `mount run="mise
tasks --usage"`, so task names are meant to come from running that. usage-argv does not execute
      mounts. Routing _plus_ mounts is what would let mise delete its hand-rolled dispatch; routing
      alone is half of it. One divergence found and then **fixed in usage-lib** rather than recorded:
      it applied the single declared name at whichever command it was standing on, so `ex config zzz`
      descended into `config ls` when an unrelated `config` happened to have an `ls`. A spec declares
      one name, once, at the top. The corpus vector that recorded the difference is now an ordinary
      agreeing vector — the reference test refused to let the label stay, which is what it is for.
- [ ] **A mount on the root command** — the spec accepts `mount` only inside a
      `cmd` block, so a CLI whose _top-level_ subcommands are discovered by running
      something cannot say so. Worth deciding whether that is a gap or a deliberate
      restriction.

- [x] **Three things a spec could say that the derive could not** — a flag's value name
      (`--tool <TOOL>` came back as `--tool <tool>`, since the flag's own name was all there
      was), a collecting argument that needs at least one value (`<TARGET>…`, which a `Vec`
      cannot express because it has no bare-versus-`Option` shape), and `var` on a counted flag
      (a count _is_ repetition, so it is inferred now rather than needing told). All three were
      dropped by `gen-shadow` without being counted, which is the part that made them invisible:
      the report is the thing that was supposed to prevent exactly this. Found by rendering
      mise's help from the shadow and diffing every line against usage-lib's — 23 of 211 differed,
      and every difference traced to one of these. They matter past help text, since the emitted
      spec is what docs, manpages, completions and the SDK generators read.

### Then: what a CLI framework has to have

- [x] **Help rendering** — `-h` and `--help` both match usage-lib byte for byte across all 211 of
      mise's commands, and both are wired: the parser recognises them itself, after a command's own
      flags so a CLI that declares its own keeps it, and a request comes back as `Error::Help`
      carrying the command it was asked about. `parse()` renders and exits; `parse_from` returns
      it, so a library embedding this decides for itself. Costs 111 instructions, since the check
      is only reached by a flag that matched nothing. The `help` _subcommand_ landed with it:
      asked after the subcommand lookup, so a CLI declaring its own `help` keeps it, and the
      words after it name a command rather than being descended into. Holding the rendering to
      parity found ten things a spec could say that the derive could not, and two bugs in
      usage-lib's own renderer.
- [x] **Completions, self-contained** — `<bin> completion <shell>` emits the
      script; a hidden `__complete_word__` serves requests from the binary's own
      tables. Same dispatch shape usage-cli uses today, without requiring `usage`
      on the end user's machine. bash, zsh, fish and PowerShell, behind the
      `complete` feature and asked for with `#[usage(completion)]`.
- [ ] **Docs and manpages** — no new code: confirm the emitted KDL feeds
      `usage g markdown|manpage` exactly as a clap-derived spec does today, so an
      adopter's docs pipeline does not change.
- [x] **Diagnostics** — rich errors behind the `diagnostics` feature. The hot path returns a
      compact code; rendering re-examines the command line only once it has
      already failed, and says what clap would have said, colour included, down to
      suggesting what was probably meant. `parse()` renders and exits the way a
      program does; `parse_from` hands the error back.

### What clap can say that we cannot

An audit of clap's surface against `derive/src/model.rs`, `argv/src/` and the
spec model in `lib/src/spec/`, done once the framework list above was mostly
ticked. This is the list that decides whether an adopter can move without losing
behaviour, so a gap here is worth more than another point of speed.

Some of these are dropped by **`clap_usage` too**, which means every spec in the
fleet generated from a clap command is already lossy in exactly this way — the
same shape as the `conflicts` hole above, found the same way. Those are marked
_(bridge too)_ and are the reason this list is ordered as it is.

One of them the bridge will never carry. clap 4.6 gives `Arg::requires`,
`requires_if`, `requires_ifs` and `requires_all` as **setters with no getter**,
and keeps the field `pub(crate)`, so a `Command` cannot be asked what it
requires. It does appear in `Debug for Arg`, so scraping the debug format would
technically recover it — declined, because that format carries no compatibility
promise and would break by producing a wrong spec rather than a failed build.
The consequence is worth stating plainly: `requires` reaches a spec **only** by
being declared in usage, and a CLI that keeps its declaration in clap does not
have this constraint in its spec at all.

Groups are the opposite case: `Command::get_groups`, `ArgGroup::get_args` and
`Arg::is_exclusive_set` are all public, so the bridge gains those for free.

**Changes what a CLI does**

- [ ] **`requires` / `requires_if` / `requires_ifs`** — "this flag needs that
      one". The spec has `conflicts`, `overrides`, `required_if` and
      `required_unless`, and no way to say the positive form. Nothing in
      `lib/src/spec/flag.rs` parses it and the derive has no attribute. The one
      true blocker in the original v1 list. The bridge will not carry it, per the
      note above, so this is a reason to declare in usage rather than a bridge
      bug: a CLI that moves its declaration here gains a constraint its generated
      spec never had.
- [ ] **`ArgGroup`, and `exclusive`** _(bridge too)_ — "exactly one of these
      three", "at least one of these". Only pairwise `conflicts` exists, which is
      O(n²) declarations for what clap says once, and cannot express requiredness
      across a set at all. Readable from a clap `Command`, so the bridge gains
      these once the spec can hold them.
- [ ] **`value_delimiter`** — `--tags a,b,c` as three values. `lib/src/spec/arg.rs`
      splits a clap _default_ by it and says the spec has a list rather than a
      delimiter, which is true of a default and not of a command line: nothing
      splits a value at parse time.
- [ ] **`default_missing_value`, and optional-value flags** — `--color` versus
      `--color=always`, which is `Option<Option<T>>` in clap. `Shape` has no
      variant for it.
- [ ] **`default_value_if` / `default_value_ifs`** — a default that depends on
      another flag. Ours are unconditional.
- [ ] **`value_parser`** — clap takes an arbitrary parser function and range
      validators (`value_parser!(u16).range(1..=65535)`). We are `T: FromStr` and
      nothing else, so there is no per-field validation and no bounded numeric.
- [ ] **`allow_hyphen_values` on the derive path** — the spec says it and
      usage-lib honours it (`lib/src/parse.rs`), but there is no derive attribute
      and no mention in `argv/src/`. A spec-versus-derive asymmetry rather than a
      plain absence, which makes it cheaper than the rest of this group.
- [ ] **`require_equals`** — accept `--flag=value` and refuse `--flag value`.
- [ ] **`#[arg(skip)]`** — a field that is not an argument at all, filled from
      `Default`. Every field is currently a flag or a positional.

**Changes what a CLI accepts, less sharply**

- [ ] **`ignore_case` on values, and `#[value(alias = …)]`** — subcommand variants
      take aliases; enumerated _values_ take neither an alias nor a case-insensitive
      match.
- [ ] **`infer_subcommands` / `infer_long_args`** — unambiguous prefixes. We
      suggest what was probably meant but do not accept it.
- [ ] **`external_subcommand`** — the bridge reads clap's flag to compute
      `forwards` in `lib/src/spec/cmd.rs`, and the derive cannot declare one.
- [ ] **`multicall` and `no_binary_name`** — busybox-style applets, and parsing an
      argv that has no `argv[0]`.
- [ ] **`arg_required_else_help`, `args_conflicts_with_subcommands`,
      `subcommand_negates_reqs`, `allow_missing_positional`.**

**Help output**

- [ ] **Colour.** Errors are styled (`argv/src/diagnostic.rs`); help is not. clap
      colours headings and flag names by default and exposes `Command::styles`.
- [ ] **`term_width` / `max_term_width`** — we wrap to `COLUMNS` and take no
      instruction about it.
- [ ] **The granular hides** — `hide_default_value`, `hide_env`, `hide_env_values`,
      `hide_possible_values`, `hide_short_help`, `hide_long_help`. We have
      whole-entry `hide` and nothing finer.
- [ ] **`help_template`, `next_line_help`, `flatten_help`,
      `subcommand_help_heading`, `subcommand_value_name`.**
- [ ] **`verbatim_doc_comment`, `rename_all`, `rename_all_env`** — casing is kebab
      via `to_kebab`, with no opt-out.

**API surface**

- [ ] **`update_from` / `try_update_from`** — merge a parse into an existing struct.
- [ ] **The builder** — `Command::new`, `augment_args`, `CommandFactory`,
      `ArgMatches::get_one`, hand-written `FromArgMatches`. Architectural, and
      deliberate: usage-lib interprets a spec at run time and covers the dynamic
      case from the other side. Worth writing down as a decision rather than
      leaving it to be discovered as an absence.

**What is _not_ a gap**, checked rather than assumed, because two of these were
recorded as gaps here and had quietly been closed: flag aliases (several `long`
and `short` per field), subcommand aliases and hidden aliases, `--help`/`-h`, the
`help` subcommand, `--version`/`-V` with per-command propagation, self-contained
completions for four shells, `flatten`, `global`, `count`, `env`, `negate`
(clap's `SetFalse`), `value_enum`, `num_args` via `var_min`/`var_max`, clap's
`last` via `double_dash`, `help_heading`, `subcommand_required`, declaration
order, and non-UTF-8 `OsString`/`PathBuf` values.

And the other direction: `mount`, `restart_token`, `default_subcommand` and
`effect` are things a spec says that clap cannot hear, and `gen-shadow` counts
them.

`double_dash` is the one to state carefully, because two different claims about
it have been made in this file. The **bridge** carries three of the four modes:
`lib/src/spec/arg.rs` reads clap's `last` as `required` and `trailing_var_arg` as
`automatic`, and the default as `optional`. Only `preserve` — where the `--` is
itself a value — has no clap spelling. What drops the other two is **`gen-shadow`
writing a clap shadow**, whose `clap_double_dash` emits only `last`; that is a
gap in the shadow generator rather than in clap, since clap's derive does have
`trailing_var_arg`, and it is worth closing so the shadow stops overstating the
distance.

### The gate

Everything above is speculative until this passes. Baseline is mise's current
clap parser at mise's real scale, using a shadow CLI generated from mise's
checked-in `mise.usage.kdl` for both parsers.

- [x] **Shadow generation** — `xtask gen-shadow` turns any `.usage.kdl` into a crate
      of derived types. mise's committed 5,592-line spec compiles: 211 commands, 711
      flags, 128 arguments, four levels deep, in 2.6s. What it cannot express, it
      counts: 13 secondary flag aliases, 3 `double_dash="automatic"`, 1 default on a
      collecting flag. The clap dialect additionally drops the 2 mounts, 2 restart tokens
      and 1 `default_subcommand`, which the derive now declares and clap has no vocabulary
      for — so the two shadows no longer drop quite the same set, and the report says which
      side lost what.
      The generated crates are ordinary workspace members: their command enums are boxed,
      as the real mise boxes its own, so `large_enum_variant` has nothing to say and no
      lint is silenced anywhere.
- [x] **Bench harness** — `xtask gen-shadow … clap` writes the same CLI in clap's
      vocabulary, from the same spec and the same traversal, so the comparison is
      between parsers rather than between two transcriptions. Both shadows drop the
      same properties. Three release binaries — one per parser, one that does
      everything except parse — measured by `tak`, which gates the counts in CI.
- [x] **Perf report** — at mise's full scale, a cold parse costs **50.9k instructions
      against clap's 5.96M: 117× fewer**, and **2.1µs against 490µs of wall clock**.
      Measured by differencing two runs of the _same_ binary over how many parses it
      does, so nothing but the parse varies. clap's 490µs is 305µs building its command
      tree, ~160µs validating it, and ~24µs actually parsing — so even against clap's
      parse alone, with the tree already built and paid for, this is 12× faster.
- [ ] **Differential fuzzing** — proptest over argv against usage-lib on the mise
      spec, to find disagreements the corpus did not think of.
- [ ] **Perf report** — published honestly, whichever way it goes.

Runtime targets, which gate:

| measurement                        | clap, measured | target | result            |
| ---------------------------------- | -------------- | ------ | ----------------- |
| instructions, route + parse        | 5.96M          | < 100k | 50.9k, 117×       |
| wall time, argv to parsed struct   | 490µs          | < 50µs | 2.1µs, 238×       |
| heap allocations, successful parse | 6,560          | 0      | 0 bare, 3–4 bound |

All three gating targets are met, and not narrowly. Allocations were the last one owed:
a parse with nothing to bind allocates **nothing at all** at mise's scale — 211 commands
and 711 flags, and the allocator is never reached — while binding three or four words
costs three or four allocations, one per value. clap's tree costs **6,560** every time,
so this is about 2,000× fewer.

Getting there needed one fix and one correction. Defaults were being applied in `start`,
which builds the partial for _every_ command in the CLI rather than the selected one, so a
bare `mise` was allocating 60 times for defaults it would never read; they now run in
`check`, guarded on whether the flag was given. And the counter itself was wrong — armed
per thread but counting into a global — so parallel tests were counting each other and a
4-allocation parse read as 24, intermittently. usage-argv's own counter had the same
latent flaw and now counts per thread too.

An earlier draft of these numbers said 48–58× rather than 117×, because it subtracted a
baseline measured in a _separate_ no-op binary. Two binaries do measurably different
amounts of setup before `main`, and that difference had been landing in what was
attributed to parsing. Differencing two runs of one binary over how many parses it does
holds everything else fixed.

Secondary, measured and reported but not gated: compile time (full and
incremental) and binary-size contribution against an equivalent clap-derived
shadow.

If the runtime targets miss by a wide margin, the honest outcome is to write that
down and stop. Nothing gets integrated into mise before this point.

### After the gate

- [ ] **mise** — the largest and least forgiving adopter. Likely a router first, then
      commands lowered a few at a time, with mise's e2e argv corpus replayed against both
      parsers. Adoption is measured by what it lets mise delete, listed below.
- [ ] **hk, pitchfork, fnox** — smaller, and all three already generate their
      spec from clap, so they are the natural second adopters.
- [ ] **Other languages** — the grammar and corpus are language-neutral on
      purpose. A Go, JavaScript, or Python implementation is verified by running
      the corpus, not by reading this repository's Rust.

### What adoption should let mise delete

Checked against mise rather than assumed, and two of them do not survive contact.

- `GLOBAL_FLAGS_WITH_VALUES` and `first_non_global_arg_idx` (`src/cli/mod.rs`) — a
  hand-maintained copy of the root's value-taking flags, plus a test asserting it still
  matches clap. Its own comment says why it exists: `env.rs` needs it from `Lazy` statics at
  startup, and deriving it means building clap's tree, "which costs ~3.1M instructions… what
  made every mise command ~6.3M instructions more expensive". With `&'static` tables there is
  no tree to build, so that code can read the real thing. **This is the one that should
  disappear outright, list and guard test together.**
- `src/cli/usage.rs` — post-processes the emitted spec: clears `run`'s arguments, adds a
  mount and a restart token. The derive now declares `mount`, `restart_token` and
  `default_subcommand`, so those three patches become attributes on the commands that own
  them and the file should end up near empty — what remains is clearing `run`'s arguments,
  which is a consequence of the mount rather than a separate hack. It already records one
  hack that went away when jdx/usage#738 landed.
- `src/cli/command_effects.rs` — 451 lines classifying each command as read, write or
  destructive, "because mise's usage spec is derived from clap, and clap has no way to
  express this". The derive can express `effect` inline, so the _workaround_ reason goes —
  but the file also argues that a safety classification is easier to review as one list than
  as annotations over sixty files, and that argument survives any framework. Offer the
  annotation; do not assume the table should go.
- `Run(Box<run::Run>)` — boxed to stay out of trouble with clap at that size. The clap reason
  goes; the stack-size reason is real, and boxing stays supported.
- `src/assets/mise-extra.usage.kdl` — **not** a clap workaround. It is mostly a
  `source_code_link_template` for the docs, which a spec is the right home for.

## Not covered by the corpus yet

- [ ] **Restart tokens** — `restart_token` (mise's `:::`) makes one command line
      describe several invocations. `expect` holds a single result, so this needs a
      multi-invocation vector shape first.
- [ ] **Mounts** — `mount` resolves a sub-spec by running a command mid-parse,
      which makes a vector depend on an external process. Needs stubbing.
- [ ] **Completion parsing** — `parse_partial` accepts deliberately incomplete
      input. Different contract, different expectations, its own corpus.

## Known usage-lib divergences

**The corpus records none today**: usage-lib answers all 153 vectors, and so do
usage-argv and the Go runner. What is left below is the history, plus the two
items marked _needs a decision_ — which are not divergences but questions about
what the grammar should say.

They were bugs to fix or decisions to revisit, not settled behavior. Each was a
small change to `lib/src/parse.rs`, and the corpus is how a fix got verified —
including telling you to delete the label afterwards.

- [x] Help printed everything marked `hide` — hidden flags, hidden arguments, hidden
      subcommands. The usage _line_ filtered them already, through `SpecCommand::usage`, so
      `ex --help` listed a `--secret` that the line above it did not mention; markdown and
      manpage rendering filtered too. The help templates were the one place that did not.
      Found while building usage-argv's renderer, which would otherwise have had to reproduce
      it for parity.
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
- [x] A flag with a variadic argument rejects its second value, though
      [the flag reference](https://usage.jdx.dev/spec/reference/flag) documents the
      form. It collects now — until a token is flag-like, a `--` arrives, `var_max` is
      reached, or the line ends — which also made that bound reachable, and the attached
      form (`--include=a b`) collects with it.
- [x] `double_dash="automatic"` is not enforced, which
      [the arg reference](https://usage.jdx.dev/spec/reference/arg) says outright. The
      arg's first value now stops flag interpretation, and that note is gone from the
      reference.
- [x] An attached value was read a second time as a token, so `--jobs=--force` bound
      `force` and left `jobs` unset. The `=` has already settled that the text is a
      value, so it binds where it is read instead of going back on the queue.
- [x] A flag left waiting when the separator was consumed took the word after it, so
      `ex --jobs -- x` quietly meant `ex --jobs=x` with the `--` gone. Such a flag is
      starved — its value could only come from after the `--`, where every token is
      data — and is reported as the missing value it is. Found by importing clap's
      `double_hyphen_as_value`.

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
