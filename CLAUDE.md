# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Conventional Commits

All commit messages and PR titles MUST follow conventional commit format:

**Format:** `<type>(<scope>): <description>`

**Types:**

- `feat:` - New features
- `fix:` - Bug fixes
- `refactor:` - Code refactoring
- `docs:` - Documentation changes
- `style:` - Code style/formatting (no logic changes)
- `perf:` - Performance improvements
- `test:` - Testing changes
- `chore:` - Maintenance tasks, releases, dependency updates
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

Three crates in a Cargo workspace:

- **lib** (`usage-lib`): Core library containing spec parsing, CLI argument parsing, shell completion generation, and documentation generation
- **cli** (`usage-cli`): Command-line tool that wraps the library
- **clap_usage**: Generates usage specs from clap Command definitions

## Architecture

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

## Key Dependencies

- `kdl`: KDL parser for spec files
- `clap`: CLI parsing for the usage tool itself
- `miette`: Error reporting with diagnostics
- `tera`: Template engine for markdown docs
- `insta`: Snapshot testing
