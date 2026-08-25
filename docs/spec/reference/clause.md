# Clauses

A clause is a repeatable group of positional arguments. A separator ends the current
instance and starts another without discarding the values already parsed.

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

Version 1 deliberately keeps clauses narrow:

- A command may declare one clause.
- A clause contains positional `arg` nodes only.
- Top-level arguments, `restart_token`, and sigil arguments cannot be combined with a
  clause on the same command.
- Defaults and environment variables do not fill inner arguments; every instance reflects
  argv supplied for that instance.

Use [sigil arguments](./sigils.md) to classify independent prefixed values. Use a clause
when several adjacent positional values form one repeatable unit.
