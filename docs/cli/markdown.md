# Generating Markdown Documentation

Usage CLI can generate markdown documentation from a Usage definition either into a single file, or a directory.

Single file, written with `--out-file` (or to stdout, the default, so you can redirect it
wherever you like):

```sh
$ usage g markdown -f ./mycli.usage.kdl --out-file ./docs/cli.md
$ usage g markdown -f ./mycli.usage.kdl > ./docs/cli.md
```

Multiple files:

```sh
$ usage g markdown -mf ./mycli.usage.kdl --out-dir ./docs
$ tree ./docs
docs
├── config
│   ├── add.md
│   ├── list.md
│   └── remove.md
├── index.md
└── update.md
```

## Custom templates from the CLI

Each renderer template can be replaced with a Tera template file. For example, a custom
single-file document can keep the built-in command template:

```sh
$ usage g markdown -f ./mycli.usage.kdl \
    --template spec=./templates/spec.md.tera \
    --out-file ./docs/cli.md
```

```tera
{# templates/spec.md.tera #}
# {{ spec.bin }} reference
{% set cmd = spec.cmd %}
{% include "cmd_template.md.tera" %}
```

The available names are `spec`, `index`, `command`, `argument`, `flag`, and `config`. Repeat
`--template` to replace more than one. Templates that are not named keep their built-in
definitions and remain available through Tera's `include`.

## Custom templates from Rust

`MarkdownRenderer` bundles a complete set of Tera templates. A Rust caller can replace one
member without copying the rest:

```rust
use usage::docs::markdown::{MarkdownRenderer, MarkdownTemplate};

let spec: usage::Spec = std::fs::read_to_string("mycli.usage.kdl")?.parse()?;
let markdown = MarkdownRenderer::new(spec)
    .with_template(
        MarkdownTemplate::Spec,
        "# {{ spec.bin }} reference\n{% set cmd = spec.cmd %}\n{% include \"cmd_template.md.tera\" %}",
    )
    .render_spec()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The selectable members are `Spec`, `Index`, `Command`, `Argument`, `Flag`, and `Config`.
Templates that are not replaced remain available through Tera's `include`; syntax and include
errors are returned when the page is rendered.
