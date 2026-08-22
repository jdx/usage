# Contributing

Thank you for your interest in contributing to usage.

## Contribution Expectations

Before opening a PR, unless it is something obvious, consider creating a
discussion or mentioning what you plan to do in
[Discord](https://discord.gg/UBa7pJUN7Z). The important part is to settle the
direction before much review happens. usage has a specific scope and design
taste. I am comfortable saying no to changes that do not clearly fit.

Before I review a PR, CI must be passing and all automated AI review comments
must be addressed. If those are still open, assume I will wait to look at the
PR.

If I am on the fence about a contribution, I will probably reject it for that
reason alone. If I did not do this, usage would suffer from feature bloat. I
may also reject a PR if the quality is poor enough that I do not have confidence
the contributor can get it across the finish line. I do not have time to coach
contributors.

I get hundreds of PRs per week across my projects, so I do not have time to
respond to every PR with detailed context. A rejection may be brief.

## Code Style

All of these repos use [hk](https://hk.jdx.dev) for linting and formatting.
Run the checks before opening a PR:

```sh
hk check --all
hk fix --all
```

Some repos also expose wrapper tasks such as `mise run lint` and
`mise run lint-fix`; prefer those when they exist.

## Commit and PR Titles

Use Conventional Commits for commit messages and PR titles. Examples:

- `fix: handle missing config file`
- `docs: clarify installation steps`
- `feat: add quiet output mode`

## Testing

Testing differs by project. Run the relevant tests for the code you changed and
the repo's CI-style task when practical. Check `mise tasks`, `mise.toml`,
and existing README/docs for the exact commands.

## Development

Install project tools with mise:

```sh
mise install
```

Run the checks listed in the repository before opening a PR.

## Performance checks

`perf-pr` compares instruction counts against the merge base. It is a signal for
the person who caused a change, not a check to loosen so a pull request can
pass. If a change genuinely costs instructions, report the numbers and ask
whether to absorb the cost, optimize, or adjust the gate.

Two things move the counts without the parser changing:

- The `markdown` benchmark parses `cli/usage.usage.kdl`. Rewriting that spec
  (for example switching escaped newlines to KDL raw multiline strings) changes
  the parse cost even when generated markdown is identical.
- The mise shadow fixture grows and is refreshed from mise's command tree.
  `tasks/perf-shadow.sh` warns when the parser-to-parser ratio falls below 80x,
  which is the comparison that belongs to the parsers rather than the fixture.
