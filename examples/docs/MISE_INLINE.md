# `mise`

mise is a tool for managing runtime versions. https://github.com/jdx/mise

It's a replacement for tools like nvm, nodenv, rbenv, rvm, chruby, pyenv, etc.
that works for any language. It's also great for managing linters/tools like
jq and shellcheck.

It is inspired by asdf and uses asdf's plugin ecosystem under the hood:
https://asdf-vm.com/


- **Usage**: `mise [FLAGS] <SUBCOMMAND>`

## Global Flags

### `-C --cd <DIR>`

Change directory before running command

### `-P --profile <PROFILE>`

Set the profile (environment)

### `-q --quiet`

Suppress non-error messages

### `-v --verbose...`

Show extra output (use -vv for even more)

### `-y --yes`

Answer yes to all confirmation prompts

## `mise activate`

- **Usage**: `mise activate [--shims] [-q --quiet] [SHELL_TYPE]`

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

### Arguments

#### `[SHELL_TYPE]`

Shell type to generate the script for

**Choices:**

- `bash`
- `fish`
- `nu`
- `xonsh`
- `zsh`

### Flags

#### `--shims`

Use shims instead of modifying PATH
Effectively the same as:

    PATH="$HOME/.local/share/mise/shims:$PATH"

#### `-q --quiet`

Suppress non-error messages

Examples:

    $ eval "$(mise activate bash)"
    $ eval "$(mise activate zsh)"
    $ mise activate fish | source
    $ execx($(mise activate xonsh))

## `mise alias`

- **Usage**: `mise alias [-p --plugin <PLUGIN>] [--no-header] <SUBCOMMAND>`
- **Aliases**: `a`

Manage aliases

### Flags

#### `-p --plugin <PLUGIN>`

filter aliases by plugin

#### `--no-header`

Don't show table header

## `mise alias get`

- **Usage**: `mise alias get <PLUGIN> <ALIAS>`

Show an alias for a plugin

This is the contents of an alias.<PLUGIN> entry in ~/.config/mise/config.toml

### Arguments

#### `<PLUGIN>`

The plugin to show the alias for

#### `<ALIAS>`

The alias to show

Examples:

   $ mise alias get node lts-hydrogen
   20.0.0

## `mise alias ls`

- **Usage**: `mise alias ls [--no-header] [PLUGIN]`
- **Aliases**: `list`

List aliases
Shows the aliases that can be specified.
These can come from user config or from plugins in `bin/list-aliases`.

For user config, aliases are defined like the following in `~/.config/mise/config.toml`:

    [alias.node]
    lts = "20.0.0"

### Arguments

#### `[PLUGIN]`

Show aliases for <PLUGIN>

### Flags

#### `--no-header`

Don't show table header

Examples:

    $ mise aliases
    node    lts-hydrogen   20.0.0

## `mise alias set`

- **Usage**: `mise alias set <ARGS>…`
- **Aliases**: `add`, `create`

Add/update an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<PLUGIN>`

The plugin to set the alias for

#### `<ALIAS>`

The alias to set

#### `<VALUE>`

The value to set the alias to

Examples:

    $ mise alias set node lts-hydrogen 18.0.0

## `mise alias unset`

- **Usage**: `mise alias unset <PLUGIN> <ALIAS>`
- **Aliases**: `rm`, `remove`, `delete`, `del`

Clears an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<PLUGIN>`

The plugin to remove the alias from

#### `<ALIAS>`

The alias to remove

Examples:

    $ mise alias unset node lts-hydrogen

## `mise asdf`

- **Usage**: `mise asdf [ARGS]...`

[internal] simulates asdf for plugins that call "asdf" internally

### Arguments

#### `[ARGS]...`

all arguments

## `mise backends`

- **Usage**: `mise backends <SUBCOMMAND>`
- **Aliases**: `b`

Manage backends

## `mise backends ls`

- **Usage**: `mise backends ls`
- **Aliases**: `list`

List built-in backends

Examples:

    $ mise backends ls
    cargo
    go
    npm
    pipx
    spm
    ubi

## `mise bin-paths`

- **Usage**: `mise bin-paths`

List all the active runtime bin paths

## `mise cache`

- **Usage**: `mise cache <SUBCOMMAND>`

Manage the mise cache

Run `mise cache` with no args to view the current cache directory.

## `mise cache clear`

- **Usage**: `mise cache clear [PLUGIN]...`
- **Aliases**: `c`

Deletes all cache files in mise

### Arguments

#### `[PLUGIN]...`

Plugin(s) to clear cache for e.g.: node, python

## `mise cache prune`

- **Usage**: `mise cache prune [--dry-run] [-v --verbose...] [PLUGIN]...`
- **Aliases**: `p`

Removes stale mise cache files

By default, this command will remove files that have not been accessed in 30 days.
Change this with the MISE_CACHE_PRUNE_AGE environment variable.

### Arguments

#### `[PLUGIN]...`

Plugin(s) to clear cache for e.g.: node, python

### Flags

#### `--dry-run`

Just show what would be pruned

#### `-v --verbose...`

Show pruned files

## `mise completion`

- **Usage**: `mise completion [SHELL]`

Generate shell completions

### Arguments

#### `[SHELL]`

Shell type to generate completions for

**Choices:**

- `bash`
- `fish`
- `zsh`

Examples:

    $ mise completion bash > /etc/bash_completion.d/mise
    $ mise completion zsh  > /usr/local/share/zsh/site-functions/_mise
    $ mise completion fish > ~/.config/fish/completions/mise.fish

## `mise config`

- **Usage**: `mise config [--no-header] [-J --json] <SUBCOMMAND>`
- **Aliases**: `cfg`

Manage config files

### Flags

#### `--no-header`

Do not print table header

#### `-J --json`

Output in JSON format

## `mise config generate`

- **Usage**: `mise config generate [-o --output <OUTPUT>]`
- **Aliases**: `g`

