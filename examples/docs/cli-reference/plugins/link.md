# `mise plugins link`

###### Aliases: `ln`

Symlinks a plugin into mise

This is used for developing a plugin.

###### Arg `<NAME>`

(required)The name of the plugin
e.g.: node, ruby

###### Arg `[PATH]`

The local path to the plugin
e.g.: ./mise-node

##### Flag `-f --force`

Overwrite existing plugin

Examples:
    # essentially just `ln -s ./mise-node ~/.local/share/mise/plugins/node`
    $ mise plugins link node ./mise-node

    # infer plugin name as "node"
    $ mise plugins link ./mise-node
