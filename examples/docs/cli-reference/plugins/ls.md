# `mise plugins ls`

###### Aliases: `list`

List installed plugins

Can also show remotely available plugins to install.

##### Flag `-a --all`

List all available remote plugins
Same as `mise plugins ls-remote`

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

Examples:

    $ mise plugins ls
    node
    ruby

    $ mise plugins ls --urls
    node    https://github.com/asdf-vm/asdf-nodejs.git
    ruby    https://github.com/asdf-vm/asdf-ruby.git
