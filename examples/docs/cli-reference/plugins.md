# `mise plugins`

###### Aliases: `p`

Manage plugins
## Subcommands

* `install [args] [flags]` - Install a plugin
* `link [args] [flags]` - Symlinks a plugin into mise
* `ls [flags]` - List installed plugins
* `ls-remote [flags]` - List all available remote plugins
* `uninstall [args] [flags]` - Removes a plugin
* `update [args] [flags]` - Updates a plugin to the latest version

##### Flag `-a --all`

list all available remote plugins

same as `mise plugins ls-remote`

##### Flag `-c --core`

The built-in plugins only
Normally these are not shown

##### Flag `--user`

List installed plugins

This is the default behavior but can be used with --core
to show core and user plugins

##### Flag `-u --urls`

Show the git url for each plugin
e.g.: https://github.com/asdf-vm/asdf-nodejs.git

##### Flag `--refs`

Show the git refs for each plugin
e.g.: main 1234abc
