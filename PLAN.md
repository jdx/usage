# Plan: a compiled parser, and a config layer, for usage

Working plan for two related pieces of work. Boxes get ticked as things land, so
this file is also the status: if a box is unchecked, that thing does not exist.
A `[~]` box is partly done, and the item says which part — the marker exists
because two items were reading as "not started" while most of their substance was
already written, which is the failure mode a status file has to avoid.

1. **A compiled argv parser** — a Rust parser that reads static tables instead of
   building a command tree, with the usage spec as its source of truth.
2. **A config layer** — one implementation of the settings model that mise, hk,
   pitchfork, and fnox have each rebuilt separately, declared once and lowered
   into the spec.

The first is underway. The second is **underway too**, which this file said it was
not: `usage-config` and `usage-config-build` are 8,220 lines, the spec's `config`
block has a model behind it, and the derive lowers `#[usage(setting = …)]` into
`SETTINGS_BINDINGS` with a `Registry::drift` check over it. What has _not_
happened is adoption — none of mise, hk, pitchfork or fnox depends on the crate —
and the boxes below are written per-item so the two are no longer conflated.

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
- [x] **`usage-derive` v1** — everything mise needs. `conflicts`, `overrides`,
      `required_if`, `required_unless`, `var`, `count`, `env`, defaults, the four
      `double_dash` modes, global flags, flatten, boxed subcommand variants,
      headings and `cfg`-gated variants have all landed since this was written.
      **`requires` has since landed** — spec, usage-lib's parser, the derive and
      the argv metadata all carry it, and `#[usage(requires = "--format")]`
      reports `MissingRequired` when the other flag is absent. The clap _bridge_
      still cannot read one, and cannot be made to: clap 4.6 exposes
      `Arg::requires` as a setter with no getter, so a `Command` cannot be asked
      what it requires. That is a clap limitation, recorded in
      `lib/src/spec/flag.rs`, not an item to close here.
      **`requires_if` / `requires_ifs` have since landed too**: the spec records
      repeated value/selector pairs, usage-lib and the derive enforce the same
      explicit-value rule clap does, and the cold tables emit the relationship
      without touching binding. Delimiters landed alongside them, so the original
      v1 list is closed.
      What a _rewrite_ of mise still trips over is in the clap-parity list and in
      **Trying the fleet** below.
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
- [x] **A mount on the root command** — top-level discovery is represented and
      usage-lib keeps completion and execution consistent about when the mount runs.
- [x] **`subcommand_required`, which the derive knew and did not say** — a bare `T`
      subcommand field requires a subcommand and an `Option<T>` does not, and the parser
      has always refused the invocation accordingly. `Spec::to_kdl` wrote neither, so the
      emitted KDL described a group command as one a user could run alone — and help, docs,
      completions and the SDK generators all read that rather than the type. Cold metadata,
      since it is not how a word binds. Found by converting usage-cli itself to the derive
      and diffing the spec it prints against the clap bridge's. One shape could have made the
      answer a lie — a `#[usage(flatten)]` group declaring subcommands of its own, which
      flatten leaves behind while the group's `build` still demands one — and is a compile
      error now, asserted during const evaluation in the parent's expansion, where the group
      is only a type.
- [x] **`unknown_flags`, which reached one command out of a tree** — usage-lib resolves it by
      walking outward from the command that ran, so a root declaring `error` makes the whole
      CLI strict. usage-argv held the effective value per command instead, on the theory that
      whoever built the tables would resolve it — which a derive cannot, since it expands one
      struct at a time and cannot see the command above. So the attribute reached the root
      alone, and on an `Args` it parsed and was then ignored: a declaration that compiled and
      did nothing. Now `None` means inherit, the parser carries the effective value down as it
      descends, and the corpus's own table builder stops resolving it — one implementation of
      the rule instead of two, and it was the second one that hid the parser not having it.
      Costs **160 instructions per parse, 72,272 against 72,112** at mise's scale.
- [x] **`subcommand_required` on the root command** — a bare root subcommand field
      now reaches the spec and both parsers report the missing command consistently.

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
- [x] **Docs and manpages** — the emitted KDL feeds `usage g markdown|manpage`
      the same way a clap-derived spec does. usage-cli already regenerates
      `docs/cli/reference` and `cli/assets/usage.1` from the derive's spec
      (`mise run render:usage-cli-completions`). The gate now asks the same of
      a clap CLI: `benches/gate/tests/fleet.rs` holds communique's markdown
      and manpage to the checked-in spec.
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
same shape as the `conflicts` hole above, found the same way. The ones the
fleet actually uses are listed under **Trying the fleet**; the rest stay here
as a clap-surface audit rather than a rewrite blocker.

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

- [x] **`requires` / `requires_if` / `requires_ifs`** — "this flag needs that
      one". Plain and value-conditional forms now reach the spec, usage-lib and
      generated checks. The bridge still cannot carry them, per the note above,
      so this remains a reason to declare in usage rather than a bridge bug.
- [x] **`ArgGroup`, and `exclusive`** — spec, derive (`group("input", required)`
      plus `#[usage(group = "input")]` / `exclusive`), usage-lib, and the
      bridge. None of the fleet specs currently _use_ a `group` node: clap's
      auto-groups for each struct are filtered out, and the live CLIs express
      mutual exclusion as `conflicts_with` or as help text that says "mutually
      exclusive".
- [x] **`value_delimiter`** — `--tags a,b,c` as three values. `lib/src/spec/arg.rs`
      and both Rust parsers now split typed, environment and default values before
      checking or converting them. The clap bridge keeps `delimiter` only for
      ASCII; a non-ASCII delimiter still splits/`var` in clap but is dropped here,
      same as clap's own restriction on `Arg::value_delimiter`. **The fleet
      fixtures do not yet carry it**: they were generated by a clap_usage that
      recorded a split default and dropped the delimiter. Regenerating them
      against this crate is the item under **Trying the fleet**, not another
      parser feature.
- [x] **`default_missing_value`, and optional-value flags** — `--color` versus
      `--color=always`. Spec `default_missing="always"`, usage-lib, usage-argv
      (`Flag::default_missing`), and `#[usage(default_missing = "always")]`.
      Absent stays absent (or takes `default`); `--color` binds the missing
      value; `--color=never` binds `never`. Combined with `require_equals`, a
      following word is still refused. `value_optional` remains help-only unless
      `default_missing` is also set, which makes help show the value as optional.
      clap 4 has the setter and no getter, so the bridge cannot read it — same
      hole as `requires`. **Used by the fleet:** mise `watch`, `generate bootstrap`,
      hk `-W/--fail-fast`, aube `--color` / `--inspect` / audit `--omit` — once
      those CLIs declare in usage rather than through clap.