[experimental] Generate a mise.toml file

### Flags

#### `-o --output <OUTPUT>`

Output to file instead of stdout

Examples:

    $ mise cf generate > mise.toml
    $ mise cf generate --output=mise.toml

## `mise config get`

- **Usage**: `mise config get [-f --file <FILE>] [KEY]`

Display the value of a setting in a mise.toml file

### Arguments

#### `[KEY]`

The path of the config to display

### Flags

#### `-f --file <FILE>`

The path to the mise.toml file to edit

If not provided, the nearest mise.toml file will be used

Examples:

    $ mise toml get tools.python
    3.12

## `mise config ls`

- **Usage**: `mise config ls [--no-header] [-J --json]`

List config files currently in use

### Flags

#### `--no-header`

Do not print table header

#### `-J --json`

Output in JSON format

Examples:

    $ mise config ls

## `mise config set`

- **Usage**: `mise config set [-f --file <FILE>] [-t --type <TYPE>] <KEY> <VALUE>`

Display the value of a setting in a mise.toml file

### Arguments

#### `<KEY>`

The path of the config to display

#### `<VALUE>`

The value to set the key to

### Flags

#### `-f --file <FILE>`

The path to the mise.toml file to edit

If not provided, the nearest mise.toml file will be used

#### `-t --type <TYPE>`

**Choices:**

- `string`
- `integer`
- `float`
- `bool`

Examples:

    $ mise config set tools.python 3.12
    $ mise config set settings.always_keep_download true
    $ mise config set env.TEST_ENV_VAR ABC

## `mise current`

- **Usage**: `mise current [PLUGIN]`

Shows current active and installed runtime versions

This is similar to `mise ls --current`, but this only shows the runtime
and/or version. It's designed to fit into scripts more easily.

### Arguments

#### `[PLUGIN]`

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

- **Usage**: `mise deactivate`

Disable mise for current shell session

This can be used to temporarily disable mise in a shell session.

Examples:

    $ mise deactivate

## `mise direnv`

- **Usage**: `mise direnv <SUBCOMMAND>`

Output direnv function to use mise inside direnv

See https://mise.jdx.dev/direnv.html for more information

Because this generates the legacy files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when legacy file versions change.

## `mise direnv envrc`

- **Usage**: `mise direnv envrc`

[internal] This is an internal command that writes an envrc file
for direnv to consume.

## `mise direnv exec`

- **Usage**: `mise direnv exec`

[internal] This is an internal command that writes an envrc file
for direnv to consume.

## `mise direnv activate`

- **Usage**: `mise direnv activate`

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

- **Usage**: `mise doctor`
- **Aliases**: `dr`

Check mise installation for possible problems

Examples:

    $ mise doctor
    [WARN] plugin node is not installed

## `mise env`

- **Usage**: `mise env [-J --json] [-s --shell <SHELL>] [TOOL@VERSION]...`
- **Aliases**: `e`

Exports env vars to activate mise a single time

Use this if you don't want to permanently install mise. It's not necessary to
use this if you have `mise activate` in your shell rc file.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to use

### Flags

#### `-J --json`

Output in JSON format

#### `-s --shell <SHELL>`

Shell type to generate environment variables for

**Choices:**

- `bash`
- `fish`
- `nu`
- `xonsh`
- `zsh`

Examples:

    $ eval "$(mise env -s bash)"
    $ eval "$(mise env -s zsh)"
    $ mise env -s fish | source
    $ execx($(mise env -s xonsh))

## `mise exec`

- **Usage**: `mise exec [FLAGS] [TOOL@VERSION]... [COMMAND]...`
- **Aliases**: `x`

Execute a command with tool(s) set

use this to avoid modifying the shell session or running ad-hoc commands with mise tools set.

Tools will be loaded from mise.toml, though they can be overridden with <RUNTIME> args
Note that only the plugin specified will be overridden, so if a `mise.toml` file
includes "node 20" but you run `mise exec python@3.11`; it will still load node@20.

The "--" separates runtimes from the commands to pass along to the subprocess.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to start e.g.: node@20 python@3.10

#### `[COMMAND]...`

Command string to execute (same as --command)

### Flags

#### `-c --command <C>`

Command string to execute

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

Examples:

    $ mise exec node@20 -- node ./app.js  # launch app.js using node-20.x
    $ mise x node@20 -- node ./app.js     # shorter alias

    # Specify command as a string:
    $ mise exec node@20 python@3.11 --command "node -v && python -V"

    # Run a command in a different directory:
    $ mise x -C /path/to/project node@20 -- node ./app.js

## `mise generate`

- **Usage**: `mise generate <SUBCOMMAND>`
- **Aliases**: `g`

[experimental] Generate files for various tools/services

## `mise generate git-pre-commit`

- **Usage**: `mise generate git-pre-commit [FLAGS]`
- **Aliases**: `pre-commit`

[experimental] Generate a git pre-commit hook

This command generates a git pre-commit hook that runs a mise task like `mise run pre-commit`
when you commit changes to your repository.

### Flags

#### `--hook <HOOK>`

Which hook to generate (saves to .git/hooks/$hook)

#### `-t --task <TASK>`

The task to run when the pre-commit hook is triggered

#### `-w --write`

write to .git/hooks/pre-commit and make it executable

Examples:

    $ mise generate git-pre-commit --write --task=pre-commit
    $ git commit -m "feat: add new feature" # runs `mise run pre-commit`

## `mise generate github-action`

- **Usage**: `mise generate github-action [FLAGS]`

[experimental] Generate a GitHub Action workflow file

This command generates a GitHub Action workflow file that runs a mise task like `mise run ci`
when you push changes to your repository.

### Flags

#### `-n --name <NAME>`

the name of the workflow to generate

#### `-t --task <TASK>`

The task to run when the workflow is triggered

#### `-w --write`

