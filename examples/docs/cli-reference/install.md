# `mise install`

###### Aliases: `i`

Install a tool version

This will install a tool version to `~/.local/share/mise/installs/<PLUGIN>/<VERSION>`
It won't be used simply by being installed, however.
For that, you must set up a `.mise.toml`/`.tool-version` file manually or with `mise use`.
Or you can call a tool version explicitly with `mise exec <TOOL>@<VERSION> -- <COMMAND>`.

Tools will be installed in parallel. To disable, set `--jobs=1` or `MISE_JOBS=1`

###### Arg `[TOOL@VERSION]...`

Tool(s) to install e.g.: node@20

##### Flag `-f --force`

Force reinstall even if already installed

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

##### Flag `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

##### Flag `-v --verbose...`

Show installation output

Examples:

    $ mise install node@20.0.0  # install specific node version
    $ mise install node@20      # install fuzzy node version
    $ mise install node         # install version specified in .tool-versions or .mise.toml
    $ mise install              # installs everything specified in .tool-versions or .mise.toml
