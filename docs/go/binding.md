# Binding and Values

::: warning Draft
This page is a draft. Some of what it documents is still in open pull requests, and details may
change before release.
:::

Generated `Parse` does everything on this page for you. It's documented separately because the
pieces are public — custom binding loops use them directly — and because the _rules_ matter even
when you never call the functions: they define what your users' command lines mean.

## Resolution order

Each flag or arg resolves **argv → env → default_if → default**, matching
[config resolution](/spec/resolution):

```go
values, source := argv.Fill(Meta.Lookup(key), given, argv.LookupEnv)
```

Fill every entry first, then `argv.ApplyDefaultIf` so a `default_if` can see a
sibling that came from argv or env. An applied `default_if` is `FromDefault`.

`Source` tells you where the value came from: `FromArgv`, `FromEnv`, `FromDefault`, or `Unset`.
`Source.Given()` is true only for the first two — a default is a fallback, not something the
user said, which matters for relations (below).

The env rules are precise and worth knowing:

- an **empty env variable is set** — `EX_JOBS=` provides the value `""`
- an env value is **one token, never re-split** on whitespace or commas
- for a value-less (boolean) flag, `argv.EnvTruth` decides whether the variable sets it at all —
  the allow-list is narrow (`1`, `true`, `True`, `TRUE`), so `yes`, `on`, and `TrUe` do **not**
  set the flag; this matches usage-lib

## Checks

```go
if err := argv.Check(meta, values, occurrences); err != nil { /* … */ }
```

- `required` is asked first: a required variadic given nothing is _missing_, not _short_
- `choices` are case-sensitive and every value is checked, not just the first
- `var_min` only fires when something was given — an absent optional variadic hasn't broken its
  minimum
- `var_max` counts **occurrences**, so one occurrence bringing three values doesn't break
  `var_max=1`

## Relations

```go
err := argv.CheckRelationships(Meta, selectedKeys, sourceOf)
winners := argv.ApplyOverrides(Meta, tokenOrder)
```

`conflicts`, `required_if`, and `required_unless` are judged with a deliberate asymmetry: a
defaulted value counts for the entry _being judged_ but not for the _partners judging it_ — so a
flag with a default doesn't conflict with everything anyone types.

`ApplyOverrides` implements last-one-wins on token order, symmetric regardless of which flag
declared the override, and runs _before_ fallbacks so a losing flag isn't refilled from env or a
default. Remember: [generated `Parse` does not call it](/go/generated-code#what-parse-enforces).

## Typed values

Generated struct fields are `string`/`[]string` — a spec names a value, it doesn't type it.
Conversions are explicit, and every failure is a `*argv.Error` with `CodeInvalidValue` carrying
the entry's name, the offending text, and a human phrase for what was expected:

```go
n, err := argv.Int("jobs", cli.Jobs)              // int64  — "a whole number"
u, err := argv.Uint("retries", cli.Retries)       // uint64 — "a whole number, not negative"
f, err := argv.Float("ratio", cli.Ratio)          // float64 — "a number"
b, err := argv.Bool("color", cli.Color)           // bool   — "true or false"
d, err := argv.Duration("wait", cli.Wait)         // time.Duration — "a duration such as 30s or 1h30m"

ports, err := argv.Each("ports", cli.Ports, argv.Int)  // []int64, stops at the first bad value
```

Two sharp edges are deliberate:

- **Nothing is trimmed.** `" 8 "` is refused, exactly as `" 8 ".parse::<i64>()` is in Rust — the
  same spec means the same thing in both implementations.
- **`Bool` is wider than `EnvTruth` on purpose.** `Bool` accepts Go's spellings (`1`, `t`, `T`,
  `true`, `TRUE`, `True` and the false counterparts) for a value someone typed; `EnvTruth` stays
  on usage-lib's narrow list for deciding whether an env var sets a value-less flag.
