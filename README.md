# Usage

**Define your CLI once. Generate the tools around it.**

Usage is a portable [KDL](https://kdl.dev/) specification, a command-line utility,
and a Rust framework. Describe commands, flags, arguments, and settings in one
place, then use that definition for parsing, help, shell completions, Markdown
docs, man pages, and typed clients.

[Get started](https://usage.jdx.dev/guide/getting-started) ·
[Rust framework](https://usage.jdx.dev/rust/) ·
[Spec reference](https://usage.jdx.dev/spec/reference/) ·
[CLI reference](https://usage.jdx.dev/cli/reference/)

| Your starting point   | How Usage fits                                                                               |
| --------------------- | -------------------------------------------------------------------------------------------- |
| A new Rust CLI        | Derive a typed parser and exportable spec from structs and enums.                            |
| An existing CLI       | [Export a spec](https://usage.jdx.dev/spec/integrations) from your framework, or write KDL.  |
| A script              | [Declare arguments in comments](https://usage.jdx.dev/cli/scripts) and let Usage parse them. |
| Code that calls a CLI | [Generate a TypeScript or Python SDK](https://usage.jdx.dev/cli/sdk).                        |

## Rust framework

Applications can use `usage-rs` to derive a typed parser and a portable Usage
spec from the same Rust declaration:

```toml
[dependencies]
usage = { package = "usage-rs", version = "6" }
```

```rust
use usage::Cli;

#[derive(Cli)]
#[usage(bin = "example", version)]
struct App {
    /// Print more detail.
    #[usage(short = 'v', long, count)]
    verbose: u8,

    /// Files to process.
    files: Vec<String>,
}

fn main() {
    let app = App::parse();
    // app.verbose and app.files are ready to use
}
```

Usage has its own derive vocabulary: use `#[usage(...)]` on commands, fields,
and value variants. See the [Rust framework guide](https://usage.jdx.dev/rust/)
and [clap migration guide](https://usage.jdx.dev/rust/migrating-from-clap) for
the supported mappings and intentional differences.

## Standalone CLI

Choose one installation method:

```sh
# mise
mise use -g usage

# Homebrew
brew install usage

# Cargo
cargo install usage-cli --locked
```

The package is `usage-cli`; the executable is `usage`.
[Other installation options](https://usage.jdx.dev/cli/#installation).

With a spec saved as `mycli.usage.kdl`:

```sh
usage lint mycli.usage.kdl
usage generate completion zsh mycli --file mycli.usage.kdl --install
usage generate markdown --file mycli.usage.kdl --out-file reference.md
usage generate manpage --file mycli.usage.kdl --out-file mycli.1
```

The generated shell scripts need `usage` at completion time. The Rust framework
can also [provide completions directly](https://usage.jdx.dev/rust/completions)
from your binary.

Follow the [spec walkthrough](https://usage.jdx.dev/guide/getting-started) for a
complete example. The [Go framework](https://usage.jdx.dev/go/) is a development
preview and is not ready for adoption or testing.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, checks, and generated
files.

## Sponsors

<p align="center">
  Sponsored by<br><br>
  <a href="https://entire.io">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://jdx.dev/sponsors/entire-lockup.svg">
      <img src="https://jdx.dev/sponsors/entire-lockup-on-light.svg" alt="Entire" height="36">
    </picture>
  </a>
  &nbsp;&nbsp;&nbsp;
  <a href="https://omarchy.org/patrons/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://jdx.dev/sponsors/omacom-foundation.svg">
      <img src="https://jdx.dev/sponsors/omacom-foundation-on-light.svg" alt="Omacom Foundation" height="36">
    </picture>
  </a>
  <br><br>
  <a href="https://jdx.dev/sponsors.html">View all sponsors</a>
</p>

## Acknowledgements

Usage's design owes a great deal to [clap](https://github.com/clap-rs/clap). Its
help output and diagnostic conventions make clap migrations familiar, while
Usage's native derive attributes reflect its portable spec model. clap's license
is reproduced in [NOTICE.md](NOTICE.md).

## License

[MIT](LICENSE). Third-party notices are in [NOTICE.md](NOTICE.md).
