### `plugins ls`

* Aliases: `list`

#### Flags

* `-a,--all` – List all available remote plugins
Same as `mise plugins ls-remote`
* `-c,--core` – The built-in plugins only
Normally these are not shown
* `--user` – List installed plugins

This is the default behavior but can be used with --core
to show core and user plugins
* `-u,--urls` – Show the git url for each plugin
e.g.: https://github.com/asdf-vm/asdf-node.git
* `--refs` – Show the git refs for each plugin
e.g.: main 1234abc
List installed plugins

Can also show remotely available plugins to install.
Examples:
  $ mise plugins ls
  node
  ruby

  $ mise plugins ls --urls
  node    https://github.com/asdf-vm/asdf-node.git
  ruby    https://github.com/asdf-vm/asdf-ruby.git