write to .github/workflows/$name.yml

Examples:

    $ mise generate github-action --write --task=ci
    $ git commit -m "feat: add new feature"
    $ git push # runs `mise run ci` on GitHub

## `mise generate task-docs`

- **Usage**: `mise generate task-docs [FLAGS]`

[experimental] Generate documentation for tasks in a project

### Flags

#### `-I --index`

write only an index of tasks, intended for use with `--multi`

#### `-i --inject`

inserts the documentation into an existing file

This will look for a special comment, <!-- mise-tasks -->, and replace it with the generated documentation.
It will replace everything between the comment and the next comment, <!-- /mise-tasks --> so it can be
run multiple times on the same file to update the documentation.

#### `-m --multi`

render each task as a separate document, requires `--output` to be a directory

#### `-o --output <OUTPUT>`

writes the generated docs to a file/directory

#### `-r --root <ROOT>`

root directory to search for tasks

#### `-s --style <STYLE>`

**Choices:**

- `simple`
- `detailed`

Examples:

    $ mise generate task-docs

## `mise global`

- **Usage**: `mise global [FLAGS] [TOOL@VERSION]...`

Sets/gets the global tool version(s)

Displays the contents of global config after writing.
The file is `$HOME/.config/mise/config.toml` by default. It can be changed with `$MISE_GLOBAL_CONFIG_FILE`.
If `$MISE_GLOBAL_CONFIG_FILE` is set to anything that ends in `.toml`, it will be parsed as `mise.toml`.
Otherwise, it will be parsed as a `.tool-versions` file.

Use MISE_ASDF_COMPAT=1 to default the global config to ~/.tool-versions

Use `mise local` to set a tool version locally in the current directory.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to add to .tool-versions
e.g.: node@20
If this is a single tool with no version, the current value of the global
.tool-versions will be displayed

### Flags

#### `--pin`

Save exact version to `~/.tool-versions`
e.g.: `mise global --pin node@20` will save `node 20.0.0` to ~/.tool-versions

#### `--fuzzy`

Save fuzzy version to `~/.tool-versions`
e.g.: `mise global --fuzzy node@20` will save `node 20` to ~/.tool-versions
this is the default behavior unless MISE_ASDF_COMPAT=1

#### `--remove... <PLUGIN>`

Remove the plugin(s) from ~/.tool-versions

#### `--path`

Get the path of the global config file

Examples:
    # set the current version of node to 20.x
    # will use a fuzzy version (e.g.: 20) in .tool-versions file
    $ mise global --fuzzy node@20

    # set the current version of node to 20.x
    # will use a precise version (e.g.: 20.0.0) in .tool-versions file
    $ mise global --pin node@20

    # show the current version of node in ~/.tool-versions
    $ mise global node
    20.0.0

## `mise hook-env`

- **Usage**: `mise hook-env [-s --shell <SHELL>] [-q --quiet]`

[internal] called by activate hook to update env vars directory change

### Flags

#### `-s --shell <SHELL>`

Shell type to generate script for

**Choices:**

- `bash`
- `fish`
- `nu`
- `xonsh`
- `zsh`

#### `-q --quiet`

Hide warnings such as when a tool is not installed

## `mise hook-not-found`

- **Usage**: `mise hook-not-found [-s --shell <SHELL>] <BIN>`

[internal] called by shell when a command is not found

### Arguments

#### `<BIN>`

Attempted bin to run

### Flags

#### `-s --shell <SHELL>`

Shell type to generate script for

**Choices:**

- `bash`
- `fish`
- `nu`
- `xonsh`
- `zsh`

## `mise implode`

- **Usage**: `mise implode [--config] [-n --dry-run]`

Removes mise CLI and all related data

Skips config directory by default.

### Flags

#### `--config`

Also remove config directory

#### `-n --dry-run`

List directories that would be removed without actually removing them

## `mise install`

- **Usage**: `mise install [FLAGS] [TOOL@VERSION]...`
- **Aliases**: `i`

Install a tool version

Installs a tool version to `~/.local/share/mise/installs/<PLUGIN>/<VERSION>`
Installing alone will not activate the tools so they won't be in PATH.
To install and/or activate in one command, use `mise use` which will create a `mise.toml` file
in the current directory to activate this tool when inside the directory.
Alternatively, run `mise exec <TOOL>@<VERSION> -- <COMMAND>` to execute a tool without creating config files.

Tools will be installed in parallel. To disable, set `--jobs=1` or `MISE_JOBS=1`

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to install e.g.: node@20

### Flags

#### `-f --force`

Force reinstall even if already installed

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

#### `-v --verbose...`

Show installation output

This argument will print plugin output such as download, configuration, and compilation output.

Examples:

    $ mise install node@20.0.0  # install specific node version
    $ mise install node@20      # install fuzzy node version
    $ mise install node         # install version specified in mise.toml
    $ mise install              # installs everything specified in mise.toml

## `mise latest`

- **Usage**: `mise latest [-i --installed] <TOOL@VERSION>`

Gets the latest available version for a plugin

Supports prefixes such as `node@20` to get the latest version of node 20.

### Arguments

#### `<TOOL@VERSION>`

Tool to get the latest version of

### Flags

#### `-i --installed`

Show latest installed instead of available version

Examples:

    $ mise latest node@20  # get the latest version of node 20
    20.0.0

    $ mise latest node     # get the latest stable version of node
    20.0.0

## `mise link`

- **Usage**: `mise link [-f --force] <TOOL@VERSION> <PATH>`
- **Aliases**: `ln`

Symlinks a tool version into mise

Use this for adding installs either custom compiled outside mise or built with a different tool.

### Arguments

#### `<TOOL@VERSION>`

Tool name and version to create a symlink for

#### `<PATH>`

The local path to the tool version
e.g.: ~/.nvm/versions/node/v20.0.0

### Flags

#### `-f --force`

Overwrite an existing tool version if it exists

