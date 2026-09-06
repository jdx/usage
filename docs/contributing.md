# Contributing

Contribute documentation, bug fixes, integrations, and improvements to Usage.
This guide covers the local workflow and what a pull request needs for review.

## Contribution expectations

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

## Set up a checkout

Install [mise](https://mise.jdx.dev), then run these commands from the repository
root. The tool versions are recorded in `mise.toml`.

```sh
mise install
mise run build
```

Mise activates the project's Cargo cache wrapper. For standalone Cargo commands,
use `mise exec -- cargo …` or an activated mise shell. See the
[repository contribution notes](https://github.com/jdx/usage/blob/main/CONTRIBUTING.md#mbx-build-cache)
if the wrapper fails.

## Work on the documentation

The website lives in `docs/` and uses VitePress. Start the development server:

```sh
mise run docs:dev
```

Build the production site and check its generated social images:

```sh
mise exec -- aube run docs:build
```

Edit guides directly. Files under `docs/cli/reference/` come from the CLI's spec;
change command help in the Rust source and regenerate them instead. The render
task also updates completion scripts and man pages, and runs formatters:

```sh
mise run render:usage-cli-completions
```

Use `mise run render` when all generated artifacts need refreshing. Review the
diff so generated changes match the intended interface.

## Test and format

Run the checks relevant to your change:

| Change                    | Check                                            |
| ------------------------- | ------------------------------------------------ |
| Rust code                 | `mise run test`                                  |
| One Rust test             | `mise exec -- cargo test -p usage-lib test_name` |
| Go runtime or integration | `mise run test:go`                               |
| Markdown and formatting   | `mise run lint:prettier`                         |
| Rust lint and formatting  | `mise run lint:clippy` and `mise run lint:fmt`   |
| Documentation website     | `mise exec -- aube run docs:build`               |
| Full CI checks            | `mise run ci`                                    |

`mise run lint-fix` applies formatters and automatic lint fixes across the project.
Review every change it produces. Snapshot tests use `cargo-insta`; when behavior
intentionally changes, use `mise exec -- cargo insta review` to inspect updates.
Shell integration tests require the shells they exercise, including Bash, Zsh,
Fish, and PowerShell.

## Prepare a pull request

Explain the problem, the resulting behavior, and how you verified it. CI must
pass and automated review comments must be addressed before maintainer review.

Use Conventional Commits for commit messages and PR titles. Keep the description
lowercase and imperative; scopes identify the affected subsystem:

- `fix(zsh): preserve spaces in completion values`
- `docs: clarify installation steps`
- `feat(spec): add a configuration node`
- `chore(ci): update the build workflow`

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
