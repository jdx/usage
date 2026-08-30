# Clauses

A clause is a repeatable group of scoped flags and positional arguments. It can use an
explicit separator, or a required terminal positional can end each instance implicitly.

```kdl
min_usage_version "6.6"

clause "tasks" separator=":::" {
  arg "<task>"
  arg "[args]..." var=#true double_dash="automatic"
}
```

`run lint --fix ::: test --all` produces two `tasks` instances. Each instance is a
map keyed by its inner argument declarations: the first contains `task="lint"` and
`args=["--fix"]`; the second contains `task="test"` and `args=["--all"]`.

The separator is recognized even after `double_dash="automatic"` made tokens verbatim,
and it re-enables flags for the next instance. An explicit `--` protects the separator,
so `run lint -- :::` passes `:::` as data instead.

When `separator` is omitted, the clause must contain exactly one required,
non-variadic positional argument and no subcommands. Consuming that terminal positional
ends the current instance, resets its scoped flags, and starts the next one:

```kdl
clause "tools" {
  flag "--postinstall <COMMAND>"
  arg "<tool>"
}
```

`use --postinstall A a --postinstall B b` produces two `tools` instances. The first
contains `postinstall="A"` and `tool="a"`; the second contains `postinstall="B"` and
`tool="b"`. Scoped flags therefore precede and apply to the next terminal positional.
A scalar flag may appear at most once in one instance, but may appear again after that
instance's terminal positional. A scoped flag at the end of argv is invalid because it
does not belong to a completed instance.

Clauses deliberately keep their grammar narrow:

- A command may declare one clause.
- An explicit-separator clause may contain multiple positional `arg` nodes. An implicit
  clause has exactly one required, non-variadic positional.
- `flag` nodes inside the clause are scoped to one instance. Their spellings must not
  conflict with command-level flags.
- Top-level arguments, `restart_token`, and sigil arguments cannot be combined with a
  clause on the same command.
- Defaults and environment variables do not fill inner arguments; every instance reflects
  argv supplied for that instance.

Use [sigil arguments](./sigils.md) to classify independent prefixed values. Use a clause
when several adjacent positional values form one repeatable unit.

## Rust derive

The outer field is a `Vec<T>` and `T` derives `Args`:

```rust
#[derive(usage::Args)]
struct TaskClause {
    task: String,
    #[usage(double_dash = "automatic")]
    args: Vec<String>,
}

#[derive(usage::Cli)]
struct Run {
    #[usage(clause, separator = ":::")]
    tasks: Vec<TaskClause>,
}
```

The compiled parser emits a clause-boundary event and builds each nested partial independently.

For an implicit clause, omit `separator` and put scoped fields on the nested `Args` type:

```rust
#[derive(usage::Args)]
struct ToolClause {
    #[usage(long)]
    postinstall: Option<String>,
    tool: String,
}

#[derive(usage::Cli)]
struct Use {
    #[usage(clause)]
    tools: Vec<ToolClause>,
}
```