Examples:
    
    # build node-20.0.0 with node-build and link it into mise
    $ node-build 20.0.0 ~/.nodes/20.0.0
    $ mise link node@20.0.0 ~/.nodes/20.0.0

    # have mise use the python version provided by Homebrew
    $ brew install node
    $ mise link node@brew $(brew --prefix node)
    $ mise use node@brew

## `mise local`

- **Usage**: `mise local [FLAGS] [TOOL@VERSION]...`

Sets/gets tool version in local .tool-versions or mise.toml

Use this to set a tool's version when within a directory
Use `mise global` to set a tool version globally
This uses `.tool-version` by default unless there is a `mise.toml` file or if `MISE_USE_TOML`
is set. A future v2 release of mise will default to using `mise.toml`.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to add to .tool-versions/mise.toml
e.g.: node@20
if this is a single tool with no version,
the current value of .tool-versions/mise.toml will be displayed

### Flags

#### `-p --parent`

Recurse up to find a .tool-versions file rather than using the current directory only
by default this command will only set the tool in the current directory ("$PWD/.tool-versions")

#### `--pin`

Save exact version to `.tool-versions`
e.g.: `mise local --pin node@20` will save `node 20.0.0` to .tool-versions

#### `--fuzzy`

Save fuzzy version to `.tool-versions` e.g.: `mise local --fuzzy node@20` will save `node 20` to .tool-versions This is the default behavior unless MISE_ASDF_COMPAT=1

#### `--remove... <PLUGIN>`

Remove the plugin(s) from .tool-versions

#### `--path`

Get the path of the config file

Examples:
    # set the current version of node to 20.x for the current directory
    # will use a precise version (e.g.: 20.0.0) in .tool-versions file
    $ mise local node@20

    # set node to 20.x for the current project (recurses up to find .tool-versions)
    $ mise local -p node@20

    # set the current version of node to 20.x for the current directory
    # will use a fuzzy version (e.g.: 20) in .tool-versions file
    $ mise local --fuzzy node@20

    # removes node from .tool-versions
    $ mise local --remove=node

    # show the current version of node in .tool-versions
    $ mise local node
    20.0.0

## `mise ls`

- **Usage**: `mise ls [FLAGS] [PLUGIN]...`
- **Aliases**: `list`

List installed and active tool versions

This command lists tools that mise "knows about".
These may be tools that are currently installed, or those
that are in a config file (active) but may or may not be installed.

It's a useful command to get the current state of your tools.

### Arguments

#### `[PLUGIN]...`

Only show tool versions from [PLUGIN]

### Flags

#### `-c --current`

Only show tool versions currently specified in a mise.toml

#### `-g --global`

Only show tool versions currently specified in the global mise.toml

#### `-i --installed`

Only show tool versions that are installed (Hides tools defined in mise.toml but not installed)

#### `-J --json`

Output in JSON format

#### `-m --missing`

Display missing tool versions

#### `--prefix <PREFIX>`

Display versions matching this prefix

#### `--no-header`

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
            "type": "mise.toml",
            "path": "/Users/jdx/mise.toml"
          }
        }
      ],
      "python": [...]
    }

## `mise ls-remote`

- **Usage**: `mise ls-remote [--all] [TOOL@VERSION] [PREFIX]`

List runtime versions available for install.

Note that the results may be cached, run `mise cache clean` to clear the cache and get fresh results.

### Arguments

#### `[TOOL@VERSION]`

Plugin to get versions for

#### `[PREFIX]`

The version prefix to use when querying the latest version
same as the first argument after the "@"

### Flags

#### `--all`

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

- **Usage**: `mise outdated [FLAGS] [TOOL@VERSION]...`

Shows outdated tool versions

See `mise upgrade` to upgrade these versions.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to show outdated versions for
e.g.: node@20 python@3.10
If not specified, all tools in global and local configs will be shown

### Flags

#### `-l --bump`

Compares against the latest versions available, not what matches the current config

For example, if you have `node = "20"` in your config by default `mise outdated` will only
show other 20.x versions, not 21.x or 22.x versions.

Using this flag, if there are 21.x or newer versions it will display those instead of 20.x.

#### `-J --json`

Output in JSON format

#### `--no-header`

Don't show table header

Examples:

    $ mise outdated
    Plugin  Requested  Current  Latest
    python  3.11       3.11.0   3.11.1
    node    20         20.0.0   20.1.0

    $ mise outdated node
    Plugin  Requested  Current  Latest
    node    20         20.0.0   20.1.0

    $ mise outdated --json
    {"python": {"requested": "3.11", "current": "3.11.0", "latest": "3.11.1"}, ...}

## `mise plugins`

- **Usage**: `mise plugins [FLAGS] <SUBCOMMAND>`
- **Aliases**: `p`

Manage plugins

### Flags

#### `-c --core`

The built-in plugins only
Normally these are not shown

#### `--user`

List installed plugins

This is the default behavior but can be used with --core
to show core and user plugins

#### `-u --urls`

Show the git url for each plugin
e.g.: https://github.com/asdf-vm/asdf-nodejs.git

## `mise plugins install`

- **Usage**: `mise plugins install [FLAGS] [NEW_PLUGIN] [GIT_URL]`
- **Aliases**: `i`, `a`, `add`

Install a plugin

note that mise automatically can install plugins when you install a tool
e.g.: `mise install node@20` will autoinstall the node plugin

This behavior can be modified in ~/.config/mise/config.toml

### Arguments

#### `[NEW_PLUGIN]`

The name of the plugin to install
e.g.: node, ruby
Can specify multiple plugins: `mise plugins install node ruby python`

#### `[GIT_URL]`

The git url of the plugin

### Flags

#### `-f --force`

Reinstall even if plugin exists

#### `-a --all`

Install all missing plugins
This will only install plugins that have matching shorthands.
i.e.: they don't need the full git repo url

