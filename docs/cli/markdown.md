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
