# Integrations

Integrations extract CLI definitions from framework internals and output a [usage spec](/spec/) in KDL format. This enables shell completions, markdown docs, and man pages from your existing CLI framework — no manual spec authoring needed.

## Available

| Framework                                                           | Language | Package                                                                        |
| ------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------ |
| [Cobra](https://github.com/spf13/cobra)                             | Go       | [`cobra_usage`](/spec/integrations/cobra)                                      |
| [clap](https://crates.io/crates/clap)                               | Rust     | [`clap_usage`](/spec/integrations/clap)                                        |
| [argparse](https://docs.python.org/3/library/argparse.html)         | Python   | [`argparse-usage`](https://github.com/acidghost/argparse-usage)                |
| [OptionParser](https://docs.ruby-lang.org/en/4.0/OptionParser.html) | Ruby     | [`option_parser_usage`](https://github.com/packrat386/option_parser_usage)     |
| [Commander.js](https://github.com/tj/commander.js)                  | Node.js  | [`@usage-spec/commander`](https://www.npmjs.com/package/@usage-spec/commander) |
| [oclif](https://oclif.io)                                           | Node.js  | [`@usage-spec/oclif`](https://www.npmjs.com/package/@usage-spec/oclif)         |
| [yargs](https://yargs.js.org)                                       | Node.js  | [`@usage-spec/yargs`](https://www.npmjs.com/package/@usage-spec/yargs)         |
| [Typer](https://github.com/fastapi/typer)                           | Python   | [`usage-spec-typer`](https://pypi.org/project/usage-spec-typer/)               |
| [Click](https://github.com/pallets/click)                           | Python   | [`usage-spec-click`](https://pypi.org/project/usage-spec-click/)               |

## Planned

Contributions welcome! Here are the frameworks we'd like to support next.

### High Priority

| Framework                                   | Language |
| ------------------------------------------- | -------- |
| [urfave/cli](https://github.com/urfave/cli) | Go       |

### Medium Priority

| Framework                                                               | Language |
| ----------------------------------------------------------------------- | -------- |
| [Spectre.Console](https://github.com/spectreconsole/spectre.console)    | C#/.NET  |
| [Symfony Console](https://github.com/symfony/console)                   | PHP      |
| [picocli](https://github.com/remkop/picocli)                            | Java     |
| [Thor](https://github.com/rails/thor)                                   | Ruby     |
| [cxxopts](https://github.com/jarro2783/cxxopts)                         | C++      |
| [CommandLineParser](https://github.com/commandlineparser/commandline)   | C#/.NET  |
| [CLI11](https://github.com/CLIUtils/CLI11)                              | C++      |
| [Laravel Zero](https://github.com/laravel-zero/laravel-zero)            | PHP      |
| [swift-argument-parser](https://github.com/apple/swift-argument-parser) | Swift    |
| [System.CommandLine](https://github.com/dotnet/command-line-api)        | C#/.NET  |

### Lower Priority

| Framework                                                                  | Language |
| -------------------------------------------------------------------------- | -------- |
| [Kong](https://github.com/alecthomas/kong)                                 | Go       |
| [Clikt](https://github.com/ajalt/clikt)                                    | Kotlin   |
| [JCommander](https://github.com/cbeust/jcommander)                         | Java     |
| [argh](https://github.com/google/argh)                                     | Rust     |
| [zig-clap](https://github.com/Hejsil/zig-clap)                             | Zig      |
| [optparse-applicative](https://github.com/pcapriotti/optparse-applicative) | Haskell  |
| [kotlinx-cli](https://github.com/Kotlin/kotlinx-cli)                       | Kotlin   |
| [cligen](https://github.com/c-blake/cligen)                                | Nim      |
| [argparse](https://github.com/mpeterv/argparse)                            | Lua      |
| [Getopt::Long](https://perldoc.perl.org/Getopt::Long)                      | Perl     |
| [OptionParser](https://hexdocs.pm/elixir/OptionParser.html)                | Elixir   |
| [getopt](https://man7.org/linux/man-pages/man3/getopt.3.html)              | C        |