#### `-v --verbose...`

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

## `mise plugins link`

- **Usage**: `mise plugins link [-f --force] <NAME> [PATH]`
- **Aliases**: `ln`

Symlinks a plugin into mise

This is used for developing a plugin.

### Arguments

#### `<NAME>`

The name of the plugin
e.g.: node, ruby

#### `[PATH]`

The local path to the plugin
e.g.: ./mise-node

### Flags

#### `-f --force`

Overwrite existing plugin

Examples:

    # essentially just `ln -s ./mise-node ~/.local/share/mise/plugins/node`
    $ mise plugins link node ./mise-node

    # infer plugin name as "node"
    $ mise plugins link ./mise-node

## `mise plugins ls`

- **Usage**: `mise plugins ls [FLAGS]`
- **Aliases**: `list`

List installed plugins

Can also show remotely available plugins to install.

### Flags

#### `-c --core`

The built-in plugins only
Normally these are not shown

#### `--user`

List installed plugins

This is the default behavior but can be used with --core
to show core and user plugins

#### `-u --urls`

Show the git url for each plugin
e.g.: https://github.com/asdf-vm/asdf-nodejs.git

Examples:

    $ mise plugins ls
    node
    ruby

    $ mise plugins ls --urls
    node    https://github.com/asdf-vm/asdf-nodejs.git
    ruby    https://github.com/asdf-vm/asdf-ruby.git

## `mise plugins ls-remote`

- **Usage**: `mise plugins ls-remote [-u --urls] [--only-names]`
- **Aliases**: `list-remote`, `list-all`


List all available remote plugins

The full list is here: https://github.com/jdx/mise/blob/main/registry.toml

Examples:

    $ mise plugins ls-remote

### Flags

#### `-u --urls`

Show the git url for each plugin e.g.: https://github.com/mise-plugins/mise-poetry.git

#### `--only-names`

Only show the name of each plugin by default it will show a "*" next to installed plugins

## `mise plugins uninstall`

- **Usage**: `mise plugins uninstall [-p --purge] [-a --all] [PLUGIN]...`
- **Aliases**: `remove`, `rm`

Removes a plugin

### Arguments

#### `[PLUGIN]...`

Plugin(s) to remove

### Flags

#### `-p --purge`

Also remove the plugin's installs, downloads, and cache

#### `-a --all`

Remove all plugins

Examples:

    $ mise uninstall node

## `mise plugins update`

- **Usage**: `mise plugins update [-j --jobs <JOBS>] [PLUGIN]...`
- **Aliases**: `up`, `upgrade`

Updates a plugin to the latest version

note: this updates the plugin itself, not the runtime versions

### Arguments

#### `[PLUGIN]...`

Plugin(s) to update

### Flags

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
Default: 4

Examples:

    $ mise plugins update            # update all plugins
    $ mise plugins update node       # update only node
    $ mise plugins update node#beta  # specify a ref

## `mise prune`

- **Usage**: `mise prune [FLAGS] [PLUGIN]...`

Delete unused versions of tools

mise tracks which config files have been used in ~/.local/share/mise/tracked_config_files
Versions which are no longer the latest specified in any of those configs are deleted.
Versions installed only with environment variables `MISE_<PLUGIN>_VERSION` will be deleted,
as will versions only referenced on the command line `mise exec <PLUGIN>@<VERSION>`.

### Arguments

#### `[PLUGIN]...`

Prune only versions from this plugin(s)

### Flags

#### `-n --dry-run`

Do not actually delete anything

#### `--configs`

Prune only tracked and trusted configuration links that point to non-existent configurations

#### `--tools`

Prune only unused versions of tools

Examples:

    $ mise prune --dry-run
    rm -rf ~/.local/share/mise/versions/node/20.0.0
    rm -rf ~/.local/share/mise/versions/node/20.0.1

## `mise registry`

- **Usage**: `mise registry`

[experimental] List available tools to install

This command lists the tools available in the registry as shorthand names.

For example, `poetry` is shorthand for `asdf:mise-plugins/mise-poetry`.

Examples:

    $ mise registry
    node    core:node
    poetry  asdf:mise-plugins/mise-poetry
    ubi     cargo:ubi-cli

## `mise reshim`

- **Usage**: `mise reshim [-f --force]`

Creates new shims based on bin paths from currently installed tools.

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

Note that this creates shims for _all_ installed tools, not just the ones that are
currently active in mise.toml.

### Flags

#### `-f --force`

Removes all shims before reshimming

Examples:

    $ mise reshim
    $ ~/.local/share/mise/shims/node -v
    v20.0.0

## `mise run`

- **Usage**: `mise run [FLAGS]`
- **Aliases**: `r`

[experimental] Run task(s)

This command will run a tasks, or multiple tasks in parallel.
Tasks may have dependencies on other tasks or on source files.
If source is configured on a tasks, it will only run if the source
files have changed.

Tasks can be defined in mise.toml or as standalone scripts.
In mise.toml, tasks take this form:

    [tasks.build]
    run = "npm run build"
    sources = ["src/**/*.ts"]
    outputs = ["dist/**/*.js"]

Alternatively, tasks can be defined as standalone scripts.
These must be located in `mise-tasks`, `.mise-tasks`, `.mise/tasks`, `mise/tasks` or
`.config/mise/tasks`.
The name of the script will be the name of the tasks.

    $ cat .mise/tasks/build<<EOF
    #!/usr/bin/env bash
    npm run build
    EOF
    $ mise run build

### Flags

#### `-C --cd <CD>`

Change to this directory before executing the command

#### `-n --dry-run`

Don't actually run the tasks(s), just print them in order of execution

#### `-f --force`

Force the tasks to run even if outputs are up to date

#### `-p --prefix`

Print stdout/stderr by line, prefixed with the tasks's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

#### `-i --interleave`

Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

#### `-t --tool... <TOOL@VERSION>`

