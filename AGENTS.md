# AGENTS.md

## mbx build cache

Compilation-heavy mise tasks use `mbx`. If an mbx command fails or creates a
development papercut, rerun the exact equivalent `cargo` command from
`CONTRIBUTING.md`; this unblocks work without weakening the check. If Cargo
succeeds, surface the mismatch and recommend a
[mr-boxington Discussion](https://github.com/jdx/mr-boxington/discussions) with
the repository and commit, OS, `mbx --version`, both commands and outputs, the
cache summary, and `MBX_BYPASS_LOG` details when relevant. Do not silently make
Cargo the permanent path, and do not post externally without user authorization.

This file provides guidance to coding agents working in this repository.

## Conventional Commits

All commit messages and PR titles MUST follow conventional commit format:

**Format:** `<type>(<scope>): <description>`

**Types:**

- `feat:` - New features
- `fix:` - Bug fixes that affect the CLI behavior (not CI, docs, or infrastructure)
- `refactor:` - Code refactoring
- `docs:` - Documentation changes
- `style:` - Code style/formatting (no logic changes)
- `perf:` - Performance improvements
- `test:` - Testing changes
- `chore:` - Maintenance tasks, releases, dependency updates, CI/infrastructure changes
- `security:` - Security-related changes

**Scopes:**

- For shell-specific changes: `bash`, `zsh`, `fish`, `powershell`
- For subsystem changes: `spec`, `parse`, `complete`, `docs`, `manpage`, `lib`, `cli`, `deps`

**Description Style:**

- Use lowercase after the colon
- Use imperative mood ("add feature" not "added feature")
- Keep it concise but descriptive

**Examples:**

- `fix(zsh): handle spaces in completion values`
- `feat(powershell): add completion support`
- `feat(spec): add mount node for nested specs`
- `docs: update KDL spec format examples`
- `chore: release 2.0.0`

## Project Overview

Usage is a spec and CLI for defining CLI tools using KDL format. It generates shell completions, markdown docs, and man pages from a single spec file. Think OpenAPI/Swagger for CLIs.

## Build and Test Commands

```bash
# Build all packages
cargo build --all

# Run all tests
cargo test --all --all-features

# Run a single test
cargo test -p usage-lib test_name
cargo test -p usage-cli test_name

# Update snapshots (uses cargo-insta)
cargo insta test --accept

# Lint and format
cargo clippy --all --all-features -- -D warnings
cargo fmt --all
prettier -w .

# Full CI check
mise run ci

# Render completions, docs, and assets
mise run render
```

## Workspace Structure

The Cargo workspace contains the published libraries and CLI plus internal
conformance, generation, and performance tooling:

- **lib** (`usage-lib`): Reference implementation for parsing specs and argv,
  generating shell completions, and rendering CLI help, Markdown, and man pages
- **cli** (`usage-cli`): Command-line interface over `usage-lib`
- **argv** (`usage-argv`): Dependency-free, zero-allocation runtime for compiled
  argv parse tables; optional features add spec metadata, completion splitting,
  and diagnostics
- **config** (`usage-config`): Layered configuration resolution with provenance
  and opt-in TOML, JSON, and YAML readers
- **derive** (`usage-derive`): Procedural macros that compile Rust CLI
  declarations into argv parse tables and usage specs
- **usage-rs** (`usage-rs`): Application-facing Rust facade that combines the
  argv runtime and derives, with optional config, validation, completion, and
  test support
- **clap_usage**: Generates usage specs from clap `Command` definitions
- **test** (`usage-test`): Assertion helpers for CLIs built with usage
- **validation** (`usage-validation`): Portable expression validation shared by
  generated parsers and the reference implementation
- **conformance** (`usage-conformance`): Unpublished harness and corpus covering
  argv, derives, config, validation, completions, rendering, and reference parity
- **xtask**: Unpublished maintainer generators for shadow CLIs and reference
  help-page data
- **benches/gate** and **benches/shadows/**: The performance/correctness gate and
  checked-in generated CLI crates used to compare usage with other parsers at
  realistic scale

## Architecture

### Rust Derives (`derive/`, `usage-rs/`)

The Rust framework uses `#[derive(Cli)]`, `Args`, `Subcommands`, `ValueEnum`, and
`ArgGroup` to compile typed declarations into argv parse tables and portable spec
metadata. All derive metadata uses the native `#[usage(...)]` helper attribute.
Legacy clap helper namespaces such as `#[command(...)]`, `#[arg(...)]`,
`#[value(...)]`, and `#[group(...)]` are rejected with migration diagnostics; do
not add compatibility spellings back. Keep clap migration examples as explicit
before/after rewrites to `#[usage(...)]`.

### Spec Model (`lib/src/spec/`)

The spec model represents a CLI definition parsed from KDL:

- `Spec` - Root struct containing name, version, commands, global completers
- `SpecCommand` - A command/subcommand with args, flags, and nested subcommands
- `SpecFlag` - A flag definition (`-v`, `--verbose`, `--config <path>`)
- `SpecArg` - A positional argument (`<input>`, `[optional]`, `<files>...`)
- `SpecComplete` - Custom completion definitions (shell commands to run)
- `SpecMount` - Mount another spec at a subcommand path

Specs can be:

1. Parsed from `.usage.kdl` files
2. Extracted from embedded `# USAGE:` comments in scripts
3. Generated from clap Command definitions

### Shell Completion Generation (`lib/src/complete/`)

Each shell has its own module generating completion scripts:

- `bash.rs` - Uses `complete` builtin
- `zsh.rs` - Uses `compdef`
- `fish.rs` - Uses `complete` command
- `powershell.rs` - Uses `Register-ArgumentCompleter`

Completions call back to `usage complete-word` at runtime for dynamic completions.

### Argument Parsing (`lib/src/parse.rs`)

The `parse()` function parses command-line arguments against a spec, returning:

- Matched command path
- Parsed args and flags with values
- Env var and default fallbacks applied
- Provenance: `tokens` says what each word of argv became, and
  `flag_origins`/`arg_origins` say where a value came from when no token supplied
  it. `Parser::explain` returns all of it with the errors kept rather than bailing
  on the first, which is what `usage explain` renders.

### Documentation Generation (`lib/src/docs/`)

Generates from specs:

- Markdown documentation (`markdown/`)
- Man pages (`manpage/`)
- CLI help text (`cli/`)

Uses Tera templates for markdown rendering.

## KDL Spec Format

Specs use KDL syntax. Key nodes:

```kdl
name "mycli"
bin "mycli"
flag "-v --verbose" help="Enable verbose output"
arg "<input>" help="Input file"
cmd "subcommand" {
    flag "--force"
    arg "[optional]"
}
complete "input" run="find . -name '*.txt'"
```

## Testing

- Snapshot tests use `cargo-insta` with auto-review enabled
- Shell integration tests require bash, zsh, fish, and pwsh installed
- Run `cargo insta test --accept` to update snapshots

## Performance Gate

`perf-pr` compares instruction counts against the merge base and fails when one
rises beyond the gate. It is a signal, not a blocking check: it exists so a
regression is noticed by the person who caused it, and a red perf check is never
by itself a reason to change how the check works.

Never loosen, disable, or route around the gate to make a pull request pass. That
means no `--gate-pct` on the `tak compare` line in `.github/workflows/perf-pr.yml`,
no raising `gate.pct` in `tak.toml`, no dropping a benchmark, and no
`continue-on-error` on the compare or gate steps. The same rule applies to any
other check whose purpose is to tell a human something: don't neuter the check to
get a green square.

When a change genuinely costs instructions, measure it, say so in the pull request
with the numbers, and **ask the user** how to proceed. Let them decide between
absorbing the cost, optimizing, or adjusting the gate themselves. This applies
generally: whenever the fix under consideration is more invasive than the problem —
bypassing a check, weakening a test, deleting an assertion, widening a lint
allowance — stop and ask instead of doing it.

## Key Dependencies

- `kdl`: KDL parser for spec files
- `clap`: CLI parsing for the usage tool itself
- `miette`: Error reporting with diagnostics
- `tera`: Template engine for markdown docs
- `insta`: Snapshot testing

## GitHub Interactions

When posting comments on GitHub PRs or discussions, always include a note that the comment was AI-generated (e.g., "_This comment was generated by Claude Code._").
