# Usage

Usage is a spec and CLI for defining CLI tools. Arguments, flags, environment variables, and config files
can all be defined in a Usage spec. It can be thought of like [OpenAPI (swagger)](https://www.openapis.org/)
for CLIs. Here are some potential reasons for defining your CLI with a Usage spec:

- Generate autocompletion scripts
- Generate markdown documentation
- Generate man pages
- Use an advanced arg parser in any language
- Scaffold one spec into different CLI frameworks—even different languages
- [coming soon] Host your CLI documentation on usage.sh

See more at [usage.jdx.dev](https://usage.jdx.dev/).

## Sponsors

usage is sponsored by [entire.io](https://entire.io) and [37signals](https://37signals.com).

[View all sponsors](https://jdx.dev/sponsors.html).

## Acknowledgements

Usage's design owes a great deal to [clap](https://github.com/clap-rs/clap) — the
derive attribute vocabulary, the help output shape, and the diagnostic conventions
all follow it deliberately so clap CLIs can be ported field by field. clap's
license is reproduced in [NOTICE.md](NOTICE.md).

## License

[MIT](LICENSE). Third-party notices — including the GPL-2.0-or-later
[bash-completion](https://github.com/scop/bash-completion) script that
`usage generate completion bash --include-bash-completion-lib` emits — are in
[NOTICE.md](NOTICE.md).