Tool(s) to also add e.g.: node@20 python@3.10

#### `-j --jobs <JOBS>`

Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var

#### `-r --raw`

Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

#### `--timings`

Shows elapsed time after each tasks

Examples:

    # Runs the "lint" tasks. This needs to either be defined in mise.toml
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

- **Usage**: `mise self-update [FLAGS] [VERSION]`

Updates mise itself.

Uses the GitHub Releases API to find the latest release and binary.
By default, this will also update any installed plugins.
Uses the `GITHUB_API_TOKEN` environment variable if set for higher rate limits.

This command is not available if mise is installed via a package manager.

### Arguments

#### `[VERSION]`

Update to a specific version

### Flags

#### `-f --force`

Update even if already up to date

#### `--no-plugins`

Disable auto-updating plugins

#### `-y --yes`

Skip confirmation prompt

## `mise set`

- **Usage**: `mise set [--file <FILE>] [-g --global] [ENV_VARS]...`

Set environment variables in mise.toml

By default, this command modifies `mise.toml` in the current directory.

### Arguments

#### `[ENV_VARS]...`

Environment variable(s) to set
e.g.: NODE_ENV=production

### Flags

#### `--file <FILE>`

The TOML file to update

Defaults to MISE_DEFAULT_CONFIG_FILENAME environment variable, or `mise.toml`.

#### `-g --global`

Set the environment variable in the global config file

Examples:

    $ mise set NODE_ENV=production

    $ mise set NODE_ENV
    production

    $ mise set
    key       value       source
    NODE_ENV  production  ~/.config/mise/config.toml

## `mise settings`

- **Usage**: `mise settings [--keys] <SUBCOMMAND>`

Manage settings

### Flags

#### `--keys`

Only display key names for each setting

## `mise settings add`

- **Usage**: `mise settings add <SETTING> <VALUE>`

Adds a setting to the configuration file

Used with an array setting, this will append the value to the array.
This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<SETTING>`

The setting to set

#### `<VALUE>`

The value to set

Examples:

    $ mise settings add disable_hints python_multi

## `mise settings get`

- **Usage**: `mise settings get <SETTING>`

Show a current setting

This is the contents of a single entry in ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases get`

### Arguments

#### `<SETTING>`

The setting to show

Examples:

    $ mise settings get legacy_version_file
    true

## `mise settings ls`

- **Usage**: `mise settings ls [--keys]`
- **Aliases**: `list`

Show current settings

This is the contents of ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases`

### Flags

#### `--keys`

Only display key names for each setting

Examples:

    $ mise settings
    legacy_version_file = false

## `mise settings set`

- **Usage**: `mise settings set <SETTING> <VALUE>`
- **Aliases**: `create`

Add/update a setting

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<SETTING>`

The setting to set

#### `<VALUE>`

The value to set

Examples:

    $ mise settings set legacy_version_file true

## `mise settings unset`

- **Usage**: `mise settings unset <SETTING>`
- **Aliases**: `rm`, `remove`, `delete`, `del`

Clears a setting

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<SETTING>`

The setting to remove

Examples:

    $ mise settings unset legacy_version_file

## `mise shell`

- **Usage**: `mise shell [FLAGS] [TOOL@VERSION]...`
- **Aliases**: `sh`

Sets a tool version for the current session.

Only works in a session where mise is already activated.

This works by setting environment variables for the current shell session
such as `MISE_NODE_VERSION=20` which is "eval"ed as a shell function created by `mise activate`.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to use

### Flags

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

#### `-u --unset`

Removes a previously set version

Examples:

    $ mise shell node@20
    $ node -v
    v20.0.0

## `mise sync`

- **Usage**: `mise sync <SUBCOMMAND>`

Add tool versions from external tools to mise

## `mise sync node`

- **Usage**: `mise sync node [FLAGS]`

Symlinks all tool versions from an external tool into mise

For example, use this to import all Homebrew node installs into mise

### Flags

#### `--brew`

Get tool versions from Homebrew

#### `--nvm`

Get tool versions from nvm

#### `--nodenv`

Get tool versions from nodenv

Examples:

    $ brew install node@18 node@20
    $ mise sync node --brew
    $ mise use -g node@18 - uses Homebrew-provided node

## `mise sync python`

- **Usage**: `mise sync python <--pyenv>`

Symlinks all tool versions from an external tool into mise

For example, use this to import all pyenv installs into mise

### Flags

#### `--pyenv`

Get tool versions from pyenv

Examples:

    $ pyenv install 3.11.0
    $ mise sync python --pyenv
    $ mise use -g python@3.11.0 - uses pyenv-provided python

## `mise tasks`

- **Usage**: `mise tasks [FLAGS] <SUBCOMMAND>`
- **Aliases**: `t`

[experimental] Manage tasks

### Flags

#### `--no-header`

Do not print table header

#### `-x --extended`

Show all columns

#### `--hidden`

Show hidden tasks

#### `--sort <COLUMN>`

Sort by column. Default is name.

**Choices:**

- `name`
- `alias`
- `description`
- `source`

#### `--sort-order <SORT_ORDER>`

Sort order. Default is asc.

**Choices:**

- `asc`
- `desc`

#### `-J --json`

Output in JSON format

Examples:

    $ mise tasks ls

## `mise tasks deps`

- **Usage**: `mise tasks deps [--hidden] [--dot] [TASKS]...`

[experimental] Display a tree visualization of a dependency graph

### Arguments

#### `[TASKS]...`

Tasks to show dependencies for
Can specify multiple tasks by separating with spaces
e.g.: mise tasks deps lint test check

### Flags

#### `--hidden`

Show hidden tasks

#### `--dot`

Display dependencies in DOT format

Examples:

    # Show dependencies for all tasks
    $ mise tasks deps

    # Show dependencies for the "lint", "test" and "check" tasks
    $ mise tasks deps lint test check

    # Show dependencies in DOT format
    $ mise tasks deps --dot

## `mise tasks edit`

- **Usage**: `mise tasks edit [-p --path] <TASK>`

[experimental] Edit a tasks with $EDITOR

The tasks will be created as a standalone script if it does not already exist.

### Arguments

#### `<TASK>`

Tasks to edit

### Flags

#### `-p --path`

Display the path to the tasks instead of editing it

Examples:

    $ mise tasks edit build
    $ mise tasks edit test

## `mise tasks info`

- **Usage**: `mise tasks info [-J --json] <TASK>`

[experimental] Get information about a task

### Arguments

#### `<TASK>`

Name of the task to get information about

### Flags

#### `-J --json`

Output in JSON format

Examples:

    $ mise tasks info
    Name: test
    Aliases: t
    Description: Test the application
    Source: ~/src/myproj/mise.toml

    $ mise tasks info test --json
    {
      "name": "test",
      "aliases": "t",
      "description": "Test the application",
      "source": "~/src/myproj/mise.toml",
      "depends": [],
      "env": {},
      "dir": null,
      "hide": false,
      "raw": false,
      "sources": [],
      "outputs": [],
      "run": [
        "echo \"testing!\""
      ],
      "file": null,
      "usage_spec": {}
    }

## `mise tasks ls`

- **Usage**: `mise tasks ls [FLAGS]`

[experimental] List available tasks to execute
These may be included from the config file or from the project's .mise/tasks directory
mise will merge all tasks from all parent directories into this list.

So if you have global tasks in `~/.config/mise/tasks/*` and project-specific tasks in
~/myproject/.mise/tasks/*, then they'll both be available but the project-specific
tasks will override the global ones if they have the same name.

### Flags

#### `--no-header`

Do not print table header

#### `-x --extended`

Show all columns

#### `--hidden`

Show hidden tasks

#### `--sort <COLUMN>`

Sort by column. Default is name.

**Choices:**

- `name`
- `alias`
- `description`
- `source`

#### `--sort-order <SORT_ORDER>`

Sort order. Default is asc.

**Choices:**

- `asc`
- `desc`

#### `-J --json`

Output in JSON format

Examples:

    $ mise tasks ls

## `mise tasks run`

- **Usage**: `mise tasks run [FLAGS] [TASK] [ARGS]...`
- **Aliases**: `r`

[experimental] Run task(s)

This command will run a tasks, or multiple tasks in parallel.
Tasks may have dependencies on other tasks or on source files.
If source is configured on a tasks, it will only run if the source
files have changed.

Tasks can be defined in mise.toml or as standalone scripts.
In mise.toml, tasks take this form:

    [tasks.build]
    run = "npm run build"
    sources = ["src/**/*.ts"]
    outputs = ["dist/**/*.js"]

