# `mise which`

Shows the path that a bin name points to

###### Arg `<BIN_NAME>`

(required)The bin name to look up

##### Flag `--plugin`

Show the plugin name instead of the path

##### Flag `--version`

Show the version instead of the path

##### Flag `-t --tool <TOOL@VERSION>`

Use a specific tool@version
e.g.: `mise which npm --tool=node@20`

Examples:

    $ mise which node
    /home/username/.local/share/mise/installs/node/20.0.0/bin/node
    $ mise which node --plugin
    node
    $ mise which node --version
    20.0.0
