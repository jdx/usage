<!-- [USAGE] load file="../mise.usage.kdl" -->
<!-- [USAGE] title -->

# mise

<!-- [USAGE] -->
<!-- [USAGE] usage_overview -->

## Usage

```bash
Usage: mise [OPTIONS] <COMMAND>
```

<!-- [USAGE] -->
## Global Args
<!-- [USAGE] global_args -->
<!-- [USAGE] -->
## Global Flags
<!-- [USAGE] global_flags -->
- `-C --cd <DIR>`: Change directory before running command
- `-q --quiet`: Suppress non-error messages
- `-v --verbose...`: Show extra output (use -vv for even more)
- `-y --yes`: Answer yes to all confirmation prompts
<!-- [USAGE] -->
## Config
<!-- [USAGE] config -->

<!-- [USAGE] -->
<!-- [USAGE] commands -->

## CLI Command Reference

* [`mise activate`](#mise-activate)
* [`mise alias`](#mise-alias)
* [`mise alias get`](#mise-alias-get)
* [`mise alias ls`](#mise-alias-ls)
* [`mise alias set`](#mise-alias-set)
* [`mise alias unset`](#mise-alias-unset)
* [`mise bin-paths`](#mise-bin-paths)
* [`mise cache`](#mise-cache)
* [`mise cache clear`](#mise-cache-clear)
* [`mise completion`](#mise-completion)
* [`mise config`](#mise-config)
* [`mise config ls`](#mise-config-ls)
* [`mise config generate`](#mise-config-generate)
* [`mise current`](#mise-current)
* [`mise deactivate`](#mise-deactivate)
* [`mise direnv`](#mise-direnv)
* [`mise direnv activate`](#mise-direnv-activate)
* [`mise doctor`](#mise-doctor)
* [`mise env`](#mise-env)
* [`mise exec`](#mise-exec)
* [`mise implode`](#mise-implode)
* [`mise install`](#mise-install)
* [`mise latest`](#mise-latest)
* [`mise link`](#mise-link)
* [`mise ls`](#mise-ls)
* [`mise ls-remote`](#mise-ls-remote)
* [`mise outdated`](#mise-outdated)
* [`mise plugins`](#mise-plugins)
* [`mise plugins install`](#mise-plugins-install)
* [`mise plugins link`](#mise-plugins-link)
* [`mise plugins ls`](#mise-plugins-ls)
* [`mise plugins ls-remote`](#mise-plugins-ls-remote)
* [`mise plugins uninstall`](#mise-plugins-uninstall)
* [`mise plugins update`](#mise-plugins-update)
* [`mise prune`](#mise-prune)
* [`mise reshim`](#mise-reshim)
* [`mise run`](#mise-run)
* [`mise self-update`](#mise-self-update)
* [`mise set`](#mise-set)
* [`mise settings`](#mise-settings)
* [`mise settings get`](#mise-settings-get)
* [`mise settings ls`](#mise-settings-ls)
* [`mise settings set`](#mise-settings-set)
* [`mise settings unset`](#mise-settings-unset)
* [`mise shell`](#mise-shell)
* [`mise sync`](#mise-sync)
* [`mise sync node`](#mise-sync-node)
* [`mise sync python`](#mise-sync-python)
* [`mise tasks`](#mise-tasks)
* [`mise tasks deps`](#mise-tasks-deps)
* [`mise tasks edit`](#mise-tasks-edit)
* [`mise tasks ls`](#mise-tasks-ls)
* [`mise tasks run`](#mise-tasks-run)
* [`mise trust`](#mise-trust)
* [`mise uninstall`](#mise-uninstall)
* [`mise unset`](#mise-unset)
* [`mise upgrade`](#mise-upgrade)
* [`mise usage`](#mise-usage)
* [`mise use`](#mise-use)
* [`mise version`](#mise-version)
* [`mise watch`](#mise-watch)
* [`mise where`](#mise-where)
* [`mise which`](#mise-which)

## `mise activate`

Initializes mise in the current shell session

This should go into your shell's rc file.
Otherwise, it will only take effect in the current session.
(e.g. ~/.zshrc, ~/.bashrc)

This is only intended to be used in interactive sessions, not scripts.
mise is only capable of updating PATH when the prompt is displayed to the user.
For non-interactive use-cases, use shims instead.

Typically this can be added with something like the following:

    echo 'eval "$(mise activate)"' >> ~/.zshrc

However, this requires that "mise" is in your PATH. If it is not, you need to
specify the full path like this:

    echo 'eval "$(/path/to/mise activate)"' >> ~/.zshrc

Customize status output with `status` settings.

###### Arg `[SHELL_TYPE]`

Shell type to generate the script for

##### Flag `-s --shell <SHELL>`

Shell type to generate the script for

##### Flag `--status`

Show "mise: <PLUGIN>@<VERSION>" message when changing directories

##### Flag `--shims`

Use shims instead of modifying PATH
Effectively the same as:
    PATH="$HOME/.local/share/mise/shims:$PATH"

##### Flag `-q --quiet`

Suppress non-error messages

Examples:

    $ eval "$(mise activate bash)"
    $ eval "$(mise activate zsh)"
    $ mise activate fish | source
    $ execx($(mise activate xonsh))

## `mise alias`

###### Aliases: `a`

Manage aliases
### Subcommands

* `get [args]` - Show an alias for a plugin
* `ls [args] [flags]` - List aliases
Shows the aliases that can be specified.
These can come from user config or from plugins in `bin/list-aliases`.
* `set [args]` - Add/update an alias for a plugin
* `unset [args]` - Clears an alias for a plugin

##### Flag `-p --plugin <PLUGIN>`

filter aliases by plugin

##### Flag `--no-header`

Don't show table header

### `mise alias get`

Show an alias for a plugin

This is the contents of an alias.<PLUGIN> entry in ~/.config/mise/config.toml

###### Arg `<PLUGIN>`

(required)The plugin to show the alias for

###### Arg `<ALIAS>`

(required)The alias to show

Examples:
   $ mise alias get node lts-hydrogen
   20.0.0

### `mise alias ls`

###### Aliases: `list`

List aliases
Shows the aliases that can be specified.
These can come from user config or from plugins in `bin/list-aliases`.

For user config, aliases are defined like the following in `~/.config/mise/config.toml`:

  [alias.node]
  lts = "20.0.0"

###### Arg `[PLUGIN]`

Show aliases for <PLUGIN>

##### Flag `--no-header`

Don't show table header

Examples:

    $ mise aliases
    node    lts-hydrogen   20.0.0

### `mise alias set`

###### Aliases: `add`, `create`

Add/update an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml

###### Arg `<PLUGIN>`

(required)The plugin to set the alias for

###### Arg `<ALIAS>`

(required)The alias to set

###### Arg `<VALUE>`

(required)The value to set the alias to

Examples:

    $ mise alias set node lts-hydrogen 18.0.0

### `mise alias unset`

###### Aliases: `rm`, `remove`, `delete`, `del`

Clears an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml

###### Arg `<PLUGIN>`

(required)The plugin to remove the alias from

###### Arg `<ALIAS>`

(required)The alias to remove

Examples:

    $ mise alias unset node lts-hydrogen

## `mise bin-paths`

List all the active runtime bin paths

## `mise cache`

Manage the mise cache

Run `mise cache` with no args to view the current cache directory.
### Subcommands

* `clear [args]` - Deletes all cache files in mise

### `mise cache clear`

###### Aliases: `c`

Deletes all cache files in mise

###### Arg `[PLUGIN]...`

Plugin(s) to clear cache for e.g.: node, python

## `mise completion`

Generate shell completions

###### Arg `[SHELL]`

Shell type to generate completions for

##### Flag `-s --shell <SHELL_TYPE>`

Shell type to generate completions for

Examples:

    $ mise completion bash > /etc/bash_completion.d/mise
    $ mise completion zsh  > /usr/local/share/zsh/site-functions/_mise
    $ mise completion fish > ~/.config/fish/completions/mise.fish

## `mise config`

###### Aliases: `cfg`

[experimental] Manage config files
### Subcommands

* `generate [flags]` - [experimental] Generate an .mise.toml file
* `ls [flags]` - [experimental] List config files currently in use

##### Flag `--no-header`

Do not print table header

### `mise config ls`

[experimental] List config files currently in use

##### Flag `--no-header`

Do not print table header

Examples:

    $ mise config ls

### `mise config generate`

###### Aliases: `g`

[experimental] Generate an .mise.toml file

##### Flag `-o --output <OUTPUT>`

Output to file instead of stdout

Examples:

    $ mise cf generate > .mise.toml
    $ mise cf generate --output=.mise.toml

## `mise current`

Shows current active and installed runtime versions

This is similar to `mise ls --current`, but this only shows the runtime
and/or version. It's designed to fit into scripts more easily.

###### Arg `[PLUGIN]`

Plugin to show versions of e.g.: ruby, node, cargo:eza, npm:prettier, etc

Examples:
    # outputs `.tool-versions` compatible format
    $ mise current
    python 3.11.0 3.10.0
    shfmt 3.6.0
    shellcheck 0.9.0
    node 20.0.0
  
    $ mise current node
    20.0.0
  
    # can output multiple versions
    $ mise current python
    3.11.0 3.10.0

## `mise deactivate`

Disable mise for current shell session

This can be used to temporarily disable mise in a shell session.

Examples:

    $ mise deactivate bash
    $ mise deactivate zsh
    $ mise deactivate fish
    $ execx($(mise deactivate xonsh))

## `mise direnv`

Output direnv function to use mise inside direnv

See https://mise.jdx.dev/direnv.html for more information

Because this generates the legacy files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when legacy file versions change.
### Subcommands

* `activate` - Output direnv function to use mise inside direnv
* `envrc` - [internal] This is an internal command that writes an envrc file
for direnv to consume.
* `exec` - [internal] This is an internal command that writes an envrc file
for direnv to consume.

### `mise direnv activate`

Output direnv function to use mise inside direnv

See https://mise.jdx.dev/direnv.html for more information

Because this generates the legacy files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when legacy file versions change.

Examples:

    $ mise direnv activate > ~/.config/direnv/lib/use_mise.sh
    $ echo 'use mise' > .envrc
    $ direnv allow

## `mise doctor`

###### Aliases: `dr`

Check mise installation for possible problems

Examples:

    $ mise doctor
    [WARN] plugin node is not installed

## `mise env`

###### Aliases: `e`

Exports env vars to activate mise a single time

Use this if you don't want to permanently install mise. It's not necessary to
use this if you have `mise activate` in your shell rc file.

###### Arg `[TOOL@VERSION]...`

Tool(s) to use

##### Flag `-J --json`

Output in JSON format

##### Flag `-s --shell <SHELL>`

Shell type to generate environment variables for

Examples:

    $ eval "$(mise env -s bash)"
    $ eval "$(mise env -s zsh)"
    $ mise env -s fish | source
    $ execx($(mise env -s xonsh))

## `mise exec`

###### Aliases: `x`

Execute a command with tool(s) set

use this to avoid modifying the shell session or running ad-hoc commands with mise tools set.

Tools will be loaded from .mise.toml/.tool-versions, though they can be overridden with <RUNTIME> args
Note that only the plugin specified will be overridden, so if a `.tool-versions` file
includes "node 20" but you run `mise exec python@3.11`; it will still load node@20.

The "--" separates runtimes from the commands to pass along to the subprocess.

###### Arg `[TOOL@VERSION]...`

Tool(s) to start e.g.: node@20 python@3.10

###### Arg `[COMMAND]...`

Command string to execute (same as --command)

##### Flag `-c --command <C>`

Command string to execute

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

##### Flag `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

Examples:

    $ mise exec node@20 -- node ./app.js  # launch app.js using node-20.x
    $ mise x node@20 -- node ./app.js     # shorter alias

    # Specify command as a string:
    $ mise exec node@20 python@3.11 --command "node -v && python -V"

    # Run a command in a different directory:
    $ mise x -C /path/to/project node@20 -- node ./app.js

## `mise implode`

Removes mise CLI and all related data

Skips config directory by default.

##### Flag `--config`

Also remove config directory

##### Flag `-n --dry-run`

List directories that would be removed without actually removing them

## `mise install`

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

## `mise latest`

Gets the latest available version for a plugin

###### Arg `<TOOL@VERSION>`

(required)Tool to get the latest version of

###### Arg `[ASDF_VERSION]`

The version prefix to use when querying the latest version same as the first argument after the "@" used for asdf compatibility

##### Flag `-i --installed`

Show latest installed instead of available version

Examples:

    $ mise latest node@20  # get the latest version of node 20
    20.0.0

    $ mise latest node     # get the latest stable version of node
    20.0.0

## `mise link`

###### Aliases: `ln`

Symlinks a tool version into mise

Use this for adding installs either custom compiled outside
mise or built with a different tool.

###### Arg `<TOOL@VERSION>`

(required)Tool name and version to create a symlink for

###### Arg `<PATH>`

(required)The local path to the tool version
e.g.: ~/.nvm/versions/node/v20.0.0

##### Flag `-f --force`

Overwrite an existing tool version if it exists

Examples:
    # build node-20.0.0 with node-build and link it into mise
    $ node-build 20.0.0 ~/.nodes/20.0.0
    $ mise link node@20.0.0 ~/.nodes/20.0.0

    # have mise use the python version provided by Homebrew
    $ brew install node
    $ mise link node@brew $(brew --prefix node)
    $ mise use node@brew

## `mise ls`

###### Aliases: `list`

List installed and active tool versions

This command lists tools that mise "knows about".
These may be tools that are currently installed, or those
that are in a config file (active) but may or may not be installed.

It's a useful command to get the current state of your tools.

###### Arg `[PLUGIN]...`

Only show tool versions from [PLUGIN]

##### Flag `-p --plugin <PLUGIN_FLAG>`



##### Flag `-c --current`

Only show tool versions currently specified in a .tool-versions/.mise.toml

##### Flag `-g --global`

Only show tool versions currently specified in a the global .tool-versions/.mise.toml

##### Flag `-i --installed`

Only show tool versions that are installed (Hides tools defined in .tool-versions/.mise.toml but not installed)

##### Flag `--parseable`

Output in an easily parseable format

##### Flag `-J --json`

Output in json format

##### Flag `-m --missing`

Display missing tool versions

##### Flag `--prefix <PREFIX>`

Display versions matching this prefix

##### Flag `--no-header`

Don't display headers

Examples:

    $ mise ls
    node    20.0.0 ~/src/myapp/.tool-versions latest
    python  3.11.0 ~/.tool-versions           3.10
    python  3.10.0

    $ mise ls --current
    node    20.0.0 ~/src/myapp/.tool-versions 20
    python  3.11.0 ~/.tool-versions           3.11.0

    $ mise ls --json
    {
      "node": [
        {
          "version": "20.0.0",
          "install_path": "/Users/jdx/.mise/installs/node/20.0.0",
          "source": {
            "type": ".mise.toml",
            "path": "/Users/jdx/.mise.toml"
          }
        }
      ],
      "python": [...]
    }

## `mise ls-remote`

List runtime versions available for install

note that the results are cached for 24 hours
run `mise cache clean` to clear the cache and get fresh results

###### Arg `[TOOL@VERSION]`

Plugin to get versions for

###### Arg `[PREFIX]`

The version prefix to use when querying the latest version
same as the first argument after the "@"

##### Flag `--all`

Show all installed plugins and versions

Examples:

    $ mise ls-remote node
    18.0.0
    20.0.0

    $ mise ls-remote node@20
    20.0.0
    20.1.0

    $ mise ls-remote node 20
    20.0.0
    20.1.0

## `mise outdated`

Shows outdated tool versions

###### Arg `[TOOL@VERSION]...`

Tool(s) to show outdated versions for
e.g.: node@20 python@3.10
If not specified, all tools in global and local configs will be shown

Examples:

    $ mise outdated
    Plugin  Requested  Current  Latest
    python  3.11       3.11.0   3.11.1
    node    20         20.0.0   20.1.0

    $ mise outdated node
    Plugin  Requested  Current  Latest
    node    20         20.0.0   20.1.0

## `mise plugins`

###### Aliases: `p`

Manage plugins
### Subcommands

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

### `mise plugins install`

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

### `mise plugins link`

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

### `mise plugins ls`

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

### `mise plugins ls-remote`

###### Aliases: `list-remote`, `list-all`

List all available remote plugins

The full list is here: https://github.com/jdx/mise/blob/main/src/default_shorthands.rs

Examples:
  $ mise plugins ls-remote

##### Flag `-u --urls`

Show the git url for each plugin e.g.: https://github.com/mise-plugins/rtx-nodejs.git

##### Flag `--only-names`

Only show the name of each plugin by default it will show a "*" next to installed plugins

### `mise plugins uninstall`

###### Aliases: `remove`, `rm`

Removes a plugin

###### Arg `[PLUGIN]...`

Plugin(s) to remove

##### Flag `-p --purge`

Also remove the plugin's installs, downloads, and cache

##### Flag `-a --all`

Remove all plugins

Examples:

    $ mise uninstall node

### `mise plugins update`

###### Aliases: `up`, `upgrade`

Updates a plugin to the latest version

note: this updates the plugin itself, not the runtime versions

###### Arg `[PLUGIN]...`

Plugin(s) to update

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
Default: 4

Examples:

    $ mise plugins update            # update all plugins
    $ mise plugins update node       # update only node
    $ mise plugins update node#beta  # specify a ref

## `mise prune`

Delete unused versions of tools

mise tracks which config files have been used in ~/.local/share/mise/tracked_config_files
Versions which are no longer the latest specified in any of those configs are deleted.
Versions installed only with environment variables (`MISE_<PLUGIN>_VERSION`) will be deleted,
as will versions only referenced on the command line (`mise exec <PLUGIN>@<VERSION>`).

###### Arg `[PLUGIN]...`

Prune only versions from this plugin(s)

##### Flag `-n --dry-run`

Do not actually delete anything

Examples:

    $ mise prune --dry-run
    rm -rf ~/.local/share/mise/versions/node/20.0.0
    rm -rf ~/.local/share/mise/versions/node/20.0.1

## `mise reshim`

rebuilds the shim farm

This creates new shims in ~/.local/share/mise/shims for CLIs that have been added.
mise will try to do this automatically for commands like `npm i -g` but there are
other ways to install things (like using yarn or pnpm for node) that mise does
not know about and so it will be necessary to call this explicitly.

If you think mise should automatically call this for a particular command, please
open an issue on the mise repo. You can also setup a shell function to reshim
automatically (it's really fast so you don't need to worry about overhead):

npm() {
  command npm "$@"
  mise reshim
}

###### Arg `[PLUGIN]`



###### Arg `[VERSION]`



Examples:

    $ mise reshim
    $ ~/.local/share/mise/shims/node -v
    v20.0.0

## `mise run`

###### Aliases: `r`

[experimental] Run a tasks

This command will run a tasks, or multiple tasks in parallel.
Tasks may have dependencies on other tasks or on source files.
If source is configured on a tasks, it will only run if the source
files have changed.

Tasks can be defined in .mise.toml or as standalone scripts.
In .mise.toml, tasks take this form:

    [tasks.build]
    run = "npm run build"
    sources = ["src/**/*.ts"]
    outputs = ["dist/**/*.js"]

Alternatively, tasks can be defined as standalone scripts.
These must be located in the `.mise/tasks` directory.
The name of the script will be the name of the tasks.

    $ cat .mise/tasks/build<<EOF
    #!/usr/bin/env bash
    npm run build
    EOF
    $ mise run build

###### Arg `[TASK]`

Tasks to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2

###### Arg `[ARGS]...`

Arguments to pass to the tasks. Use ":::" to separate tasks

##### Flag `-C --cd <CD>`

Change to this directory before executing the command

##### Flag `-n --dry-run`

Don't actually run the tasks(s), just print them in order of execution

##### Flag `-f --force`

Force the tasks to run even if outputs are up to date

##### Flag `-p --prefix`

Print stdout/stderr by line, prefixed with the tasks's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

##### Flag `-i --interleave`

Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

##### Flag `-t --tool... <TOOL@VERSION>`

Tool(s) to also add e.g.: node@20 python@3.10

##### Flag `-j --jobs <JOBS>`

Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var

##### Flag `-r --raw`

Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

##### Flag `--timings`

Shows elapsed time after each tasks

Examples:

    # Runs the "lint" tasks. This needs to either be defined in .mise.toml
    # or as a standalone script. See the project README for more information.
    $ mise run lint

    # Forces the "build" tasks to run even if its sources are up-to-date.
    $ mise run build --force

    # Run "test" with stdin/stdout/stderr all connected to the current terminal.
    # This forces `--jobs=1` to prevent interleaving of output.
    $ mise run test --raw

    # Runs the "lint", "test", and "check" tasks in parallel.
    $ mise run lint ::: test ::: check

    # Execute multiple tasks each with their own arguments.
    $ mise tasks cmd1 arg1 arg2 ::: cmd2 arg1 arg2

## `mise self-update`

Updates mise itself

Uses the GitHub Releases API to find the latest release and binary
By default, this will also update any installed plugins

###### Arg `[VERSION]`

Update to a specific version

##### Flag `-f --force`

Update even if already up to date

##### Flag `--no-plugins`

Disable auto-updating plugins

##### Flag `-y --yes`

Skip confirmation prompt

## `mise set`

Manage environment variables

By default this command modifies ".mise.toml" in the current directory.

###### Arg `[ENV_VARS]...`

Environment variable(s) to set
e.g.: NODE_ENV=production

##### Flag `--file <FILE>`

The TOML file to update

Defaults to MISE_DEFAULT_CONFIG_FILENAME environment variable, or ".mise.toml".

##### Flag `-g --global`

Set the environment variable in the global config file

##### Flag `--remove... <ENV_VAR>`

Remove the environment variable from config file

Can be used multiple times.

Examples:

    $ mise set NODE_ENV=production

    $ mise set NODE_ENV
    production

    $ mise set
    key       value       source
    NODE_ENV  production  ~/.config/mise/config.toml

## `mise settings`

Manage settings
### Subcommands

* `get [args]` - Show a current setting
* `ls [flags]` - Show current settings
* `set [args]` - Add/update a setting
* `unset [args]` - Clears a setting

##### Flag `--keys`

Only display key names for each setting

### `mise settings get`

Show a current setting

This is the contents of a single entry in ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases get`

###### Arg `<SETTING>`

(required)The setting to show

Examples:

    $ mise settings get legacy_version_file
    true

### `mise settings ls`

###### Aliases: `list`

Show current settings

This is the contents of ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases`

##### Flag `--keys`

Only display key names for each setting

Examples:

    $ mise settings
    legacy_version_file = false

### `mise settings set`

###### Aliases: `add`, `create`

Add/update a setting

This modifies the contents of ~/.config/mise/config.toml

###### Arg `<SETTING>`

(required)The setting to set

###### Arg `<VALUE>`

(required)The value to set

Examples:

    $ mise settings set legacy_version_file true

### `mise settings unset`

###### Aliases: `rm`, `remove`, `delete`, `del`

Clears a setting

This modifies the contents of ~/.config/mise/config.toml

###### Arg `<SETTING>`

(required)The setting to remove

Examples:

    $ mise settings unset legacy_version_file

## `mise shell`

###### Aliases: `sh`

Sets a tool version for the current session

Only works in a session where mise is already activated.

This works by setting environment variables for the current shell session
such as `MISE_NODE_VERSION=20` which is "eval"ed as a shell function created
by `mise activate`.

###### Arg `[TOOL@VERSION]...`

Tool(s) to use

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

##### Flag `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

##### Flag `-u --unset`

Removes a previously set version

Examples:

    $ mise shell node@20
    $ node -v
    v20.0.0

## `mise sync`

Add tool versions from external tools to mise
### Subcommands

* `node [flags]` - Symlinks all tool versions from an external tool into mise
* `python [flags]` - Symlinks all tool versions from an external tool into mise

### `mise sync node`

Symlinks all tool versions from an external tool into mise

For example, use this to import all Homebrew node installs into mise

##### Flag `--brew`

Get tool versions from Homebrew

##### Flag `--nvm`

Get tool versions from nvm

##### Flag `--nodenv`

Get tool versions from nodenv

Examples:

    $ brew install node@18 node@20
    $ mise sync node --brew
    $ mise use -g node@18 - uses Homebrew-provided node

### `mise sync python`

Symlinks all tool versions from an external tool into mise

For example, use this to import all pyenv installs into mise

##### Flag `--pyenv`

Get tool versions from pyenv

Examples:

    $ pyenv install 3.11.0
    $ mise sync python --pyenv
    $ mise use -g python@3.11.0 - uses pyenv-provided python

## `mise tasks`

###### Aliases: `t`

[experimental] Manage tasks
### Subcommands

* `deps [args] [flags]` - [experimental] Display a tree visualization of a dependency graph
* `edit [args] [flags]` - [experimental] Edit a tasks with $EDITOR
* `ls [flags]` - [experimental] List available tasks to execute
These may be included from the config file or from the project's .mise/tasks directory
mise will merge all tasks from all parent directories into this list.
* `run [args] [flags]` - [experimental] Run a tasks

##### Flag `--no-header`

Do not print table header

##### Flag `--hidden`

Show hidden tasks

Examples:
    
    $ mise tasks ls

### `mise tasks deps`

[experimental] Display a tree visualization of a dependency graph

###### Arg `[TASKS]...`

Tasks to show dependencies for
Can specify multiple tasks by separating with spaces
e.g.: mise tasks deps lint test check

##### Flag `--dot`

Display dependencies in DOT format

Examples:

    # Show dependencies for all tasks
    $ mise tasks deps

    # Show dependencies for the "lint", "test" and "check" tasks
    $ mise tasks deps lint test check

    # Show dependencies in DOT format
    $ mise tasks deps --dot

### `mise tasks edit`

[experimental] Edit a tasks with $EDITOR

The tasks will be created as a standalone script if it does not already exist.

###### Arg `<TASK>`

(required)Tasks to edit

##### Flag `-p --path`

Display the path to the tasks instead of editing it

Examples:

    $ mise tasks edit build
    $ mise tasks edit test

### `mise tasks ls`

[experimental] List available tasks to execute
These may be included from the config file or from the project's .mise/tasks directory
mise will merge all tasks from all parent directories into this list.

So if you have global tasks in ~/.config/mise/tasks/* and project-specific tasks in
~/myproject/.mise/tasks/*, then they'll both be available but the project-specific
tasks will override the global ones if they have the same name.

##### Flag `--no-header`

Do not print table header

##### Flag `--hidden`

Show hidden tasks

Examples:
    
    $ mise tasks ls

### `mise tasks run`

###### Aliases: `r`

[experimental] Run a tasks

This command will run a tasks, or multiple tasks in parallel.
Tasks may have dependencies on other tasks or on source files.
If source is configured on a tasks, it will only run if the source
files have changed.

Tasks can be defined in .mise.toml or as standalone scripts.
In .mise.toml, tasks take this form:

    [tasks.build]
    run = "npm run build"
    sources = ["src/**/*.ts"]
    outputs = ["dist/**/*.js"]

Alternatively, tasks can be defined as standalone scripts.
These must be located in the `.mise/tasks` directory.
The name of the script will be the name of the tasks.

    $ cat .mise/tasks/build<<EOF
    #!/usr/bin/env bash
    npm run build
    EOF
    $ mise run build

###### Arg `[TASK]`

Tasks to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2

###### Arg `[ARGS]...`

Arguments to pass to the tasks. Use ":::" to separate tasks

##### Flag `-C --cd <CD>`

Change to this directory before executing the command

##### Flag `-n --dry-run`

Don't actually run the tasks(s), just print them in order of execution

##### Flag `-f --force`

Force the tasks to run even if outputs are up to date

##### Flag `-p --prefix`

Print stdout/stderr by line, prefixed with the tasks's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

##### Flag `-i --interleave`

Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

##### Flag `-t --tool... <TOOL@VERSION>`

Tool(s) to also add e.g.: node@20 python@3.10

##### Flag `-j --jobs <JOBS>`

Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var

##### Flag `-r --raw`

Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

##### Flag `--timings`

Shows elapsed time after each tasks

Examples:

    # Runs the "lint" tasks. This needs to either be defined in .mise.toml
    # or as a standalone script. See the project README for more information.
    $ mise run lint

    # Forces the "build" tasks to run even if its sources are up-to-date.
    $ mise run build --force

    # Run "test" with stdin/stdout/stderr all connected to the current terminal.
    # This forces `--jobs=1` to prevent interleaving of output.
    $ mise run test --raw

    # Runs the "lint", "test", and "check" tasks in parallel.
    $ mise run lint ::: test ::: check

    # Execute multiple tasks each with their own arguments.
    $ mise tasks cmd1 arg1 arg2 ::: cmd2 arg1 arg2

## `mise trust`

Marks a config file as trusted

This means mise will parse the file with potentially dangerous
features enabled.

This includes:
- environment variables
- templates
- `path:` plugin versions

###### Arg `[CONFIG_FILE]`

The config file to trust

##### Flag `-a --all`

Trust all config files in the current directory and its parents

##### Flag `--untrust`

No longer trust this config

Examples:
    # trusts ~/some_dir/.mise.toml
    $ mise trust ~/some_dir/.mise.toml

    # trusts .mise.toml in the current or parent directory
    $ mise trust

## `mise uninstall`

###### Aliases: `remove`, `rm`

Removes runtime versions

###### Arg `[TOOL@VERSION]...`

Tool(s) to remove

##### Flag `-a --all`

Delete all installed versions

##### Flag `-n --dry-run`

Do not actually delete anything

Examples:
    
    $ mise uninstall node@18.0.0 # will uninstall specific version
    $ mise uninstall node        # will uninstall current node version
    $ mise uninstall --all node@18.0.0 # will uninstall all node versions

## `mise unset`

Remove environment variable(s) from the config file

By default this command modifies ".mise.toml" in the current directory.

###### Arg `[KEYS]...`

Environment variable(s) to remove
e.g.: NODE_ENV

##### Flag `-f --file <FILE>`

Specify a file to use instead of ".mise.toml"

##### Flag `-g --global`

Use the global config file

## `mise upgrade`

###### Aliases: `up`

Upgrades outdated tool versions

###### Arg `[TOOL@VERSION]...`

Tool(s) to upgrade
e.g.: node@20 python@3.10
If not specified, all current tools will be upgraded

##### Flag `-n --dry-run`

Just print what would be done, don't actually do it

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

##### Flag `-i --interactive`

Display multiselect menu to choose which tools to upgrade

##### Flag `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

## `mise usage`

Generate usage spec

## `mise use`

###### Aliases: `u`

Install tool version and add it to config

This will install the tool if it is not already installed.
By default, this will use an `.mise.toml` file in the current directory.
Use the --global flag to use the global config file instead.
This replaces asdf's `local` and `global` commands, however those are still available in mise.

###### Arg `[TOOL@VERSION]...`

Tool(s) to add to config file
e.g.: node@20, cargo:ripgrep@latest npm:prettier@3
If no version is specified, it will default to @latest

##### Flag `-f --force`

Force reinstall even if already installed

##### Flag `--fuzzy`

Save fuzzy version to config file
e.g.: `mise use --fuzzy node@20` will save 20 as the version
this is the default behavior unless MISE_ASDF_COMPAT=1

##### Flag `-g --global`

Use the global config file (~/.config/mise/config.toml) instead of the local one

##### Flag `-e --env <ENV>`

Modify an environment-specific config file like .mise.<env>.toml

##### Flag `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

##### Flag `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

##### Flag `--remove... <PLUGIN>`

Remove the plugin(s) from config file

##### Flag `-p --path <PATH>`

Specify a path to a config file or directory If a directory is specified, it will look for .mise.toml (default) or .tool-versions

##### Flag `--pin`

Save exact version to config file
e.g.: `mise use --pin node@20` will save 20.0.0 as the version
Set MISE_ASDF_COMPAT=1 to make this the default behavior

Examples:

    # set the current version of node to 20.x in .mise.toml of current directory
    # will write the fuzzy version (e.g.: 20)
    $ mise use node@20

    # set the current version of node to 20.x in ~/.config/mise/config.toml
    # will write the precise version (e.g.: 20.0.0)
    $ mise use -g --pin node@20

    # sets .mise.local.toml (which is intended not to be committed to a project)
    $ mise use --env local node@20

    # sets .mise.staging.toml (which is used if MISE_ENV=staging)
    $ mise use --env staging node@20

## `mise version`

Show mise version

## `mise watch`

###### Aliases: `w`

[experimental] Run a tasks watching for changes

###### Arg `[ARGS]...`

Extra arguments

##### Flag `-t --task... <TASK>`

Tasks to run

##### Flag `-g --glob... <GLOB>`

Files to watch
Defaults to sources from the tasks(s)

Examples:
    $ mise watch -t build
    Runs the "build" tasks. Will re-run the tasks when any of its sources change.
    Uses "sources" from the tasks definition to determine which files to watch.

    $ mise watch -t build --glob src/**/*.rs
    Runs the "build" tasks but specify the files to watch with a glob pattern.
    This overrides the "sources" from the tasks definition.

    $ mise run -t build --clear
    Extra arguments are passed to watchexec. See `watchexec --help` for details.

## `mise where`

Display the installation path for a runtime

Must be installed.

###### Arg `<TOOL@VERSION>`

(required)Tool(s) to look up
e.g.: ruby@3
if "@<PREFIX>" is specified, it will show the latest installed version
that matches the prefix
otherwise, it will show the current, active installed version

###### Arg `[ASDF_VERSION]`

the version prefix to use when querying the latest version
same as the first argument after the "@"
used for asdf compatibility

Examples:
    # Show the latest installed version of node
    # If it is is not installed, errors
    $ mise where node@20
    /home/jdx/.local/share/mise/installs/node/20.0.0

    # Show the current, active install directory of node
    # Errors if node is not referenced in any .tool-version file
    $ mise where node
    /home/jdx/.local/share/mise/installs/node/20.0.0

## `mise which`

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

<!-- [USAGE] -->
