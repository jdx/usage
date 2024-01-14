<!-- [USAGE] load file="./mise.usage.kdl" -->
<!-- [USAGE] title -->

# {spec.name}

<!-- [USAGE] -->
<!-- [USAGE] usage_overview -->

## Usage

```bash
mise.usage.kdl [flags] [args]
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
<!-- [USAGE] command_index -->

## Commands Index

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


<!-- [USAGE] -->
## Commands
<!-- [USAGE] commands -->

### `activate`

#### Args

* `[SHELL_TYPE]` – Shell type to generate the script for
#### Flags

* `-s,--shell <SHELL>` – Shell type to generate the script for
* `--status` – Show "mise: <PLUGIN>@<VERSION>" message when changing directories
* `-q,--quiet` – Suppress non-error messages


### `alias`

#### Flags

* `-p,--plugin <PLUGIN>` – filter aliases by plugin
* `--no-header` – Don't show table header


### `alias get`

#### Args

* `<PLUGIN>` – The plugin to show the alias for
* `<ALIAS>` – The alias to show


### `alias ls`

#### Args

* `[PLUGIN]` – Show aliases for <PLUGIN>
#### Flags

* `--no-header` – Don't show table header


### `alias set`

#### Args

* `<PLUGIN>` – The plugin to set the alias for
* `<ALIAS>` – The alias to set
* `<VALUE>` – The value to set the alias to


### `alias unset`

#### Args

* `<PLUGIN>` – The plugin to remove the alias from
* `<ALIAS>` – The alias to remove


### `bin-paths`



### `cache`



### `cache clear`

#### Args

* `[PLUGIN]...` – Plugin(s) to clear cache for e.g.: node, python


### `completion`

#### Args

* `[SHELL]` – Shell type to generate completions for
#### Flags

* `-s,--shell <SHELL_TYPE>` – Shell type to generate completions for


### `config`

#### Flags

* `--no-header` – Do not print table header


### `config ls`

#### Flags

* `--no-header` – Do not print table header


### `config generate`

#### Flags

* `-o,--output <OUTPUT>` – Output to file instead of stdout


### `current`

#### Args

* `[PLUGIN]` – Plugin to show versions of e.g.: ruby, node, cargo:eza, npm:prettier, etc


### `deactivate`



### `direnv`



### `direnv activate`



### `doctor`



### `env`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to use
#### Flags

* `-s,--shell <SHELL>` – Shell type to generate environment variables for
* `-J,--json` – Output in JSON format


### `exec`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to start e.g.: node@20 python@3.10
* `[COMMAND]...` – Command string to execute (same as --command)
#### Flags

* `-c,--command <C>` – Command string to execute
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1


### `implode`

#### Flags

* `--config` – Also remove config directory
* `-n,--dry-run` – List directories that would be removed without actually removing them


### `install`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to install e.g.: node@20
#### Flags

* `-f,--force` – Force reinstall even if already installed
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
* `-v,--verbose` – Show installation output


### `latest`

#### Args

* `<TOOL@VERSION>` – Tool to get the latest version of
* `[ASDF_VERSION]` – The version prefix to use when querying the latest version same as the first argument after the "@" used for asdf compatibility
#### Flags

* `-i,--installed` – Show latest installed instead of available version


### `link`

#### Args

* `<TOOL@VERSION>` – Tool name and version to create a symlink for
* `<PATH>` – The local path to the tool version
e.g.: ~/.nvm/versions/node/v20.0.0
#### Flags

* `-f,--force` – Overwrite an existing tool version if it exists


### `ls`

#### Args

* `[PLUGIN]...` – Only show tool versions from [PLUGIN]
#### Flags

* `-p,--plugin <PLUGIN_FLAG>` – 
* `-c,--current` – Only show tool versions currently specified in a .tool-versions/.mise.toml
* `-g,--global` – Only show tool versions currently specified in a the global .tool-versions/.mise.toml
* `-i,--installed` – Only show tool versions that are installed Hides missing ones defined in .tool-versions/.mise.toml but not yet installed
* `--parseable` – Output in an easily parseable format
* `-J,--json` – Output in json format
* `-m,--missing` – Display missing tool versions
* `--prefix <PREFIX>` – Display versions matching this prefix
* `--no-header` – Don't display headers


### `ls-remote`

#### Args

* `[TOOL@VERSION]` – Plugin to get versions for
* `[PREFIX]` – The version prefix to use when querying the latest version
same as the first argument after the "@"
#### Flags

* `--all` – Show all installed plugins and versions


### `outdated`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to show outdated versions for
e.g.: node@20 python@3.10
If not specified, all tools in global and local configs will be shown


### `plugins`

#### Flags

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


### `plugins install`

#### Args

* `[NEW_PLUGIN]` – The name of the plugin to install
e.g.: node, ruby
Can specify multiple plugins: `mise plugins install node ruby python`
* `[GIT_URL]` – The git url of the plugin
* `[REST]...` – 
#### Flags

* `-f,--force` – Reinstall even if plugin exists
* `-a,--all` – Install all missing plugins
This will only install plugins that have matching shorthands.
i.e.: they don't need the full git repo url
* `-v,--verbose` – Show installation output


### `plugins link`

#### Args

* `<NAME>` – The name of the plugin
e.g.: node, ruby
* `[PATH]` – The local path to the plugin
e.g.: ./mise-node
#### Flags

* `-f,--force` – Overwrite existing plugin


### `plugins ls`

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


### `plugins ls-remote`

#### Flags

* `-u,--urls` – Show the git url for each plugin e.g.: https://github.com/mise-plugins/rtx-nodejs.git
* `--only-names` – Only show the name of each plugin by default it will show a "*" next to installed plugins


### `plugins uninstall`

#### Args

* `[PLUGIN]...` – Plugin(s) to remove
#### Flags

* `-p,--purge` – Also remove the plugin's installs, downloads, and cache
* `-a,--all` – Remove all plugins


### `plugins update`

#### Args

* `[PLUGIN]...` – Plugin(s) to update
#### Flags

* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
Default: 4


### `prune`

#### Args

* `[PLUGIN]...` – Prune only versions from this plugin(s)
#### Flags

* `-n,--dry-run` – Do not actually delete anything


### `reshim`

#### Args

* `[PLUGIN]` – 
* `[VERSION]` – 


### `run`

#### Args

* `[TASK]` – Task to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2
* `[ARGS]...` – Arguments to pass to the task. Use ":::" to separate tasks
#### Flags

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


### `self-update`

#### Args

* `[VERSION]` – Update to a specific version
#### Flags

* `-f,--force` – Update even if already up to date
* `--no-plugins` – Disable auto-updating plugins
* `-y,--yes` – Skip confirmation prompt


### `set`

#### Args

* `[ENV_VARS]...` – Environment variable(s) to set
e.g.: NODE_ENV=production
#### Flags

* `--file <FILE>` – The TOML file to update

Defaults to MISE_DEFAULT_CONFIG_FILENAME environment variable, or ".mise.toml".
* `-g,--global` – Set the environment variable in the global config file
* `--remove <ENV_VAR>` – Remove the environment variable from config file

Can be used multiple times.


### `settings`



### `settings get`

#### Args

* `<SETTING>` – The setting to show


### `settings ls`



### `settings set`

#### Args

* `<SETTING>` – The setting to set
* `<VALUE>` – The value to set


### `settings unset`

#### Args

* `<SETTING>` – The setting to remove


### `shell`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to use
#### Flags

* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1
* `-u,--unset` – Removes a previously set version


### `sync`



### `sync node`

#### Flags

* `--brew` – Get tool versions from Homebrew
* `--nvm` – Get tool versions from nvm
* `--nodenv` – Get tool versions from nodenv


### `sync python`

#### Flags

* `--pyenv` – Get tool versions from pyenv


### `task`

#### Flags

* `--no-header` – Do not print table header
* `--hidden` – Show hidden tasks


### `task edit`

#### Args

* `<TASK>` – Task to edit
#### Flags

* `-p,--path` – Display the path to the task instead of editing it


### `task ls`

#### Flags

* `--no-header` – Do not print table header
* `--hidden` – Show hidden tasks


### `task run`

#### Args

* `[TASK]` – Task to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2
* `[ARGS]...` – Arguments to pass to the task. Use ":::" to separate tasks
#### Flags

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


### `trust`

#### Args

* `[CONFIG_FILE]` – The config file to trust
#### Flags

* `-a,--all` – Trust all config files in the current directory and its parents
* `--untrust` – No longer trust this config


### `uninstall`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to remove
#### Flags

* `-a,--all` – Delete all installed versions
* `-n,--dry-run` – Do not actually delete anything


### `unset`

#### Args

* `[KEYS]...` – Environment variable(s) to remove
e.g.: NODE_ENV
#### Flags

* `-f,--file <FILE>` – Specify a file to use instead of ".mise.toml"
* `-g,--global` – Use the global config file


### `upgrade`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to upgrade
e.g.: node@20 python@3.10
If not specified, all current tools will be upgraded
#### Flags

* `-n,--dry-run` – Just print what would be done, don't actually do it
* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
[default: 4]
* `-i,--interactive` – Display multiselect menu to choose which tools to upgrade
* `--raw` – Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1


### `usage`



### `use`

#### Args

* `[TOOL@VERSION]...` – Tool(s) to add to config file
e.g.: node@20, cargo:ripgrep@latest npm:prettier@3
If no version is specified, it will default to @latest
#### Flags

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


### `version`



### `watch`

#### Args

* `[ARGS]...` – Extra arguments
#### Flags

* `-t,--task <TASK>` – Task to run
* `-g,--glob <GLOB>` – Files to watch
Defaults to sources from the task(s)


### `where`

#### Args

* `<TOOL@VERSION>` – Tool(s) to look up
e.g.: ruby@3
if "@<PREFIX>" is specified, it will show the latest installed version
that matches the prefix
otherwise, it will show the current, active installed version
* `[ASDF_VERSION]` – the version prefix to use when querying the latest version
same as the first argument after the "@"
used for asdf compatibility


### `which`

#### Args

* `<BIN_NAME>` – The bin name to look up
#### Flags

* `--plugin` – Show the plugin name instead of the path
* `--version` – Show the version instead of the path
* `-t,--tool <TOOL@VERSION>` – Use a specific tool@version
e.g.: `mise which npm --tool=node@20`

<!-- [USAGE] -->
