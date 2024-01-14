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
- `-C,--cd <DIR>`: Change directory before running command
- `-q,--quiet`: Suppress non-error messages
- `-v,--verbose`: Show extra output (use -vv for even more)
- `-y,--yes`: Answer yes to all confirmation prompts
<!-- [USAGE] -->
## Config
<!-- [USAGE] config -->
### `activate_accessive`

* env: `MISE_ACTIVATE_ACCESSIVE`
* default: `false`

foooooooo

### `color`

* env: `MISE_COLOR`
* default: `true`

### `jobs`

* default: `4`

### `timeout`

* default: `1.5`

### `user`

* default: `"admin"`

<!-- [USAGE] -->
<!-- [USAGE] commands -->

## CLI Command Reference

* [`activate`](#activate)
* [`alias`](#alias)
* [`alias get`](#alias-get)
* [`alias ls`](#alias-ls)
* [`alias set`](#alias-set)
* [`alias unset`](#alias-unset)
* [`bin-paths`](#bin-paths)
* [`cache`](#cache)
* [`cache clear`](#cache-clear)
* [`completion`](#completion)
* [`config`](#config)
* [`config ls`](#config-ls)
* [`config generate`](#config-generate)
* [`current`](#current)
* [`deactivate`](#deactivate)
* [`direnv`](#direnv)
* [`direnv activate`](#direnv-activate)
* [`doctor`](#doctor)
* [`env`](#env)
* [`exec`](#exec)
* [`implode`](#implode)
* [`install`](#install)
* [`latest`](#latest)
* [`link`](#link)
* [`ls`](#ls)
* [`ls-remote`](#ls-remote)
* [`outdated`](#outdated)
* [`plugins`](#plugins)
* [`plugins install`](#plugins-install)
* [`plugins link`](#plugins-link)
* [`plugins ls`](#plugins-ls)
* [`plugins ls-remote`](#plugins-ls-remote)
* [`plugins uninstall`](#plugins-uninstall)
* [`plugins update`](#plugins-update)
* [`prune`](#prune)
* [`reshim`](#reshim)
* [`run`](#run)
* [`self-update`](#self-update)
* [`set`](#set)
* [`settings`](#settings)
* [`settings get`](#settings-get)
* [`settings ls`](#settings-ls)
* [`settings set`](#settings-set)
* [`settings unset`](#settings-unset)
* [`shell`](#shell)
* [`sync`](#sync)
* [`sync node`](#sync-node)
* [`sync python`](#sync-python)
* [`task`](#task)
* [`task deps`](#task-deps)
* [`task edit`](#task-edit)
* [`task ls`](#task-ls)
* [`task run`](#task-run)
* [`trust`](#trust)
* [`uninstall`](#uninstall)
* [`unset`](#unset)
* [`upgrade`](#upgrade)
* [`usage`](#usage)
* [`use`](#use)
* [`version`](#version)
* [`watch`](#watch)
* [`where`](#where)
* [`which`](#which)
* [`zzz`](#zzz)

### `activate`

**Args:**

* `[SHELL_TYPE]` – Shell type to generate the script for

**Flags:**

* `-s,--shell <SHELL>` – Shell type to generate the script for
* `--status` – Show "mise: <PLUGIN>@<VERSION>" message when changing directories
* `-q,--quiet` – Suppress non-error messages

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
Examples:
  $ eval "$(mise activate bash)"
  $ eval "$(mise activate zsh)"
  $ mise activate fish | source
  $ execx($(mise activate xonsh))

### `alias`

* Aliases: `a`

**Flags:**

* `-p,--plugin <PLUGIN>` – filter aliases by plugin
* `--no-header` – Don't show table header

Manage aliases

#### `alias get`

**Args:**

* `<PLUGIN>` – The plugin to show the alias for
* `<ALIAS>` – The alias to show

Show an alias for a plugin

This is the contents of an alias.<PLUGIN> entry in ~/.config/mise/config.toml
Examples:
 $ mise alias get node lts-hydrogen
 20.0.0

#### `alias ls`

* Aliases: `list`

**Args:**

* `[PLUGIN]` – Show aliases for <PLUGIN>

**Flags:**

* `--no-header` – Don't show table header

List aliases
Shows the aliases that can be specified.
These can come from user config or from plugins in `bin/list-aliases`.

For user config, aliases are defined like the following in `~/.config/mise/config.toml`:

  [alias.node]
  lts = "20.0.0"
Examples:
  $ mise aliases
  node    lts-hydrogen   20.0.0

#### `alias set`

* Aliases: `add`, `create`

**Args:**

* `<PLUGIN>` – The plugin to set the alias for
* `<ALIAS>` – The alias to set
* `<VALUE>` – The value to set the alias to

Add/update an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml
Examples:
  $ mise alias set node lts-hydrogen 18.0.0

#### `alias unset`

* Aliases: `rm`, `remove`, `delete`, `del`

**Args:**

* `<PLUGIN>` – The plugin to remove the alias from
* `<ALIAS>` – The alias to remove

Clears an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml
Examples:
  $ mise alias unset node lts-hydrogen

### `bin-paths`

List all the active runtime bin paths

### `cache`

Manage the mise cache

Run `mise cache` with no args to view the current cache directory.

#### `cache clear`

* Aliases: `c`

**Args:**

* `[PLUGIN]...` – Plugin(s) to clear cache for e.g.: node, python

Deletes all cache files in mise

### `completion`

**Args:**

* `[SHELL]` – Shell type to generate completions for

**Flags:**

* `-s,--shell <SHELL_TYPE>` – Shell type to generate completions for

Generate shell completions
Examples:
  $ mise completion bash > /etc/bash_completion.d/mise
  $ mise completion zsh  > /usr/local/share/zsh/site-functions/_mise
  $ mise completion fish > ~/.config/fish/completions/mise.fish

### `config`

* Aliases: `cfg`

**Flags:**

* `--no-header` – Do not print table header

[experimental] Manage config files

#### `config ls`

**Flags:**

* `--no-header` – Do not print table header

[experimental] List config files currently in use
Examples:
  $ mise config ls

#### `config generate`

* Aliases: `g`

**Flags:**

* `-o,--output <OUTPUT>` – Output to file instead of stdout

[experimental] Generate an .mise.toml file
Examples:
  $ mise cf generate > .mise.toml
  $ mise cf generate --output=.mise.toml

### `current`

**Args:**

* `[PLUGIN]` – Plugin to show versions of e.g.: ruby, node, cargo:eza, npm:prettier, etc

Shows current active and installed runtime versions

This is similar to `mise ls --current`, but this only shows the runtime
and/or version. It's designed to fit into scripts more easily.
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

### `deactivate`

Disable mise for current shell session

This can be used to temporarily disable mise in a shell session.
Examples:
  $ mise deactivate bash
  $ mise deactivate zsh
  $ mise deactivate fish
  $ execx($(mise deactivate xonsh))

### `direnv`

Output direnv function to use mise inside direnv

See https://mise.rtx.dev/direnv.html for more information

Because this generates the legacy files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when legacy file versions change.

#### `direnv activate`

Output direnv function to use mise inside direnv

See https://mise.jdx.dev/direnv.html for more information

Because this generates the legacy files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when legacy file versions change.
Examples:
  $ mise direnv activate > ~/.config/direnv/lib/use_mise.sh
  $ echo 'use mise' > .envrc
  $ direnv allow

### `doctor`

Check mise installation for possible problems.
Examples:
  $ mise doctor
  [WARN] plugin node is not installed

### `env`

* Aliases: `e`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to use

**Flags:**

* `-s,--shell <SHELL>` – Shell type to generate environment variables for
* `-J,--json` – Output in JSON format

Exports env vars to activate mise a single time

Use this if you don't want to permanently install mise. It's not necessary to
use this if you have `mise activate` in your shell rc file.
Examples:
  $ eval "$(mise env -s bash)"
  $ eval "$(mise env -s zsh)"
  $ mise env -s fish | source
  $ execx($(mise env -s xonsh))

### `exec`

* Aliases: `x`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to start e.g.: node@20 python@3.10
* `[COMMAND]...` – Command string to execute (same as --command)

**Flags:**

* `-c,--command <C>` – Command string to execute
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

Execute a command with tool(s) set

use this to avoid modifying the shell session or running ad-hoc commands with mise tools set.

Tools will be loaded from .mise.toml/.tool-versions, though they can be overridden with <RUNTIME> args
Note that only the plugin specified will be overridden, so if a `.tool-versions` file
includes "node 20" but you run `mise exec python@3.11`; it will still load node@20.

The "--" separates runtimes from the commands to pass along to the subprocess.
Examples:
  $ mise exec node@20 -- node ./app.js  # launch app.js using node-20.x
  $ mise x node@20 -- node ./app.js     # shorter alias

  # Specify command as a string:
  $ mise exec node@20 python@3.11 --command "node -v && python -V"

  # Run a command in a different directory:
  $ mise x -C /path/to/project node@20 -- node ./app.js

### `implode`

**Flags:**

* `--config` – Also remove config directory
* `-n,--dry-run` – List directories that would be removed without actually removing them

Removes mise CLI and all related data

Skips config directory by default.

### `install`

* Aliases: `i`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to install e.g.: node@20

**Flags:**

* `-f,--force` – Force reinstall even if already installed
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
* `-v,--verbose` – Show installation output

Install a tool version

This will install a tool version to `~/.local/share/mise/installs/<PLUGIN>/<VERSION>`
It won't be used simply by being installed, however.
For that, you must set up a `.mise.toml`/`.tool-version` file manually or with `mise use`.
Or you can call a tool version explicitly with `mise exec <TOOL>@<VERSION> -- <COMMAND>`.

Tools will be installed in parallel. To disable, set `--jobs=1` or `MISE_JOBS=1`
Examples:
  $ mise install node@20.0.0  # install specific node version
  $ mise install node@20      # install fuzzy node version
  $ mise install node         # install version specified in .tool-versions or .mise.toml
  $ mise install              # installs everything specified in .tool-versions or .mise.toml

### `latest`

**Args:**

* `<TOOL@VERSION>` – Tool to get the latest version of
* `[ASDF_VERSION]` – The version prefix to use when querying the latest version same as the first argument after the "@" used for asdf compatibility

**Flags:**

* `-i,--installed` – Show latest installed instead of available version

Gets the latest available version for a plugin
Examples:
  $ mise latest node@20  # get the latest version of node 20
  20.0.0

  $ mise latest node     # get the latest stable version of node
  20.0.0

### `link`

* Aliases: `ln`

**Args:**

* `<TOOL@VERSION>` – Tool name and version to create a symlink for
* `<PATH>` – The local path to the tool version
e.g.: ~/.nvm/versions/node/v20.0.0

**Flags:**

* `-f,--force` – Overwrite an existing tool version if it exists

Symlinks a tool version into mise

Use this for adding installs either custom compiled outside
mise or built with a different tool.
Examples:
  # build node-20.0.0 with node-build and link it into mise
  $ node-build 20.0.0 ~/.nodes/20.0.0
  $ mise link node@20.0.0 ~/.nodes/20.0.0

  # have mise use the python version provided by Homebrew
  $ brew install node
  $ mise link node@brew $(brew --prefix node)
  $ mise use node@brew

### `ls`

* Aliases: `list`

**Args:**

* `[PLUGIN]...` – Only show tool versions from [PLUGIN]

**Flags:**

* `-p,--plugin <PLUGIN_FLAG>` – 
* `-c,--current` – Only show tool versions currently specified in a .tool-versions/.mise.toml
* `-g,--global` – Only show tool versions currently specified in a the global .tool-versions/.mise.toml
* `-i,--installed` – Only show tool versions that are installed Hides missing ones defined in .tool-versions/.mise.toml but not yet installed
* `--parseable` – Output in an easily parseable format
* `-J,--json` – Output in json format
* `-m,--missing` – Display missing tool versions
* `--prefix <PREFIX>` – Display versions matching this prefix
* `--no-header` – Don't display headers

List installed and/or currently selected tool versions
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

### `ls-remote`

**Args:**

* `[TOOL@VERSION]` – Plugin to get versions for
* `[PREFIX]` – The version prefix to use when querying the latest version
same as the first argument after the "@"

**Flags:**

* `--all` – Show all installed plugins and versions

List runtime versions available for install

note that the results are cached for 24 hours
run `mise cache clean` to clear the cache and get fresh results
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

### `outdated`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to show outdated versions for
e.g.: node@20 python@3.10
If not specified, all tools in global and local configs will be shown

Shows outdated tool versions
Examples:
  $ mise outdated
  Plugin  Requested  Current  Latest
  python  3.11       3.11.0   3.11.1
  node    20         20.0.0   20.1.0

  $ mise outdated node
  Plugin  Requested  Current  Latest
  node    20         20.0.0   20.1.0

### `plugins`

* Aliases: `p`

**Flags:**

* `-a,--all` – list all available remote plugins

same as `mise plugins ls-remote`
* `-c,--core` – The built-in plugins only
Normally these are not shown
* `--user` – List installed plugins

This is the default behavior but can be used with --core
to show core and user plugins
* `-u,--urls` – Show the git url for each plugin
e.g.: https://github.com/asdf-vm/asdf-node.git
* `--refs` – Show the git refs for each plugin
e.g.: main 1234abc

Manage plugins

#### `plugins install`

* Aliases: `i`, `a`, `add`

**Args:**

* `[NEW_PLUGIN]` – The name of the plugin to install
e.g.: node, ruby
Can specify multiple plugins: `mise plugins install node ruby python`
* `[GIT_URL]` – The git url of the plugin
* `[REST]...` – 

**Flags:**

* `-f,--force` – Reinstall even if plugin exists
* `-a,--all` – Install all missing plugins
This will only install plugins that have matching shorthands.
i.e.: they don't need the full git repo url
* `-v,--verbose` – Show installation output

Install a plugin

note that mise automatically can install plugins when you install a tool
e.g.: `mise install node@20` will autoinstall the node plugin

This behavior can be modified in ~/.config/mise/config.toml
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

#### `plugins link`

* Aliases: `ln`

**Args:**

* `<NAME>` – The name of the plugin
e.g.: node, ruby
* `[PATH]` – The local path to the plugin
e.g.: ./mise-node

**Flags:**

* `-f,--force` – Overwrite existing plugin

Symlinks a plugin into mise

This is used for developing a plugin.
Examples:
  # essentially just `ln -s ./mise-node ~/.local/share/mise/plugins/node`
  $ mise plugins link node ./mise-node

  # infer plugin name as "node"
  $ mise plugins link ./mise-node

#### `plugins ls`

* Aliases: `list`

**Flags:**

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

#### `plugins ls-remote`

* Aliases: `list-remote`, `list-all`

**Flags:**

* `-u,--urls` – Show the git url for each plugin e.g.: https://github.com/mise-plugins/rtx-nodejs.git
* `--only-names` – Only show the name of each plugin by default it will show a "*" next to installed plugins

List all available remote plugins

The full list is here: https://github.com/jdx/mise/blob/main/src/default_shorthands.rs

Examples:
  $ mise plugins ls-remote

#### `plugins uninstall`

* Aliases: `remove`, `rm`

**Args:**

* `[PLUGIN]...` – Plugin(s) to remove

**Flags:**

* `-p,--purge` – Also remove the plugin's installs, downloads, and cache
* `-a,--all` – Remove all plugins

Removes a plugin
Examples:
  $ mise uninstall node

#### `plugins update`

* Aliases: `upgrade`

**Args:**

* `[PLUGIN]...` – Plugin(s) to update

**Flags:**

* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
Default: 4

Updates a plugin to the latest version

note: this updates the plugin itself, not the runtime versions
Examples:
  $ mise plugins update            # update all plugins
  $ mise plugins update node       # update only node
  $ mise plugins update node#beta  # specify a ref

### `prune`

**Args:**

* `[PLUGIN]...` – Prune only versions from this plugin(s)

**Flags:**

* `-n,--dry-run` – Do not actually delete anything

Delete unused versions of tools

mise tracks which config files have been used in ~/.local/share/mise/tracked_config_files
Versions which are no longer the latest specified in any of those configs are deleted.
Versions installed only with environment variables (`MISE_<PLUGIN>_VERSION`) will be deleted,
as will versions only referenced on the command line (`mise exec <PLUGIN>@<VERSION>`).
Examples:
  $ mise prune --dry-run
  rm -rf ~/.local/share/mise/versions/node/20.0.0
  rm -rf ~/.local/share/mise/versions/node/20.0.1

### `reshim`

**Args:**

* `[PLUGIN]` – 
* `[VERSION]` – 

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
Examples:
  $ mise reshim
  $ ~/.local/share/mise/shims/node -v
  v20.0.0

### `run`

* Aliases: `r`

**Args:**

* `[TASK]` – Task to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2
* `[ARGS]...` – Arguments to pass to the task. Use ":::" to separate tasks

**Flags:**

* `-C,--cd <CD>` – Change to this directory before executing the command
* `-n,--dry-run` – Don't actually run the task(s), just print them in order of execution
* `-f,--force` – Force the task to run even if outputs are up to date
* `-p,--prefix` – Print stdout/stderr by line, prefixed with the task's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var
* `-i,--interleave` – Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var
* `-t,--tool <TOOL@VERSION>` – Tool(s) to also add e.g.: node@20 python@3.10
* `-j,--jobs <JOBS>` – Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var
* `-r,--raw` – Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

[experimental] Run a task

This command will run a task, or multiple tasks in parallel.
Tasks may have dependencies on other tasks or on source files.
If source is configured on a task, it will only run if the source
files have changed.

Tasks can be defined in .mise.toml or as standalone scripts.
In .mise.toml, tasks take this form:

    [tasks.build]
    run = "npm run build"
    sources = ["src/**/*.ts"]
    outputs = ["dist/**/*.js"]

Alternatively, tasks can be defined as standalone scripts.
These must be located in the `.mise/tasks` directory.
The name of the script will be the name of the task.

    $ cat .mise/tasks/build<<EOF
    #!/usr/bin/env bash
    npm run build
    EOF
    $ mise run build
Examples:
  $ mise run lint
  Runs the "lint" task. This needs to either be defined in .mise.toml
  or as a standalone script. See the project README for more information.

  $ mise run build --force
  Forces the "build" task to run even if its sources are up-to-date.

  $ mise run test --raw
  Runs "test" with stdin/stdout/stderr all connected to the current terminal.
  This forces `--jobs=1` to prevent interleaving of output.

  $ mise run lint ::: test ::: check
  Runs the "lint", "test", and "check" tasks in parallel.

  $ mise task cmd1 arg1 arg2 ::: cmd2 arg1 arg2
  Execute multiple tasks each with their own arguments.

### `self-update`

**Args:**

* `[VERSION]` – Update to a specific version

**Flags:**

* `-f,--force` – Update even if already up to date
* `--no-plugins` – Disable auto-updating plugins
* `-y,--yes` – Skip confirmation prompt

Updates mise itself

Uses the GitHub Releases API to find the latest release and binary
By default, this will also update any installed plugins

### `set`

**Args:**

* `[ENV_VARS]...` – Environment variable(s) to set
e.g.: NODE_ENV=production

**Flags:**

* `--file <FILE>` – The TOML file to update

Defaults to MISE_DEFAULT_CONFIG_FILENAME environment variable, or ".mise.toml".
* `-g,--global` – Set the environment variable in the global config file
* `--remove <ENV_VAR>` – Remove the environment variable from config file

Can be used multiple times.

Manage environment variables

By default this command modifies ".mise.toml" in the current directory.
Examples:
  $ mise set NODE_ENV=production

  $ mise set NODE_ENV
  production

  $ mise set
  key       value       source
  NODE_ENV  production  ~/.config/mise/config.toml

### `settings`

Manage settings

#### `settings get`

**Args:**

* `<SETTING>` – The setting to show

Show a current setting

This is the contents of a single entry in ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases get`
Examples:
  $ mise settings get legacy_version_file
  true

#### `settings ls`

* Aliases: `list`

Show current settings

This is the contents of ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases`
Examples:
  $ mise settings
  legacy_version_file = false

#### `settings set`

* Aliases: `add`, `create`

**Args:**

* `<SETTING>` – The setting to set
* `<VALUE>` – The value to set

Add/update a setting

This modifies the contents of ~/.config/mise/config.toml
Examples:
  $ mise settings set legacy_version_file true

#### `settings unset`

* Aliases: `rm`, `remove`, `delete`, `del`

**Args:**

* `<SETTING>` – The setting to remove

Clears a setting

This modifies the contents of ~/.config/mise/config.toml
Examples:
  $ mise settings unset legacy_version_file

### `shell`

* Aliases: `sh`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to use

**Flags:**

* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
* `-u,--unset` – Removes a previously set version

Sets a tool version for the current shell session

Only works in a session where mise is already activated.
Examples:
  $ mise shell node@20
  $ node -v
  v20.0.0

### `sync`

Add tool versions from external tools to mise

#### `sync node`

**Flags:**

* `--brew` – Get tool versions from Homebrew
* `--nvm` – Get tool versions from nvm
* `--nodenv` – Get tool versions from nodenv

Symlinks all tool versions from an external tool into mise

For example, use this to import all Homebrew node installs into mise
Examples:
  $ brew install node@18 node@20
  $ mise sync node --brew
  $ mise use -g node@18 - uses Homebrew-provided node

#### `sync python`

**Flags:**

* `--pyenv` – Get tool versions from pyenv

Symlinks all tool versions from an external tool into mise

For example, use this to import all pyenv installs into mise
Examples:
  $ pyenv install 3.11.0
  $ mise sync python --pyenv
  $ mise use -g python@3.11.0 - uses pyenv-provided python

### `task`

* Aliases: `t`

**Flags:**

* `--no-header` – Do not print table header
* `--hidden` – Show hidden tasks

[experimental] Manage tasks
Examples:
  $ mise task ls

#### `task deps`

**Args:**

* `[TASKS]...` – Tasks to show dependencies for
Can specify multiple tasks by separating with spaces
e.g.: mise task deps lint test check

**Flags:**

* `--dot` – Display dependencies in DOT format

[experimental] Display a tree visualization of a dependency graph
Examples:
  $ mise task deps
  Shows dependencies for all tasks

  $ mise task deps lint test check
  Shows dependencies for the "lint", "test" and "check" tasks

  $ mise task deps --dot
  Shows dependencies in DOT format

#### `task edit`

**Args:**

* `<TASK>` – Task to edit

**Flags:**

* `-p,--path` – Display the path to the task instead of editing it

[experimental] Edit a task with $EDITOR

The task will be created as a standalone script if it does not already exist.
Examples:
  $ mise task edit build
  $ mise task edit test

#### `task ls`

**Flags:**

* `--no-header` – Do not print table header
* `--hidden` – Show hidden tasks

[experimental] List available tasks to execute
These may be included from the config file or from the project's .mise/tasks directory
mise will merge all tasks from all parent directories into this list.

So if you have global tasks in ~/.config/mise/tasks/* and project-specific tasks in
~/myproject/.mise/tasks/*, then they'll both be available but the project-specific
tasks will override the global ones if they have the same name.
Examples:
  $ mise task ls

#### `task run`

* Aliases: `r`

**Args:**

* `[TASK]` – Task to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2
* `[ARGS]...` – Arguments to pass to the task. Use ":::" to separate tasks

**Flags:**

* `-C,--cd <CD>` – Change to this directory before executing the command
* `-n,--dry-run` – Don't actually run the task(s), just print them in order of execution
* `-f,--force` – Force the task to run even if outputs are up to date
* `-p,--prefix` – Print stdout/stderr by line, prefixed with the task's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var
* `-i,--interleave` – Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var
* `-t,--tool <TOOL@VERSION>` – Tool(s) to also add e.g.: node@20 python@3.10
* `-j,--jobs <JOBS>` – Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var
* `-r,--raw` – Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

[experimental] Run a task

This command will run a task, or multiple tasks in parallel.
Tasks may have dependencies on other tasks or on source files.
If source is configured on a task, it will only run if the source
files have changed.

Tasks can be defined in .mise.toml or as standalone scripts.
In .mise.toml, tasks take this form:

    [tasks.build]
    run = "npm run build"
    sources = ["src/**/*.ts"]
    outputs = ["dist/**/*.js"]

Alternatively, tasks can be defined as standalone scripts.
These must be located in the `.mise/tasks` directory.
The name of the script will be the name of the task.

    $ cat .mise/tasks/build<<EOF
    #!/usr/bin/env bash
    npm run build
    EOF
    $ mise run build
Examples:
  $ mise run lint
  Runs the "lint" task. This needs to either be defined in .mise.toml
  or as a standalone script. See the project README for more information.

  $ mise run build --force
  Forces the "build" task to run even if its sources are up-to-date.

  $ mise run test --raw
  Runs "test" with stdin/stdout/stderr all connected to the current terminal.
  This forces `--jobs=1` to prevent interleaving of output.

  $ mise run lint ::: test ::: check
  Runs the "lint", "test", and "check" tasks in parallel.

  $ mise task cmd1 arg1 arg2 ::: cmd2 arg1 arg2
  Execute multiple tasks each with their own arguments.

### `trust`

**Args:**

* `[CONFIG_FILE]` – The config file to trust

**Flags:**

* `-a,--all` – Trust all config files in the current directory and its parents
* `--untrust` – No longer trust this config

Marks a config file as trusted

This means mise will parse the file with potentially dangerous
features enabled.

This includes:
- environment variables
- templates
- `path:` plugin versions
Examples:
  # trusts ~/some_dir/.mise.toml
  $ mise trust ~/some_dir/.mise.toml

  # trusts .mise.toml in the current or parent directory
  $ mise trust

### `uninstall`

* Aliases: `remove`, `rm`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to remove

**Flags:**

* `-a,--all` – Delete all installed versions
* `-n,--dry-run` – Do not actually delete anything

Removes runtime versions
Examples:
  $ mise uninstall node@18.0.0 # will uninstall specific version
  $ mise uninstall node        # will uninstall current node version
  $ mise uninstall --all node@18.0.0 # will uninstall all node versions

### `unset`

**Args:**

* `[KEYS]...` – Environment variable(s) to remove
e.g.: NODE_ENV

**Flags:**

* `-f,--file <FILE>` – Specify a file to use instead of ".mise.toml"
* `-g,--global` – Use the global config file

Remove environment variable(s) from the config file

By default this command modifies ".mise.toml" in the current directory.

### `upgrade`

* Aliases: `up`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to upgrade
e.g.: node@20 python@3.10
If not specified, all current tools will be upgraded

**Flags:**

* `-n,--dry-run` – Just print what would be done, don't actually do it
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `-i,--interactive` – Display multiselect menu to choose which tools to upgrade
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

Upgrades outdated tool versions

### `usage`

Generate usage spec

### `use`

* Aliases: `u`

**Args:**

* `[TOOL@VERSION]...` – Tool(s) to add to config file
e.g.: node@20, cargo:ripgrep@latest npm:prettier@3
If no version is specified, it will default to @latest

**Flags:**

* `-f,--force` – Force reinstall even if already installed
* `--fuzzy` – Save fuzzy version to config file
e.g.: `mise use --fuzzy node@20` will save 20 as the version
this is the default behavior unless MISE_ASDF_COMPAT=1
* `-g,--global` – Use the global config file (~/.config/mise/config.toml) instead of the local one
* `-e,--env <ENV>` – Modify an environment-specific config file like .mise.<env>.toml
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
* `--remove <PLUGIN>` – Remove the plugin(s) from config file
* `-p,--path <PATH>` – Specify a path to a config file or directory If a directory is specified, it will look for .mise.toml (default) or .tool-versions
* `--pin` – Save exact version to config file
e.g.: `mise use --pin node@20` will save 20.0.0 as the version
Set MISE_ASDF_COMPAT=1 to make this the default behavior

Change the active version of a tool locally or globally.

This will install the tool if it is not already installed.
By default, this will use an `.mise.toml` file in the current directory.
Use the --global flag to use the global config file instead.
This replaces asdf's `local` and `global` commands, however those are still available in mise.
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

### `version`

Show mise version

### `watch`

* Aliases: `w`

**Args:**

* `[ARGS]...` – Extra arguments

**Flags:**

* `-t,--task <TASK>` – Task to run
* `-g,--glob <GLOB>` – Files to watch
Defaults to sources from the task(s)

[experimental] Run a task watching for changes
Examples:
  $ mise watch -t build
  Runs the "build" task. Will re-run the task when any of its sources change.
  Uses "sources" from the task definition to determine which files to watch.

  $ mise watch -t build --glob src/**/*.rs
  Runs the "build" task but specify the files to watch with a glob pattern.
  This overrides the "sources" from the task definition.

  $ mise run -t build --clear
  Extra arguments are passed to watchexec. See `watchexec --help` for details.

### `where`

**Args:**

* `<TOOL@VERSION>` – Tool(s) to look up
e.g.: ruby@3
if "@<PREFIX>" is specified, it will show the latest installed version
that matches the prefix
otherwise, it will show the current, active installed version
* `[ASDF_VERSION]` – the version prefix to use when querying the latest version
same as the first argument after the "@"
used for asdf compatibility

Display the installation path for a runtime

Must be installed.
Examples:
  # Show the latest installed version of node
  # If it is is not installed, errors
  $ mise where node@20
  /home/jdx/.local/share/mise/installs/node/20.0.0

  # Show the current, active install directory of node
  # Errors if node is not referenced in any .tool-version file
  $ mise where node
  /home/jdx/.local/share/mise/installs/node/20.0.0

### `which`

**Args:**

* `<BIN_NAME>` – The bin name to look up

**Flags:**

* `--plugin` – Show the plugin name instead of the path
* `--version` – Show the version instead of the path
* `-t,--tool <TOOL@VERSION>` – Use a specific tool@version
e.g.: `mise which npm --tool=node@20`

Shows the path that a bin name points to
Examples:
  $ mise which node
  /home/username/.local/share/mise/installs/node/20.0.0/bin/node
  $ mise which node --plugin
  node
  $ mise which node --version
  20.0.0

### `zzz`

Sleeps for a while.  The amount of time is determined by the --timeout option.

**Examples:**

**Create something**

```sh
mise zzz
mise zzz --timeout 2
```

xxx

```
$ mise zzz
Sleeping for 1.5 seconds...
Done.
```

**Create something**

```sh
$ mise zzz --timeout 2
Sleeping for 2 seconds...
Done.
```

xxx

<!-- [USAGE] -->