Alternatively, tasks can be defined as standalone scripts.
These must be located in `mise-tasks`, `.mise-tasks`, `.mise/tasks`, `mise/tasks` or
`.config/mise/tasks`.
The name of the script will be the name of the tasks.

    $ cat .mise/tasks/build<<EOF
    #!/usr/bin/env bash
    npm run build
    EOF
    $ mise run build

### Arguments

#### `[TASK]`

Tasks to run
Can specify multiple tasks by separating with `:::`
e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2

**Default:** `default`

#### `[ARGS]...`

Arguments to pass to the tasks. Use ":::" to separate tasks

### Flags

#### `-C --cd <CD>`

Change to this directory before executing the command

#### `-n --dry-run`

Don't actually run the tasks(s), just print them in order of execution

#### `-f --force`

Force the tasks to run even if outputs are up to date

#### `-p --prefix`

Print stdout/stderr by line, prefixed with the tasks's label
Defaults to true if --jobs > 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

#### `-i --interleave`

Print directly to stdout/stderr instead of by line
Defaults to true if --jobs == 1
Configure with `task_output` config or `MISE_TASK_OUTPUT` env var

#### `-t --tool... <TOOL@VERSION>`

Tool(s) to also add e.g.: node@20 python@3.10

#### `-j --jobs <JOBS>`

Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var

#### `-r --raw`

Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

#### `--timings`

Shows elapsed time after each tasks

Examples:

    # Runs the "lint" tasks. This needs to either be defined in mise.toml
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

- **Usage**: `mise trust [FLAGS] [CONFIG_FILE]`

Marks a config file as trusted

This means mise will parse the file with potentially dangerous
features enabled.

This includes:
- environment variables
- templates
- `path:` plugin versions

### Arguments

#### `[CONFIG_FILE]`

The config file to trust

### Flags

#### `-a --all`

Trust all config files in the current directory and its parents

#### `--untrust`

No longer trust this config

#### `--show`

Show the trusted status of config files from the current directory and its parents.
Does not trust or untrust any files.

Examples:

    # trusts ~/some_dir/mise.toml
    $ mise trust ~/some_dir/mise.toml

    # trusts mise.toml in the current or parent directory
    $ mise trust

## `mise uninstall`

- **Usage**: `mise uninstall [-a --all] [-n --dry-run] [INSTALLED_TOOL@VERSION]...`
- **Aliases**: `remove`, `rm`

Removes installed tool versions

This only removes the installed version, it does not modify mise.toml.

### Arguments

#### `[INSTALLED_TOOL@VERSION]...`

Tool(s) to remove

### Flags

#### `-a --all`

Delete all installed versions

#### `-n --dry-run`

Do not actually delete anything

Examples:

    # will uninstall specific version
    $ mise uninstall node@18.0.0

    # will uninstall the current node version (if only one version is installed)
    $ mise uninstall node

    # will uninstall all installed versions of node
    $ mise uninstall --all node@18.0.0 # will uninstall all node versions

## `mise unset`

- **Usage**: `mise unset [-f --file <FILE>] [-g --global] [KEYS]...`

