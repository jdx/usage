# `mise plugins install`

###### Aliases: `i`, `a`, `add`

Install a plugin

note that mise automatically can install plugins when you install a tool
e.g.: `mise install node@20` will autoinstall the node plugin

This behavior can be modified in ~/.config/mise/config.toml

###### Arg `[NEW_PLUGIN]`

The name of the plugin to install
e.g.: node, ruby
Can specify multiple plugins: `mise plugins install node ruby python`

###### Arg `[GIT_URL]`

The git url of the plugin

###### Arg `[REST]...`



##### Flag `-f --force`

Reinstall even if plugin exists

##### Flag `-a --all`

Install all missing plugins
This will only install plugins that have matching shorthands.
i.e.: they don't need the full git repo url

##### Flag `-v --verbose...`

Show installation output

Examples:
    # install the node via shorthand
    $ mise plugins install node

    # install the node plugin using a specific git url
    $ mise plugins install node https://github.com/mise-plugins/rtx-nodejs.git

    # install the node plugin using the git url only
    # (node is inferred from the url)
    $ mise plugins install https://github.com/mise-plugins/rtx-nodejs.git

    # install the node plugin using a specific ref
    $ mise plugins install node https://github.com/mise-plugins/rtx-nodejs.git#v1.0.0