- [x] **`default_if` (clap's `default_value_if` / `default_value_ifs`)** — a
      default that depends on another flag. Spec `default_if` on the target,
      usage-lib, usage-argv, `#[usage(default_if("--json", "true"))]`, and Go
      `ApplyDefaultIf`. Two arguments are `ArgPredicate::IsPresent`; three are
      `Equals`. First matching condition wins, and only when the target was not
      on the command line and has no env value. clap 4 has the setter and no
      getter, so the bridge cannot read it — same hole as `requires`. **Used
      by:** mise `bin_paths` (`default_value_if("json", IsPresent, "true")`).
- [x] **Portable value validation** — `validate="int(value) >= 1 && int(value) <=
65535"` is a declarative expr rule stored in KDL and enforced by usage-lib and
      generated Rust and Go parsers. `validate_error` supplies the user-facing failure.
      This covers clap's common range-validation use case without embedding a Rust
      parser function in the spec. clap's arbitrary `value_parser` remains inherently
      opaque to `clap_usage`, so an existing clap command must declare the equivalent
      rule when moving to the typed usage rewrite.
- [ ] **Token-boundary controls** — `allow_negative_numbers`, `value_terminator`
      and `dont_delimit_trailing_values`. `allow_hyphen_values` is the broader
      answer to the first one, but accepting every dash-word is not equivalent to
      accepting only a negative number; the other two have no spelling at all.
- [ ] **Fixed arity and distinct value names** — clap can say
      `num_args(2)` with `<START> <END>`. `var_min` / `var_max` express the bound,
      and the clap bridge now preserves it for positionals and non-repeatable value
      flags. Optional-value ranges beginning at zero and repeatable `Append` ranges
      remain bridge-lossy because their per-occurrence semantics cannot be represented
      as one accumulated bound. The derive also has one display name for the collection,
      so distinct `<START> <END>` labels are still absent.
- [x] **`allow_hyphen_values` on the derive path** — the spec said it and
      usage-lib honoured it (`lib/src/parse.rs`); usage-argv now has the same
      bit on `Flag`, so a detached value that looks like a flag binds when
      declared, including `--`. `#[usage(allow_hyphen_values)]` is the attribute,
      and the emitted KDL is `allow_hyphen_values=#true`. A positional that needs
      the same thing is already `double_dash = "automatic"`.
      **For the fleet this is mostly already spelled:** clap_usage encodes
      `allow_hyphen_values` on a trailing argument as `double_dash=automatic`,
      which the derive has. That is mise `run`/`exec`/`watch`/`asdf`, aube
      `run`/`exec`/`dlx`/`node`, fnox `exec`/`proxy`, pitchfork `daemons add`.
      A hyphen-taking _flag value_ that is not also trailing is now the same
      declaration on the flag.
- [x] **`require_equals`** — accept `--flag=value` and refuse `--flag value`.
      Spec, usage-lib, usage-argv, the derive (`#[usage(require_equals)]`), and
      the clap bridge (`Arg::is_require_equals_set`). A short's attached form
      (`-i9229`, `-i=9229`) still binds. **Used by:** aube `run --inspect` /
      `--inspect-brk`.
- [x] **`#[arg(skip)]`** — a field that is not an argument at all, filled from
      `Default`. `#[usage(skip)]` is that: the field stays on the struct so a
      rewrite can keep computed state beside parsed state, and nothing about it
      reaches the spec, the parse tables, or help. Combining it with `long` or
      `arg` is a compile error. **Used by:** mise `run`/`install`/`doctor`/
      `bootstrap`, hk `hook_options`, aube `update`.

**Changes what a CLI accepts, less sharply**

- [ ] **The full `PossibleValue` model** — `ignore_case`, aliases and their
      visibility, per-value help, and hidden values. Subcommand variants take
      aliases. The KDL model, usage-lib parser, and clap bridge now preserve this
      metadata. Generated Go tables distinguish accepted aliases and hidden values from
      the visible diagnostic/help list and honor `ignore_case`; Rust `ValueEnum` accepts
      aliases and case-insensitive values. Carrying per-value help and visibility through
      Rust's static metadata is the remaining work before this item is complete.
- [ ] **`infer_subcommands` / `infer_long_args`** — unambiguous prefixes. We
      suggest what was probably meant but do not accept it.
- [x] **`external_subcommand`** — an unmatched word is forwarded with the rest
      of argv. Spec `external_subcommand`, usage-lib, usage-argv, the derive
      (`#[usage(external_subcommand)]` on a catch-all `Vec` variant), and the
      clap bridge (`Command::is_allow_external_subcommands_set`). Known
      subcommands still win; a `default_subcommand` still catches first; a
      flag-like token on the parent is still an unknown flag. **Used by:**
      aube's catch-all and pitchfork's root.
- [ ] **Positionals in relationships and groups.** clap names relationships by
      argument id and allows positional members; the spec's selectors name only
      flags, so the clap bridge deliberately drops positional conflicts and group
      members. Required groups and conflicts can therefore become weaker.
- [ ] **The complete relationship families** — `requires_all`,
      `required_if_eq`, `required_if_eq_all`, `required_if_eq_any`, and the
      `required_unless_present` all/any variants. The common single-selector
      forms exist, but a clap migration needs the complete truth table or an
      explicit non-goal for each omitted form.
- [x] **`multicall`** — busybox-style applets: argv[0]'s basename selects a
      subcommand when it is not the dispatcher. Spec `multicall #true`,
      usage-lib (which sees argv0), usage-argv / the derive `parse()` /
      Go `RewriteMulticall` (which rewrite at process entry; `parse_from` /
      `Parse` stay without argv0), and the clap bridge
      (`Command::is_multicall_set`). Path components and a trailing `.exe`
      are stripped. **Not used by the fleet today**; clap's own applets and
      busybox-style binaries are the reason it exists.
- [ ] **`no_binary_name`** — parsing an argv that has no `argv[0]`. usage-argv
      already takes argv without the program name; this is clap's setter that
      skips stripping it. Out of scope until a fleet CLI needs it.
- [ ] **Command parsing policy** — `arg_required_else_help`,
      `args_conflicts_with_subcommands`, `subcommand_negates_reqs`,
      `subcommand_precedence_over_arg`, `allow_missing_positional`,
      and `args_override_self`. One design decision is made ahead of the
      implementation: `arg_required_else_help` reads argv, not bound values.
      clap counts an environment-supplied value as present (clap#3572), so a
      user with a configured environment never sees the help — with MISE_*
      set, that is most mise users. An empty command line means help,
      whatever the environment holds.

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
- [ ] **Built-in action and flag control** — custom `ArgAction::Help`,
      `HelpShort`, `HelpLong` and `Version`, plus `disable_help_flag`,
      `disable_help_subcommand` and `disable_version_flag`. A clap
      CLI can move or remove these entry points; usage currently supplies its own.
- [ ] **Ordering and remaining metadata** — explicit `display_order`, the full
      `ValueHint` vocabulary, `author`, `long_version`, and custom help/version
      text. Declaration order and path hints cover the common cases, not the
      complete clap surface.

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
order, non-UTF-8 `OsString`/`PathBuf` values, `requires`, `group`/`exclusive`,
and `delimiter`, `#[usage(skip)]`, `allow_hyphen_values`, `require_equals`, and
`default_missing`.

And the other direction: `mount`, `restart_token`, `default_subcommand` and
`effect` are things a spec says that clap cannot hear, and `gen-shadow` counts
them. `requires`, `group`/`exclusive`, and `delimiter` are now things a spec
says that clap can only half-hear: the first has no getter, the other two the
bridge reads.

`double_dash` is the one to state carefully, because two different claims about
it have been made in this file. The **bridge** carries three of the four modes:
`lib/src/spec/arg.rs` reads clap's `last` as `required` and `trailing_var_arg` as
`automatic`, and the default as `optional`. Only `preserve` — where the `--` is
itself a value — has no clap spelling. What drops the other two is **`gen-shadow`
writing a clap shadow**, whose `clap_double_dash` emits only `last`; that is a
gap in the shadow generator rather than in clap, since clap's derive does have
`trailing_var_arg`, and it is worth closing so the shadow stops overstating the
distance.

### General clap launch gate

The fleet proves that this can replace clap for the CLIs in front of us. A public
launch makes a broader promise: a clap user must be able to tell, before changing
their parser, whether every behavior they rely on survives. clap's derive forwards
arbitrary `Command`, `Arg` and `PossibleValue` builder methods, so a hand-selected
feature list is not an exhaustive audit.

- [ ] **A versioned clap compatibility matrix.** Inventory clap's public derive
      attributes and relevant builder methods, pinned to the clap release audited.
      Give every row a result for `usage-derive`, `usage-argv`, the KDL spec and
      usage-lib, help/completions, and the `clap_usage` bridge. Each cell must say
      supported and tested, usage-only, bridge-lossy, intentionally different, or
      unsupported. Updating clap must update the matrix in the same PR.
- [ ] **A clap-to-spec fidelity report.** The bridge currently loses detectable
      metadata without reporting it: `num_args` bounds, environment bindings,
      hidden flag aliases, value hints and `PossibleValue` metadata, positional
      relationship/group members, and command parsing settings. Return or emit a
      structured loss report for everything clap exposes. For setter-only state
      such as `requires` and `default_missing_value`, where loss cannot be detected,
      the matrix and integration docs must say that the bridge is not a lossless
      migration verifier.
- [ ] **Defaults must preserve optionality in metadata.** The typed tak rewrite
      accepts an omitted defaulted `String`, but its emitted KDL spells the
      argument `<REV>` and the flag `required=#true`, where clap_usage emitted
      `[REV]` and an optional value-taking flag. Binding succeeds because the
      derive applies the default, but help, spec consumers and differential
      tooling see a required shape. A default satisfies a field; it must not
      make the user's token required in the static metadata.
- [x] **Rust expressions for compile-time metadata.** hk and fnox use constants
      for defaults, aube and hk use constants for long help, and all three use
      generated or computed version strings. Requiring every value to be copied
      into a string literal creates exactly the drift this project is meant to
      remove. `version` and `default_value_t` now accept Rust expressions. A
      computed version pairs with `version_spec`, and a typed default pairs with
      `default`; the expression drives runtime behavior while the explicit literal
      keeps emitted KDL deterministic and portable.
- [ ] **One Args type used by more than one command.** tak's `push` and `init`
      have the same `--remote` shape. The derive rejects two variants wrapping
      one `Args` type because collection is attached to the declaring struct,
      forcing two identical structs. `flatten` solves shared fields within a
      command but not a whole command body shared under two names. Either support
      this directly or document the duplication as an architectural constraint.
- [x] **Inline struct-style subcommand variants.** aube and hk use clap enums
      whose variants declare fields directly. usage requires every non-bare
      variant to wrap one dedicated Args struct, turning a mechanical migration
      into a broad public-type refactor before argv behavior can even be tested.
      The Subcommands derive now lowers inline variants to hidden Args structs and
      moves the bound fields back into the original enum shape. It accepts both
      native `usage` field attributes and clap-shaped `arg` attributes in this
      migration form.
- [x] **Clap-compatible value metadata spelling.** Existing domain enums commonly
      use `#[value(name = "...", alias = "...")]`. Requiring those attributes to
      be renamed solely to change parsers makes migrations noisier and prevents a
      transition where clap and usage derive against the same enum. `ValueEnum`
      now accepts both `#[usage(...)]` and `#[value(...)]`, while continuing to
      leave `FromStr` ownership with the domain type.
- [x] **ValueEnum must coexist with domain parsing and cfg.** aube and fnox enums
      already implement `FromStr`; deriving usage `ValueEnum` adds a conflicting
      implementation. fnox also cfg-gates individual variants, while usage's
      const word list refused holes. ValueEnum now describes choices without
      taking ownership of domain parsing, and copies variant `cfg`/`cfg_attr`
      attributes onto the corresponding static-table entries.
- [ ] **Clap-compatible field spellings and IDs.** Multiple `long` and `short`
      entries express flag aliases in usage, but real migrations still have to
      rewrite clap's `alias` / `visible_alias`, `id`, `num_args`, `value_parser`
      and `rename_all` vocabulary before the derive can explain the semantic
      replacement. Accept the lossless spellings directly where practical and
      give the rest targeted migration diagnostics rather than a generic unknown
      option error. Preserve the visibility distinction too: clap's `alias` and
      `aliases` are hidden while `visible_alias` and `visible_aliases` are
      advertised; usage spells those `alias_hidden` and `alias`. The fnox rewrite
      initially made `completion`'s hidden aliases and `exec run` visible because
      a mechanical rename erased that distinction.
- [ ] **Command-with-arguments completion hints.** fnox uses
      `ValueHint::CommandWithArguments` for forwarded argv. usage accepts only
      file, path and directory hints today. Add the command/argv cases or record
      them as an explicit completion non-goal before claiming clap coverage.
- [ ] **Version omission and dynamic version policy.** tak intentionally removes
      the package version from its generated spec so release-plz version-only PRs
      do not dirty generated docs; hk computes a richer version string. Specify
      static, expression-backed and omitted versions separately rather than
      forcing a hard-coded literal into every `Cli` derive.
- [ ] **Unit and tuple Args migration shapes.** fnox's bare command structs and
      hk's one-field tuple Args are valid clap derive inputs. usage currently
      requires named-field braces, so even a command with no arguments changes
      from `Command;` to `Command {}` and a tuple wrapper needs a public shape
      change. Support unit structs directly; either support tuple Args or emit a
      diagnostic that identifies the named-field rewrite.
- [ ] **Relationships through flatten and positional IDs.** Positional
      relationships are already a general gap above. The fleet exposed the
      second half: a field cannot name a flag contributed by a flattened Args
      type because validation runs against the declaring struct before the
      command is assembled. aube lost statically declared relationships and hk
      and fnox needed runtime conflict checks. Validate selectors against the
      composed command and carry stable IDs for both flags and positionals.
- [ ] **Flattened help topology.** clap's `next_help_heading` and flattened flag
      groups preserve meaningful sections in aube's long help. usage flattens the
      fields but discards that struct-level heading, so a migration can preserve
      parsing while silently degrading help. Define heading inheritance for a
      flattened Args type and cover short/long help ordering in conformance.
- [ ] **Facade-owned derive validation.** A direct `usage-derive` adopter can
      compile generated code only after separately adding `usage-validation`;
      the implementation dependency leaks into every converted manifest. The
      supported facade should own or re-export this path so the documented
      dependency set is sufficient and generated code does not require users to
      discover an internal crate from a compiler error.
- [ ] **Spec mutation without an MSRV jump.** aube and hk parse derived KDL into
      usage-lib solely to attach command effects and other generated fragments.
      That raises an argv-only adopter from the 1.91 tier to usage-lib's 1.95
      tier (and hk currently declares Rust 1.88). Provide a static metadata hook,
      a lightweight spec-editing surface, or an explicit release policy before
      claiming these migrations are mergeable.
- [ ] **Keep generated-spec producers and consumers on one dialect.** A derive
      pinned to the 6.x stack can emit nodes such as `unknown_flags` that the
      released 5.x `usage` binary in an adopter's docs task cannot read. The
      communique experiment had to build a second git-pinned `usage-cli` just to
      render the KDL emitted by its Rust dependency. Define a supported way to
      install matching pre-release tooling (and make version diagnostics name the
      required dialect) so a migration does not silently combine two revisions.
- [ ] **Runtime program identity.** aube embeds the same CLI under a caller-chosen
      binary name. A derived spec can be rewritten after emission, but parser
      help and diagnostics still use the static name. Support a runtime identity
      source with an explicit portable `name`/`bin` value, analogous to computed
      version plus `version_spec`.
- [ ] **Test parsing with argv0.** `parse_from` intentionally takes words after
      the binary, while clap tests commonly call `try_parse_from(["tool", ...])`.
      Fleet ports needed local wrappers just to preserve their parser tests.
      Add an explicit argv0-taking helper rather than making every migration
      hand-roll `skip(1)` and accidentally obscure multicall semantics.
- [ ] **Generated micro-conformance against clap.** One minimal CLI per matrix row,
      compared on accepted and rejected argv, typed values, error kind and exit
      status, stdout versus stderr, short and long help, usage/version output, and
      completion candidates. Run the portable cases on Unix and Windows and the
      byte-value cases on Unix. The mise fuzzer remains the scale test; this is the
      configuration-space test it cannot be.
- [ ] **Combination and stateful tests.** Pairwise-cover settings that interact:
      defaults with env and delimiters, optional values with `require_equals`,
      globals with overrides, subcommands with required positionals, groups with
      defaults, and help/version collisions. Add `update_from` cases if that API is
      implemented. Single-feature parity is not enough where clap resolves settings
      in an order.
- [ ] **External clap adopters.** Before calling the replacement generally ready,
      port or shadow at least three maintained, non-jdx clap CLIs chosen for
      different shapes: a derive-heavy CLI, a builder-heavy CLI, and one using
      custom validation/completions. Record every unsupported feature they expose;
      do not silently narrow the sample to what already works.
- [ ] **Migration and non-goal documentation.** Publish an attribute mapping guide,
      examples for the common `Parser` / `Args` / `Subcommand` / `ValueEnum`
      rewrites, compile-fail examples for unsupported combinations, the clap
      compatibility baseline, and the parser-behavior semver policy. State which
      builder and `ArgMatches` APIs are architectural non-goals instead of leaving
      their absence implicit.
- [ ] **A release documentation audit.** Generate the limitations page from, or
      check it against, the compatibility matrix; verify dependency snippets against
      workspace versions; and remove stale claims after features land. Today the
      Rust limitations page still says non-UTF-8 `OsString` values cannot be accepted,
      and the clap integration still recommends `clap_usage = "2"` while this
      workspace is on version 5.
- [ ] **Completion ecosystem coverage.** Decide whether general clap parity includes
      every shell in `clap_complete` and every `ValueHint`. At minimum, either add
      Elvish beside bash, fish, PowerShell and zsh or document it as a launch non-goal;
      keep Nushell as an explicit usage extension rather than accidentally counting
      it as clap parity.

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
      _Re-measured later, and it moved:_ **60.4k against 5.90M — 97×**. clap is flat;
      our parse grew about 19% as the derive gained vocabulary, so the headroom under
      the <100k target went from 2× to 1.65×. Still passing, and nothing watches it:
      the shadow comparison is reported and never gated, on the grounds that the
      fixture grows on purpose. That reasoning holds for the absolute number and not
      for the _ratio_, which is a property of the two parsers. Worth a gate before the
      margin erodes further.
      Measured by differencing two runs of the _same_ binary over how many parses it
      does, so nothing but the parse varies. clap's 490µs is 305µs building its command
      tree, ~160µs validating it, and ~24µs actually parsing — so even against clap's
      parse alone, with the tree already built and paid for, this is 12× faster.
- [x] **Differential fuzzing** — proptest over argv on the mise spec, against
      usage-lib **and clap**. Two parsers was the wrong design and the first run showed
      it: every disagreement had usage-argv stricter than usage-lib, which reads as a
      pile of usage-argv bugs until clap is asked the same question and sides with
      usage-argv. usage-lib is lax — it accepts a missing subcommand, a missing flag
      value, and a repeated non-repeatable flag. So clap is the standard for
      _accepting_, being what mise ships, while usage-lib stays the standard for
      rendering.
      It found one real usage-argv bug — a lone `-` selecting the root's default
      subcommand instead of binding as a value — now fixed.
      It also found that **a fuzzer over a real spec must strip mounts first**:
      usage-lib resolves `mount run="mise tasks --usage"` by _running_ it, so the first
      draft spawned real `mise` processes that loaded config, fetched vfox metadata and
      shelled out to `apt-cache`. See `benches/gate/tests/differential.rs`.
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

### Trying the fleet

The gate asked whether usage-argv can parse _mise's spec_. That is not the same
question as whether the jdx.dev CLIs can move onto `usage-rs`. The first is
answered: `xtask gen-shadow` on every checked-in fleet spec drops **nothing** in
the usage dialect, and `benches/gate/tests/fleet.rs` holds help to usage-lib
across all seven. The second is what is left, and it is a different fixture.

The seven are mise, hk, fnox, pitchfork, aube, tak, and communique. usage-cli
already ships on `usage-rs` — it is the first adopter, not a remaining one.

Shadows are generated from a `.usage.kdl`, and those files are themselves
generated from clap via `clap_usage`. Anything clap does not expose, or that an
older clap_usage dropped, is invisible to the gate. Trying the CLIs means
looking at the clap surface, not only at the spec.

- [ ] **Refresh the fleet fixtures against this crate's `clap_usage`.** The
      copies in `benches/fleet/` and `benches/mise.usage.kdl` are snapshots.
      Live `mise usage` (2026.8.8) and `aube usage` (1.20.0) still emit no
      `delimiter=` and no `allow_hyphen_values`, because they were built against
      a clap_usage that did not carry them. This crate now does. Until the
      fixtures are regenerated from the CLIs linked to _this_ `clap_usage`, the
      shadows cannot see delimiter-split flags that mise, hk, pitchfork and aube
      all declare. tak currently has no `usage` subcommand at all, so it cannot
      even produce a fresh spec without one. The experiment may add `usage` or
      `--usage-spec` solely to expose the new canonical metadata; that new entry
      point has no clap-era behavior to preserve and is excluded from tak's
      compatibility baseline.
- [x] **Docs and manpages on a fleet spec.** communique's checked-in spec and
      the KDL its shadow emits render the same markdown (index and every
      command) and the same manpage. usage-cli's own
      `render:usage-cli-completions` already does this for `usage`; the gate
      now asks it of a clap CLI.
- [x] **Typed rewrites of communique, tak, aube, hk, and fnox, not String
      shadows.** `gen-shadow`
      types every field as `String`. The derive already holds `PathBuf`,
      `OsString`, `ValueEnum`, `FromStr`, `flatten`, `Option`/`Vec`. usage-cli
      proves that for usage's own types. These five will prove it across real
      clap CLIs rewritten in place: real field types, skip-fields or the split
      they force, and binaries that preserve every pre-existing `--help` and
      spec-emission entry point. tak's experiment-only spec entry point is tested
      as new behavior rather than compared with a nonexistent baseline. Each
      experiment is a ready-for-review PR whose `Cargo.toml`
      deliberately points at usage's git revision; the PR is evidence for the
      6.x gate, not something to merge before usage 6.x is published. Together
      they tell us whether the fleet is a rewrite or a set of blocked rewrites.
      **The experiment PRs now exist and all five modify the real CLI:**
      jdx/communique#265, jdx/tak#47, jdx/aube#1336, jdx/hk#1211 and
      jdx/fnox#725 all remove clap, compile against the stacked usage changes and
      pass their migrated test suites. The ports preserve their typed domain
      values rather than lowering to String, keep intentional forwarding behavior,
      and opt strict CLIs into `unknown_flags="error"`; aube remains permissive at
      the root because its external-subcommand path is a package-manager forwarder.
      All five pin the experiment stack at `88786493`. The workarounds they still
      contain are the unchecked launch-gate rows above, not unfinished conversions.
- [x] **The clap-only validation behaviour the fleet actually uses.** Portable
      `validate` expressions cover numeric ranges in the typed rewrite. Arbitrary clap
      parser functions remain opaque to `clap_usage`, but they no longer require a
      Rust-only extension to the spec: the rewrite declares the equivalent expr rule.

`external_subcommand` and `default_if` have landed: the parser, the derive, and
the corpus all say them. clap's bridge reads `allow_external_subcommands`;
`default_value_if` is a setter with no getter, same hole as `requires`. They
are off this table because they no longer change what a rewrite accepts.

`value_delimiter`, `requires`, `allow_hyphen_values`, `require_equals`, and
`default_missing` are not in that table because the _parser_ can say them. They
are lost only on the clap → spec round trip (and a non-ASCII delimiter is
dropped even then; `default_missing_value` has no clap getter), and a rewrite
that declares in usage keeps them. `#[usage(skip)]` is a compile-time field, not
a command-line shape. Trailing argv is already `double_dash=automatic` in every
fleet spec that has one.

- [x] **The grammar decision that would change mise at run time**, not just at
      completion time. Unrecognized flags falling through to positionals is how
      mise parses task arguments; tightening it is a behaviour change to every
      `mise run`. **Decided: the default stays lax, everywhere, and strict is
      opt-in per spec** — recorded in the divergence list below — so a rewrite
      changes nothing about what a task accepts. Repeated `--` handling was
      settled and fixed in #809.

**Not on this list, on purpose.** Config is a second project: the four CLIs
would keep their generated `Settings` through an argv-only move. Mounts are
declared and emitted; usage-argv does not execute them, so `mise run`'s task
names still come from `src/cli/usage.rs` until a later stage. Root `mount` and
root `subcommand_required` are spec-shape questions none of the seven needs —
usage-cli's root requires a subcommand in the type, and the usage line already
says `<COMMAND>`. Publishing the perf report is documentation of the gate, not
a prerequisite for trying a CLI.

### After the gate

- [x] **usage-cli** — the first adopter, already shipping. It parses with
      `usage-rs`, emits its spec from the same tables, and feeds that spec to
      the markdown, manpage and completion generators. The remaining items in
      **Trying the fleet** are about the _other_ CLIs, not this one.
- [x] **communique, tak, aube, hk, and fnox** — five ready-for-review fleet
      experiment PRs parse their real typed commands with usage, remove clap and
      pass locally. They deliberately retain a git dependency and are evidence for
      the 6.x gate rather than merge candidates before publication. tak's added
      spec endpoint is experiment-only and outside its preserved CLI contract.
      The gaps found are recorded in the general launch gate above; closing the
      merge-blocking rows is required before publishing 6.x and converting these
      experiments into release-dependency PRs.
- [ ] **mise** — the largest and least forgiving adopter. Likely a router first, then
      commands lowered a few at a time, with mise's e2e argv corpus replayed against both
      parsers. Adoption is measured by what it lets mise delete, listed below.
      Do not start this until the clap-only rows in **Trying the fleet** that
      mise actually uses are either implemented or accepted as lost.
- [ ] **pitchfork** — not part of the five typed experiments, but already
      generates its spec from clap. It is the next small adopter after those
      experiment branches become mergeable.
- [~] **Other languages** — Go now parses, validates, renders help and answers
  completions from generated static tables, verified against the shared corpus.
  JavaScript and Python implementations remain open.

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

### What adoption should let the rest of the fleet delete

A survey of the whole jdx.dev fleet (2026-08-19), done the same way as the mise
list above: checked against the source rather than assumed. Every CLI is on
clap 4.6 derive, every one already generates its spec through clap_usage, and
every one carries glue of the same three species — argv rewritten before clap
runs, metadata clap cannot express spliced into the generated spec afterwards,
and completions that depend on a separately-installed `usage` binary.

**mise**, beyond the delete-list above:

- `escape_task_args` / `unescape_task_args` (`src/cli/mod.rs:447-696`) — ~200
  lines plus eight tests that prefix task-side flags with `\x00MISE_TASK_ARG\x00`
  so clap will not bind them, then strip the prefix after the parse. It exists
  because mise runs two parsers per invocation — clap for the mise side, usage
  for the task side — and words must be smuggled across the boundary. A
  usage-native parse with `restart_token` (already declared) has no boundary to
  smuggle across.
- `preprocess_args_for_naked_run` (`src/cli/mod.rs:698-732`) — hand-scans argv
  to inject `"run"`, and must therefore re-know which global flags take values.
  Routing on `default_subcommand` (landed above) is the replacement.
- The hand scanner has a confirmed user-facing bug clap does not:
  `mise --env=production` is silently ignored while `mise --env production`
  works (jdx/mise discussion #8883) — the cost of a third partial flag parser.
  `hook_env.rs:202-222`, `activate.rs:209-239` and `version.rs:88-98` each
  carry another copy of the same knowledge.
- The deferred `bootstrap` subtree — a stub plus a hand-written `FromArgMatches`
  (`src/cli/bootstrap.rs:41-76`), purely to keep clap's tree-build off the hot
  path. Static tables have no tree to defer.
- `tool_stub.rs:659-678` ignores clap's parse and re-reads raw argv "to avoid
  version flag interception".
- `task/mod.rs:1870-1912` reconstructs the `--` separator clap consumed, by
  suffix-matching argv, because clap reports `last=true` values but not where
  the separator stood.
- `mcp.rs:127-131` re-implements hide-propagation ("clap does not propagate
  `hide` to children, so a visible child of a hidden parent is still not a
  documented path").
- `completion.rs:136-140` accepts `pwsh` in one command and `powershell` in
  another, because two clap `ValueEnum` lists were declared separately —
  whichever name a user learns first is rejected by the other.

**aube** — the heaviest workaround load after mise:

- `is_usage_invocation` in `main.rs` intercepts `aube usage` before clap runs;
  multicall dispatch for the `aubr`/`aubx` shims also happens pre-clap.
- `multicall_usage_spec()` (`commands/completion.rs`) — ~80 lines of spec
  surgery (shift_remove a subcommand, splice globals in, rewrite name/bin/usage
  strings, force `var_max` and `double_dash`) to fake standalone `aubr`/`aubx`
  specs out of the generated one.
- `command_effects.rs` — a 100+-entry hand table, same species as mise's.
- A manual `--version` flag (`lib.rs:87`) because clap's auto-version exits
  inside `parse_from`, before the tokio runtime that runs the async update
  notifier is built.
- `trailing_var_arg + allow_hyphen_values` on five forwarding commands;
  `num_args = 0..=1` + `require_equals` + `default_missing_value` for
  `--inspect[=HOST:PORT]`; `overrides_with` pairs for hand-rolled
  `--sort`/`--no-sort`; a hand-written `FromArgMatches` on `PruneArgs` with an
  `AtomicBool` side channel; and `npm_fallback.rs`'s hidden catch-all stub
  commands for npm-only verbs.

**hk**:

- `reexec_for_cd` (`src/cli/mod.rs:86`) hand-walks argv to strip `--cd` and
  re-exec in the target directory, with per-platform OsStr byte handling —
  clap gives no way to re-render parsed args back into argv.
- Hand-rolled `--fail-fast`/`--no-fail-fast` and `--stage`/`--no-stage` bool
  pairs with mutual `overrides_with`, plus repeated `conflicts_with_all`
  string arrays — the spec's `negate` is the declaration these want to be.
- `--why [STEP]` is `num_args = 0..=1, default_missing_value = ""`, an
  empty-string sentinel meaning "all steps".
- Completion invocations were loading full project config until special-cased
  (hk#615) — the cost of completions being ordinary subcommands of a heavy CLI.
- `command_effects.rs`, again.

**fnox**: dynamic completers via hidden `--complete` probe flags on three
commands plus a hand-written extras KDL; `min_usage_version "1.3"` hardcoded
while siblings emit `"4.0"` — sidecar drift in the flesh; `trailing_var_arg +
allow_hyphen_values + ValueHint::CommandWithArguments` on `exec` and `proxy`.

**pitchfork**: default-subcommand fallthrough via `external_subcommand` plus a
second `StartFallback` parser with `bin_name = "pitchfork start"`
(`src/cli/mod.rs:70-131`), and its own `command_effects.rs`.

**communique and tak**: no shell completions at all, despite both having specs —
the wiring cost is real enough that small CLIs skip it. tak also sets
`spec.version = None` post-hoc (`src/main.rs:817`) so release-plz's version-only
PR does not fail the generated-reference CI check; clap_usage has no knob for
it. communique keeps its spec honest with a unit test that tells you to
regenerate by hand.

The cross-cutting counts: command-effect sidecar tables in five repos, extras
KDL spliced onto the generated spec by string concatenation in four, the
external `usage` binary as a runtime completion dependency in three — the
most-reported public issue class (jdx/mise discussions #5659 and #5675,
nixpkgs#343832), and the reason mise now maintains a prerendered static
fallback pipeline — hand-rolled negation pairs in two, and spec-sync CI chores
in all of them.

### Gaps to close before dogfooding the fleet

The fleet's workarounds also route around usage, not only clap. These are the
items the survey adds; the ones already tracked as unchecked boxes under **What
clap can say that we cannot** (positionals in relationships, the complete
relationship families, command parsing policy including
`arg_required_else_help`, fixed arity, the full `ValueHint` vocabulary) are not
repeated here.

- [ ] **The completer channel is unescaped Tera into `sh -c`.** aube's
      `completion.usage.kdl` documents the quote-escaping gymnastics, and its
      `extra.usage.kdl` records a `-C` non-forwarding limitation outright:
      "usage's only channel for the typed words is tera interpolation into a
      `sh -c` string, with no shell-quoting filter". A quoting filter — or a
      structured argv channel — is owed before dynamic completers are a
      recommendation rather than a hazard.
- [ ] **A multicall CLI cannot describe its applets.** aube's 80 lines of spec
      surgery for `aubr`/`aubx` is the requirement written as a workaround: a
      spec (or the derive) should declare a sub-view — name, bin, a subset of
      commands, the global flags — without the host mutating a generated spec
      by hand. Related: help and diagnostics render the compiled-in name, so an
      embedder with a dynamic identity is stuck with the static one.
      **Direction decided (2026-08-19): a spec-first `view` node** on the root,
      carrying name, bin, the command subset, and which globals carry over,
      that help, completions and docs all read, with the derive lowering an
      attribute into it. Not a derive-only emission, and not a blessed
      transform API: the spec defines.
- [ ] **No home for post-parse hooks.** aube reimplements `--version` because
      the built-in exits before its async update notifier can run; hk strips
      `--cd` and re-execs by hand. `parse_from` returning `Error::Help` and
      version instead of exiting is the derive's answer in principle — confirm
      the pattern covers both cases and document it, or add the hook.
- [ ] **MSRV.** usage-lib is on Rust 1.95 while the fleet floors are 1.88 and
      1.91, so a CLI that links it for spec generation inherits the bump. The
      argv/derive tier is dependency-free by design; keeping a low-MSRV path to
      spec emission — without usage-lib — is what lets a conservative CLI adopt.
- [ ] **The `parse_from` argv0 contract.** It differs from clap's, which broke
      fnox's test helpers in the rewrite experiment. Decide it, document it,
      and provide a clap-shaped variant if the difference stays.
- [ ] **Shared `Args` under multiple commands need wrapper types** in the
      derive, where clap lets one struct serve several parents directly.
      flatten covers the mise `ConfigLs` shape; the fnox shape is the same
      struct as a full command body in two places.
- [ ] **Relationships across a flatten boundary.** A flag in the parent
      conflicting with a flag in the flattened group has no spelling; hk and
      aube both hit it and enforce post-bind by hand.
- [ ] **Checked-in specs vs release automation.** tak sets `spec.version = None`
      post-hoc so release-plz's version-only PR does not fail the
      generated-reference CI check. This is not a bridge problem and it survives
      removing clap: the derive's spec emission carries `version` too. Decide
      how a checked-in spec treats version under release automation — an
      omit-version option on spec emission, or regenerating the reference as
      part of the release. Not a clap_usage feature; the bridge is transitional
      and the fleet's endpoint is the derive. **Decided (2026-08-19): the
      omit-version option.** The binary knows its version at runtime; docs and
      manpage rendering inject it at render time; release PRs stay
      version-only.

### clap's backlog, read as a roadmap

A pass over clap's most-upvoted open issues (2026-08-19), asking two questions:
what do clap users want that usage already has, and what demand should shape
what gets built next.

**Already answered here — promotion material for the docs, later.** A striking
share of clap's top-voted open requests is usage's existing feature set:

- Dynamic completions (clap#3166, 102 votes, plus clap#1232's 157 before it was
  folded in) — clap's native completion engine has been unstable for 4+ years;
  runtime completion served from the spec is usage's core architecture. Nushell
  (clap#5840) comes with it, which clap_complete lacks.
- Automatic negation flags (clap#815, 66) — `negate`.
- Argument validation on globals (clap#1546, 48) — globals go through the same
  post-binding checks as everything else.
- Partial parsing that captures unknown args instead of erroring (clap#1404) —
  the spec's lax `unknown_flags` mode, which mise task parsing runs on.
- Command chaining (clap#2222, 29) — `restart_token`.
- Manpage customization (clap#3354) — generated from Tera templates rather than
  a fixed renderer.
- `args_override_self` as the default (clap#4261) — the grammar's "a repeat is
  a correction" rule.
- Default subcommands (clap#3857 and clap#4442, both closed "not planned",
  with the discussion still active in 2026) — `default_subcommand`, declared,
  routed, and completing.
- GNU-correct optional option-arguments (clap#3030, where fixing the default
  "likely isn't" possible without breaking rustc and cargo) —
  `default_missing`, under which `--color bar` binds the missing value and
  leaves `bar` a positional, with `require_equals` beside it.
- Help order following declaration order (clap#1807, 25 comments of stalled
  design) — a spec is ordered, so help order is spec order, with
  `help_heading` on top.
- A machine-readable export of the whole CLI (clap#918, open since 2017, and
  discussion clap#6491 asking for schemas AI agents can read) — the spec _is_
  the export, `effect` is the safety vocabulary an agent wants, and usage-cli
  renders JSON already. clap#6026 was closed with advice to "implement your
  own argument parser" — which is this project.

When the migration guide is written, a "top clap feature requests that just
work here" section is cheap and persuasive; the launch-gate documentation items
above are where it lands.

**Worth building, demand attached:**

- [ ] **Subcommand help headings** (clap#1553, 38 votes) — `help_heading`
      landed for flags and arguments; this is the same property on `cmd` nodes,
      so a 210-command CLI can group its help into sections. mise is the
      obvious first user. clap#4589 asks for prose under a heading — Deno wants
      per-section doc links — which is the same node with one more field.
- [ ] **Deprecation and stability metadata on flags and commands** (clap#3321) —
      `deprecated`, with warn/remove versions, already exists in the
      config-prop vocabulary; the same on flags and commands would flow into
      help, docs and completions from one declaration. Explicit full-name
      deprecated env aliases (clap#5447) fit the same slot — full names, so
      they stay greppable — and clap#5925's ordered fallback across several
      env names is the same declaration read in order.
- [ ] **A group as an enum in the derive** (clap#2621, 102 votes — tied for
      clap's most-requested) — mutually exclusive flags declared as enum
      variants, lowering to the `group`/`conflicts` vocabulary the spec already
      has. Derive ergonomics rather than new spec surface, and clap has sat on
      it since 2021.
- [ ] **Alias into a nested subcommand** (clap#1603, reopened, 22 comments) —
      rustup's `install` meaning `toolchain install`, args carried along. A
      spec-level redirect an interpreter applies before parsing, so help and
      completions describe the alias too; clap's unstable `App::replace`
      answer died for lack of interest.
- [ ] **Recursive help** (clap#4813) — help for a whole command tree in one
      output. The markdown generator already walks the tree; a `--help-all`
      output mode is cheap, and doubles as the golden file the help-parity
      tests already want.
- [ ] **Non-strict choices** (clap#5885) — known values complete, document and
      suggest; an unknown value is still accepted. mise's tool names are this
      exact shape: a registry to offer, arbitrary backends still legal.
- [ ] **Completion runtime niceties** — completions that work when the binary
      is invoked through a shell alias (clap#1764, stalled since 2020), and
      multi-segment path completion, `tar/de/inc` completing to
      `target/debug/incremental` (clap#5279). usage owns the registration
      scripts and the `complete-word` runtime, so each is implementable once,
      for every shell.
- [ ] **`--flag=false` on booleans** (clap#5577; clap#1649 closed with 28
      reactions behind it) — **semantic decided (2026-08-19): opt-in per flag,
      and `=`-attached only.** `--flag=false` binds; `--flag false` never does,
      so the `=` settles the next word's role and no existing bool changes
      behavior. The wider rule that comes with it: optional values on flags
      deserve an admonishment in the docs, and possibly a lint — a detached
      optional value is ambiguous to a human reader even where the grammar
      resolves it (the parser already gives `--color bar`'s `bar` to the
      positionals) — so the recommended declaration is `default_missing` with
      `require_equals` beside it.
- [ ] **`license` metadata** (clap#1768) — a spec node rendered into manpages
      and docs. GPL display requirements want it, and it rounds out the
      manpage story.

Demand also attaches to boxes already open above: fixed arity with distinct
value names is clap#1717 + clap#1682 (31 votes combined); `Option` on a
flattened group is clap#5092 (18) — the derive refuses it for lack of a rule,
and the votes say people want the rule defined; visible aliases on enum values
is clap#4416, stalled in clap on binary-size grounds a spec interpreter does
not have; and a help template set once for the whole tree is clap#1184, which
is the `help_template` row — a Tera template at spec root is the natural shape.

Noted, not taken — one item: conditional argument groups unlocked by a flag's
value (clap#6258), the missing quadrant beside `requires_if`, `required_if`
and `default_if` — "this flag is invalid unless that flag has value X". Not
built on its own, and not designed on its own either: it is a constraint on
the group-as-enum item above, whose design must leave room for a group whose
membership condition is a value. The enforcement half may already have a home
in the expr layer: `validate` today scopes one value, and widened to command
scope over all bound flags, "`--dockerfile` without `driver == "docker"`" is
one declarative expression — covering this and the long tail of cross-flag
rules without new vocabulary. What an expr cannot do is the other half: help
and completions cannot read a black-box expression to know not to offer a
flag, which is why the structured group vocabulary stays the answer for
anything those need to understand.

**Declined: `env_prefix`** (clap#3221, 45 votes). Assembling `MISE_JOBS` from a
prefix and a field name makes the one string a user actually sees ungreppable
in the codebase that declares it. Env names stay fully spelled at the
declaration site.

**Declined: conflict-aware positional skipping** (clap#1794, Deno's shape,
still unmerged in clap in 2026). Handing a word to the _next_ positional
because a flag elsewhere on the line conflicts with the first one requires the
binder to consult relationship tables mid-parse, and the architecture rule is
that binding stays relationship-free — every check that needs more lives after
the parse. It is also bad grammar independent of the architecture: which slot
a word lands in would depend on the rest of the line, which is unpredictable
for exactly the readers a spec serves.

**Declined: case-insensitive subcommand matching** (clap#6097, closed "not
planned" in clap as well). The demand comes from mobile keyboards and chat-bot
REPLs, not shells; no fleet CLI wants it; and the hot path's exact byte
comparison against static tables is budget not worth spending here.

**Non-goals, now stated rather than implied:** interactive prompts (clap#1634);
non-Unix option styles — `find -exec`, `/c`, `-Wl,` (clap#2468) — the framework
targets GNU-style CLIs on purpose; and no_std (clap#1485), though usage-argv
being dependency-free and allocation-free means the distance is small if
embedded ever matters. i18n (clap#380, open since 2015) is the long-term
sleeper: clap structurally cannot do it because every string is compile-time
Rust, while a spec is data — not built now, but worth a line in the vision
docs.

## Not covered by the corpus yet

- [ ] **Restart tokens** — `restart_token` (mise's `:::`) makes one command line
      describe several invocations. `expect` holds a single result, so this needs a
      multi-invocation vector shape first.
- [ ] **Mounts** — `mount` resolves a sub-spec by running a command mid-parse,
      which makes a vector depend on an external process. Needs stubbing.
- [ ] **Completion parsing** — `parse_partial` accepts deliberately incomplete
      input. Different contract, different expectations, its own corpus.

## Known usage-lib divergences

**The corpus records none today**: usage-lib answers every vector, and so do
usage-argv and the Go runner. That is a measurement, checked on every run
rather than asserted here. What is left below is the history, plus the one
item marked _needs a decision_ — which is not a divergence but a question about
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
- [x] **usage-lib accepts three things usage-argv and clap both refuse** — _withdrawn as
      stated, and the correction is the useful part._ The differential fuzzer found three, and
      this entry called all three usage-lib's to tighten. One was: `subcommand_required` was
      in the spec and no parser read it, fixed in #992. **The other two are the grammar working
      as specified**, and the corpus says so in its own words — `long-repeated-keeps-the-last`
      ("a repeat is a correction… the later occurrence wins") and `long-unknown` ("more likely
      data in transit than a mistake", with `unknown_flags "error"` as the opt-in). Acting on
      the wrong reading got as far as three failing conformance vectors.
      One refusal comes from a different layer: for repeated command-line occurrences,
      `Error::DuplicateFlag` is constructed only in `derive/src/codegen.rs`, never by
      usage-argv's parser, so a derive-generated binary still rejects a repeated flag as clap
      does. Separately, `Spec::to_kdl` validates `duplicate_flag_form` at the spec boundary so
      two declarations cannot claim the same spelling. Unknown flags are intentionally different: usage parsers
      are permissive by default and a command opts into `unknown_flags="error"` when it owns the
      whole grammar. That is what fleet adopters should declare for clap parity, while forwarding
      commands such as `mise run` keep the default. `differential.rs` carries a named test so
      tightening usage-lib fails with the reason attached.
- [x] Unrecognized flags fall through to positionals, so `ex --wat` binds `--wat`
      to an argument, or reports `unexpected_arg` when there is none. **Decided
      (2026-08-19): lax is the default everywhere — both parsers — and strict is
      the opt-in, `unknown_flags "error"` at whatever scope wants it.** This is
      a position, not a compatibility concession: clap's strict default is held
      to be the wrong one — a wrapper appending to a command line it did not
      write is ordinary, which is the grammar's "data in transit" rationale —
      and what `mise run` tasks accept does not change. The derive's strictness
      about a _repeated_ non-repeatable flag is a separate rule and unchanged.
      The migration guide presents the difference as intentional, with strict
      one root-level line away, as communique's rewrite already declares it.
- [x] A flag missing its value is dropped silently — now an error, in `parse` but
      not `parse_partial`, since a half-typed flag is exactly what a completion is
      asked about.
- [x] `=` is kept in attached short values, so `-j=8` binds `=8`.
- [x] A repeated `--` was eaten, altering a forwarded command line containing
      its own separator. Only the first `--` is parser syntax; later separators
      are data and are preserved. `double_dash="preserve"` has the narrower role
      of preserving the first separator too. Fixed in #809.
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
- [~] **A prop vocabulary that is the union of the four registries** — `type`
  (bool, int, string, path, duration, list, map, plus a Rust-type escape
  hatch), `default`, `env` and `deprecated_env`, `docs`, `deprecated` with
  warn/remove versions, `enum`, `optional`, `aliases`, `merge`
  (`replace`/`union` — hk needs union for its list settings), `scope`
  (mise strips `global_only` settings out of project files, which is a
  security property, not a preference), and per-source bindings in hk's
  `sources.{cli,env,git,...}` shape.
- [x] **Named, ordered, pluggable layers.** The order is CLI flags, then
      environment, then env-files, then the project file (found upward, with
      `.local` variants outranking their base), then user-global, then system, then
      defaults. That already matches all four CLIs wherever a layer is present;
      which layers exist stays per-CLI, so hk's git-config layer and mise's `/etc`
      and `conf.d` layers slot in without being universal.
- [x] **Generate the CLI binding** rather than hand-writing it. This is the single
      highest-value piece: it is what all four wrote by hand and all four got
      subtly wrong. `#[usage(setting = "jobs")]` on a flag emits into
      `Ex::SETTINGS_BINDINGS`, and `Registry::drift` compares the executable
      bindings against the documented ones — which is what hk's eighteen declared
      and five read `sources.cli` lines needed and never had.
- [x] **Provenance through one merge path**, so `<bin> config explain` comes free
      everywhere instead of needing a parallel implementation. `config/src/explain.rs`
      — `explain`, `warnings`, `list`.
- [~] **Extend `SpecConfigProp` first.** Mostly done, and the list of what was
  missing has shrunk to two: `deprecated`, `merge`, `scope`, the per-source
  `bindings`, and `choices` (the `enum` case) all exist now, alongside
  `default`, `data_type`, `value_type`, `env`/`envs`, `cli`, and the help
  fields. **`optional` and `aliases` are still absent.** The markdown renderer
  reads the block, so it is no longer true that nothing consumes it — but no
  _CLI_ emits one yet, which is the adoption half below.
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
