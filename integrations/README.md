# Usage Integrations

Integrations that generate usage specs from CLI framework definitions, enabling shell completions, docs, and man pages from a single source.

## Status

- [x] **clap** (Rust) - [`clap_usage`](https://crates.io/crates/clap_usage) ~16k stars

### High Priority

- [x] **Cobra** (Go) - [`cobra_usage`](cobra/) ~43k stars
- [ ] **Commander.js** (Node.js) - [tj/commander.js](https://github.com/tj/commander.js) ~28k stars
- [ ] **urfave/cli** (Go) - [urfave/cli](https://github.com/urfave/cli) ~24k stars
- [ ] **Typer** (Python) - [fastapi/typer](https://github.com/fastapi/typer) ~19k stars
- [ ] **Click** (Python) - [pallets/click](https://github.com/pallets/click) ~17k stars
- [ ] **argparse** (Python) - stdlib

### Medium Priority

- [ ] **yargs** (Node.js) - [yargs/yargs](https://github.com/yargs/yargs) ~11k stars
- [ ] **Spectre.Console** (C#/.NET) - [spectreconsole/spectre.console](https://github.com/spectreconsole/spectre.console) ~11k stars
- [ ] **Symfony Console** (PHP) - [symfony/console](https://github.com/symfony/console) ~10k stars
- [ ] **oclif** (Node.js) - [oclif/oclif](https://github.com/oclif/oclif) ~9k stars
- [ ] **picocli** (Java) - [remkop/picocli](https://github.com/remkop/picocli) ~5k stars
- [ ] **Thor** (Ruby) - [rails/thor](https://github.com/rails/thor) ~5k stars
- [ ] **cxxopts** (C++) - [jarro2783/cxxopts](https://github.com/jarro2783/cxxopts) ~5k stars
- [ ] **CommandLineParser** (C#/.NET) - [commandlineparser/commandline](https://github.com/commandlineparser/commandline) ~5k stars
- [ ] **CLI11** (C++) - [CLIUtils/CLI11](https://github.com/CLIUtils/CLI11) ~4k stars
- [ ] **Laravel Zero** (PHP) - [laravel-zero/laravel-zero](https://github.com/laravel-zero/laravel-zero) ~4k stars
- [ ] **swift-argument-parser** (Swift) - [apple/swift-argument-parser](https://github.com/apple/swift-argument-parser) ~4k stars
- [ ] **System.CommandLine** (C#/.NET) - [dotnet/command-line-api](https://github.com/dotnet/command-line-api) ~4k stars

### Lower Priority

- [ ] **Kong** (Go) - [alecthomas/kong](https://github.com/alecthomas/kong) ~3k stars
- [ ] **Clikt** (Kotlin) - [ajalt/clikt](https://github.com/ajalt/clikt) ~3k stars
- [ ] **JCommander** (Java) - [cbeust/jcommander](https://github.com/cbeust/jcommander) ~2k stars
- [ ] **argh** (Rust) - [google/argh](https://github.com/google/argh) ~2k stars
- [ ] **zig-clap** (Zig) - [Hejsil/zig-clap](https://github.com/Hejsil/zig-clap) ~1.5k stars
- [ ] **optparse-applicative** (Haskell) - [pcapriotti/optparse-applicative](https://github.com/pcapriotti/optparse-applicative) ~1k stars
- [ ] **kotlinx-cli** (Kotlin) - [Kotlin/kotlinx-cli](https://github.com/Kotlin/kotlinx-cli) ~900 stars
- [ ] **cligen** (Nim) - [c-blake/cligen](https://github.com/c-blake/cligen) ~560 stars
- [ ] **argparse** (Lua) - [mpeterv/argparse](https://github.com/mpeterv/argparse) ~285 stars
- [ ] **Getopt::Long** (Perl) - stdlib
- [ ] **OptionParser** (Elixir) - stdlib
- [ ] **OptionParser** (Ruby) - stdlib
- [ ] **getopt** (C) - POSIX stdlib

## How Integrations Work

An integration extracts the CLI definition (commands, flags, args, completions) from a framework's internal representation and outputs a [usage spec](https://usage.jdx.dev/spec/) in KDL format. This spec can then drive:

- **Shell completions** for bash, zsh, fish, PowerShell, and nushell
- **Markdown documentation**
- **Man pages**
- **`--help` output**

See [`clap_usage`](../clap_usage/) for a reference implementation.