Remove environment variable(s) from the config file.

By default, this command modifies `mise.toml` in the current directory.

### Arguments

#### `[KEYS]...`

Environment variable(s) to remove
e.g.: NODE_ENV

### Flags

#### `-f --file <FILE>`

Specify a file to use instead of `mise.toml`

#### `-g --global`

Use the global config file

Examples:

    # Remove NODE_ENV from the current directory's config
    $ mise unset NODE_ENV

    # Remove NODE_ENV from the global config
    $ mise unset NODE_ENV -g

## `mise upgrade`

- **Usage**: `mise upgrade [FLAGS] [TOOL@VERSION]...`
- **Aliases**: `up`

Upgrades outdated tools

By default, this keeps the range specified in mise.toml. So if you have node@20 set, it will
upgrade to the latest 20.x.x version available. See the `--bump` flag to use the latest version
and bump the version in mise.toml.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to upgrade
e.g.: node@20 python@3.10
If not specified, all current tools will be upgraded

### Flags

#### `-n --dry-run`

Just print what would be done, don't actually do it

#### `-i --interactive`

Display multiselect menu to choose which tools to upgrade

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `-l --bump`

Upgrades to the latest version available, bumping the version in mise.toml

For example, if you have `node = "20.0.0"` in your mise.toml but 22.1.0 is the latest available,
this will install 22.1.0 and set `node = "22.1.0"` in your config.

It keeps the same precision as what was there before, so if you instead had `node = "20"`, it
would change your config to `node = "22"`.

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

Examples:

    # Upgrades node to the latest version matching the range in mise.toml
    $ mise upgrade node

    # Upgrades node to the latest version and bumps the version in mise.toml
    $ mise upgrade node --bump

    # Upgrades all tools to the latest versions
    $ mise upgrade

    # Upgrades all tools to the latest versions and bumps the version in mise.toml
    $ mise upgrade --bump

    # Just print what would be done, don't actually do it
    $ mise upgrade --dry-run

    # Upgrades node and python to the latest versions
    $ mise upgrade node python

    # Show a multiselect menu to choose which tools to upgrade
    $ mise upgrade --interactive

## `mise usage`

- **Usage**: `mise usage`

Generate a usage CLI spec

See https://usage.jdx.dev for more information on this specification.

## `mise use`

- **Usage**: `mise use [FLAGS] [TOOL@VERSION]...`
- **Aliases**: `u`

Installs a tool and adds the version it to mise.toml.

This will install the tool version if it is not already installed.
By default, this will use a `mise.toml` file in the current directory.

Use the `--global` flag to use the global config file instead.

### Arguments

#### `[TOOL@VERSION]...`

Tool(s) to add to config file

e.g.: node@20, cargo:ripgrep@latest npm:prettier@3
If no version is specified, it will default to @latest

### Flags

#### `-f --force`

Force reinstall even if already installed

#### `--fuzzy`

Save fuzzy version to config file

e.g.: `mise use --fuzzy node@20` will save 20 as the version
this is the default behavior unless `MISE_PIN=1` or `MISE_ASDF_COMPAT=1`

#### `-g --global`

Use the global config file (`~/.config/mise/config.toml`) instead of the local one

#### `-e --env <ENV>`

Modify an environment-specific config file like .mise.<env>.toml

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets `--jobs=1`

#### `--remove... <PLUGIN>`

Remove the plugin(s) from config file

#### `-p --path <PATH>`

Specify a path to a config file or directory

If a directory is specified, it will look for `mise.toml` (default) or `.tool-versions` if `MISE_ASDF_COMPAT=1`

#### `--pin`

Save exact version to config file
e.g.: `mise use --pin node@20` will save 20.0.0 as the version
Set `MISE_PIN=1` or `MISE_ASDF_COMPAT=1` to make this the default behavior

Examples:

    # set the current version of node to 20.x in mise.toml of current directory
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

- **Usage**: `mise version`
- **Aliases**: `v`

Display the version of mise

Displays the version, os, architecture, and the date of the build.

If the version is out of date, it will display a warning.

Examples:

    $ mise version
    $ mise --version
    $ mise -v
    $ mise -V

## `mise watch`

- **Usage**: `mise watch [-t --task... <TASK>] [-g --glob... <GLOB>] [ARGS]...`
- **Aliases**: `w`

[experimental] Run task(s) and watch for changes to rerun it

This command uses the `watchexec` tool to watch for changes to files and rerun the specified task(s).
It must be installed for this command to work, but you can install it with `mise use -g watchexec@latest`.

### Arguments

#### `[ARGS]...`

Extra arguments

### Flags

#### `-t --task... <TASK>`

Tasks to run

#### `-g --glob... <GLOB>`

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

- **Usage**: `mise where <TOOL@VERSION>`

Display the installation path for a tool

The tool must be installed for this to work.

### Arguments

#### `<TOOL@VERSION>`

Tool(s) to look up
e.g.: ruby@3
if "@<PREFIX>" is specified, it will show the latest installed version
that matches the prefix
otherwise, it will show the current, active installed version

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

- **Usage**: `mise which [FLAGS] <BIN_NAME>`

Shows the path that a tool's bin points to.

Use this to figure out what version of a tool is currently active.

### Arguments

#### `<BIN_NAME>`

The bin to look up

### Flags

#### `--plugin`

Show the plugin name instead of the path

#### `--version`

Show the version instead of the path

#### `-t --tool <TOOL@VERSION>`

Use a specific tool@version
e.g.: `mise which npm --tool=node@20`

Examples:

    $ mise which node
    /home/username/.local/share/mise/installs/node/20.0.0/bin/node

    $ mise which node --plugin
    node

    $ mise which node --version
    20.0.0

## `mise render-help`

- **Usage**: `mise render-help`

internal command to generate markdown from help

## `mise render-mangen`

- **Usage**: `mise render-mangen`

internal command to generate markdown from help
