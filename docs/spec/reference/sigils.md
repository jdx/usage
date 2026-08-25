# Sigil arguments

A sigil argument is a positional classified by a leading prefix instead of by its
place in the positional sequence. The prefix is syntax: the parser removes it before
storing, validating, or completing the value.

```kdl
min_usage_version "6.5"

arg "[tool]..." sigil="+" {
  choices "node@22" "node@24" "python@3.14"
}
arg "<command>"
arg "[args]..."
```

With that declaration, `ex +node@24 node -v` binds `tool=["node@24"]`,
`command="node"`, and `args=["-v"]`. A sigil argument never advances the ordinary
positional cursor, so several classified values can appear before the command.

## Matching

Sigils follow a deliberately small grammar:

- A token must start with the declared sigil and contain at least one byte after it.
  A bare `+` is ordinary positional data.
- When declarations overlap, the longest prefix wins. With `sigil="+"` and
  `sigil="++"`, `++edge` belongs to the latter and stores `edge`.
- A flag waiting for a detached value wins before sigil matching, so
  `--label +raw` stores the literal flag value `+raw`.
- Sigils are recognized only while flags are enabled and only in the leading segment.
  An explicit `--`, the first value of a `double_dash="automatic"` argument, or a
  command restart boundary makes later sigil-shaped tokens ordinary data.
- Binding a sigil argument does not block a later subcommand from being selected.

The sigil must be non-empty, may not start with `-`, and may not contain whitespace.
Each command may declare a sigil only once. Sigil arguments cannot use
`value_terminator` or a non-optional `double_dash` policy.

## Completion

Completion uses the argument declaration after removing the prefix, then restores the
prefix on every candidate. Given the example above, completing `+n` offers
`+node@22` and `+node@24`. This applies equally to fixed `choices`, built-in completion
types, and runtime completers.

The prefix is part of the shell word. Bash, zsh, fish, nushell, and PowerShell all pass
`+node` as one completion token; no shell-specific escaping is required.

## Rust derive

The same declaration is available on typed Rust fields:

```rust
#[derive(usage::Cli)]
struct Cli {
    #[usage(sigil = "+")]
    tools: Vec<String>,
    command: String,
    args: Vec<String>,
}
```

Generated Rust and Go tables carry the sigil in their hot parser metadata, so parsing,
completion, emitted KDL, and generated SDK invocation builders use the same contract.

Sigils classify individual arguments. For a repeatable sequence made of several
positionals separated by a boundary token, use a [clause](./clause.md).
