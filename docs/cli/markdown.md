# Generating Markdown Documentation

`usage generate markdown` renders a spec as Markdown reference pages: one page for the whole
CLI, or one per command for a docs site with a sidebar. The [CLI reference](/cli/reference/) on
this site is its output for `usage`'s own spec.

A single file goes to `--out-file`, or to stdout when there is none:

```sh
usage g markdown -f ./mycli.usage.kdl --out-file ./docs/cli.md
usage g markdown -f ./mycli.usage.kdl > ./docs/cli.md
```

`--multi` writes one page per command into `--out-dir`, nested the way the commands are:

```sh
usage g markdown -mf ./mycli.usage.kdl --out-dir ./docs
tree ./docs
```

```text
docs
├── config
│   ├── add.md
│   ├── list.md
│   └── remove.md
├── index.md
└── update.md
```

Links between the pages are written from the root of the output, as `/bash.md`.
`--url-prefix /cli/reference` puts a path in front of them, `/cli/reference/bash.md`, which is
what a docs site serving the pages under a subdirectory needs.

## Custom templates from the CLI

Every part of the output comes from a [Tera](https://keats.github.io/tera/) template, and
`--template NAME=PATH` replaces one. A custom single-file document can keep the built-in
command template by including it:

```sh
usage g markdown -f ./mycli.usage.kdl \
    --template spec=./templates/spec.md.tera \
    --out-file ./docs/cli.md
```

```tera
{# templates/spec.md.tera #}
# {{ spec.bin }} reference
{% set cmd = spec.cmd %}
{% include "cmd_template.md.tera" %}
```

The names are `spec`, `index`, `command`, `argument`, `flag`, and `config`. Repeat `--template`
to replace more than one. Templates that are not named keep their built-in definitions and
remain available through Tera's `include`.

## Custom templates from Rust

`MarkdownRenderer` in `usage-lib` bundles the same templates. A Rust caller replaces one member
without copying the rest:

```rust
use usage::docs::markdown::{MarkdownRenderer, MarkdownTemplate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec: usage::Spec = std::fs::read_to_string("mycli.usage.kdl")?.parse()?;
    let markdown = MarkdownRenderer::new(spec)
        .with_template(
            MarkdownTemplate::Spec,
            "# {{ spec.bin }} reference\n{% set cmd = spec.cmd %}\n{% include \"cmd_template.md.tera\" %}",
        )
        .render_spec()?;
    print!("{markdown}");
    Ok(())
}
```

The members are `Spec`, `Index`, `Command`, `Argument`, `Flag`, and `Config`. Templates that are
not replaced remain available through `include`; syntax and include errors are returned when
the page is rendered, not when the template is set.

Pages use `MarkdownTheme::Compact` by default: arguments and flags are grouped into dense lists
that stay easy to scan on a large page. `MarkdownTheme::Detailed` gives each entry its own
heading, so that a flag can be linked to directly:

```rust
use usage::docs::markdown::{MarkdownRenderer, MarkdownTheme};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let spec: usage::Spec = std::fs::read_to_string("mycli.usage.kdl")?.parse()?;
    let markdown = MarkdownRenderer::new(spec)
        .with_theme(MarkdownTheme::Detailed)
        .render_spec()?;
    print!("{markdown}");
    Ok(())
}
```
