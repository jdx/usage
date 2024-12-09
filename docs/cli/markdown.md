# Generating Markdown Documentation

Usage CLI can generate markdown documentation from a Usage definition either into a single file, or a directory.

Single file (will be injected in the comment):

```sh
$ cat <<EOF > ./README.md
# My CLI
## Header
...
## CLI Commands
<!-- usage:start -->
## Footer
...
EOF
$ usage g markdown -f ./mycli.usage.kdl --inject README.md
$ cat README.md
# My CLI
## Header
## CLI Commands
<!-- usage:start -->
### `mycli config add KEY VALUE`
### `mycli config remove NAME`
<!-- usage:end   -->
## Footer
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
