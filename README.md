# Usage

Usage is a spec, CLI, and Rust framework for defining command-line interfaces.
Arguments, flags, environment variables, and config files can all be described in
a portable KDL spec. Think of it as [OpenAPI](https://www.openapis.org/) for CLIs:
one declaration can drive parsing and every user-facing artifact.

- Generate shell completions
- Generate Markdown documentation and man pages
- Parse arguments from any language
- Scaffold a spec into CLI frameworks in different languages
- Build a typed Rust CLI with a zero-dependency runtime

See more at [usage.jdx.dev](https://usage.jdx.dev/).

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

## Sponsors

usage is sponsored by [entire.io](https://entire.io) and [37signals](https://37signals.com).

[View all sponsors](https://jdx.dev/sponsors.html).

## Acknowledgements

Usage's design owes a great deal to [clap](https://github.com/clap-rs/clap). Its
help output and diagnostic conventions make clap migrations familiar, while
Usage's native derive attributes reflect its portable spec model. clap's license
is reproduced in [NOTICE.md](NOTICE.md).

## License

[MIT](LICENSE). Third-party notices are in [NOTICE.md](NOTICE.md).
