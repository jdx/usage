# `mise`


mise is a tool for managing runtime versions. https://github.com/jdx/mise

It's a replacement for tools like nvm, nodenv, rbenv, rvm, chruby, pyenv, etc.
that works for any language. It's also great for managing linters/tools like
jq and shellcheck.

It is inspired by asdf and uses asdf's plugin ecosystem under the hood:
https://asdf-vm.com/


- **Usage**: `mise [FLAGS] [TASK] <SUBCOMMAND>`

## Arguments

### `[TASK]`

Task to run.

Shorthand for `mise task run <TASK>`.

## Global Flags

### `-C --cd <DIR>`

Change directory before running command

### `-E --env… <ENV>`

Set the environment for loading `mise.<ENV>.toml`

### `-j --jobs <JOBS>`

How many jobs to run in parallel [default: 8]

### `--raw`

Read/write directly to stdin/stdout/stderr instead of by line

### `-y --yes`

Answer yes to all confirmation prompts

### `-q --quiet`

Suppress non-error messages

### `--silent`

Suppress all task output and mise non-error messages

### `-v --verbose…`

Show extra output (use -vv for even more)

## Flags

### `--output <OUTPUT>`

### `--no-config`

Do not load any config files

Can also use `MISE_NO_CONFIG=1`

## `mise activate`

- **Usage**: `mise activate [FLAGS] [SHELL_TYPE]`
- **Source code**: [`src/cli/activate.rs`](https://github.com/jdx/mise/blob/main/src/cli/activate.rs)

Initializes mise in the current shell session

This should go into your shell's rc file or login shell.
Otherwise, it will only take effect in the current session.
(e.g. ~/.zshrc, ~/.zprofile, ~/.zshenv, ~/.bashrc, ~/.bash_profile, ~/.profile, ~/.config/fish/config.fish)

Typically, this can be added with something like the following:

    echo 'eval "$(mise activate zsh)"' >> ~/.zshrc

However, this requires that "mise" is in your PATH. If it is not, you need to
specify the full path like this:

    echo 'eval "$(/path/to/mise activate zsh)"' >> ~/.zshrc

Customize status output with `status` settings.

### Arguments

#### `[SHELL_TYPE]`

Shell type to generate the script for

**Choices:**

- `bash`
- `elvish`
- `fish`
- `nu`
- `xonsh`
- `zsh`
- `pwsh`

### Flags

#### `--shims`

Use shims instead of modifying PATH
Effectively the same as:

    PATH="$HOME/.local/share/mise/shims:$PATH"

`mise activate --shims` does not support all the features of `mise activate`.
See https://mise.jdx.dev/dev-tools/shims.html#shims-vs-path for more information

#### `-q --quiet`

Suppress non-error messages

#### `--no-hook-env`

Do not automatically call hook-env

This can be helpful for debugging mise. If you run `eval "$(mise activate --no-hook-env)"`, then you can call `mise hook-env` manually which will output the env vars to stdout without actually modifying the environment. That way you can do things like `mise hook-env --trace` to get more information or just see the values that hook-env is outputting.

Examples:

    $ eval "$(mise activate bash)"
    $ eval "$(mise activate zsh)"
    $ mise activate fish | source
    $ execx($(mise activate xonsh))

## `mise alias`

- **Usage**: `mise alias [-p --plugin <PLUGIN>] [--no-header] <SUBCOMMAND>`
- **Aliases**: `a`
- **Source code**: [`src/cli/alias/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/alias/mod.rs)

Manage aliases

### Flags

#### `-p --plugin <PLUGIN>`

filter aliases by plugin

#### `--no-header`

Don't show table header

## `mise alias get`

- **Usage**: `mise alias get <PLUGIN> <ALIAS>`
- **Source code**: [`src/cli/alias/get.rs`](https://github.com/jdx/mise/blob/main/src/cli/alias/get.rs)

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

- **Usage**: `mise alias ls [--no-header] [TOOL]`
- **Aliases**: `list`
- **Source code**: [`src/cli/alias/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/alias/ls.rs)

List aliases
Shows the aliases that can be specified.
These can come from user config or from plugins in `bin/list-aliases`.

For user config, aliases are defined like the following in `~/.config/mise/config.toml`:

    [alias.node.versions]
    lts = "22.0.0"

### Arguments

#### `[TOOL]`

Show aliases for <TOOL>

### Flags

#### `--no-header`

Don't show table header

Examples:

    $ mise aliases
    node  lts-jod      22

## `mise alias set`

- **Usage**: `mise alias set <ARGS>…`
- **Aliases**: `add`, `create`
- **Source code**: [`src/cli/alias/set.rs`](https://github.com/jdx/mise/blob/main/src/cli/alias/set.rs)

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

    $ mise alias set node lts-jod 22.0.0

## `mise alias unset`

- **Usage**: `mise alias unset <PLUGIN> <ALIAS>`
- **Aliases**: `rm`, `remove`, `delete`, `del`
- **Source code**: [`src/cli/alias/unset.rs`](https://github.com/jdx/mise/blob/main/src/cli/alias/unset.rs)

Clears an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<PLUGIN>`

The plugin to remove the alias from

#### `<ALIAS>`

The alias to remove

Examples:

    $ mise alias unset node lts-jod

## `mise asdf`

- **Usage**: `mise asdf [ARGS]…`
- **Source code**: [`src/cli/asdf.rs`](https://github.com/jdx/mise/blob/main/src/cli/asdf.rs)

[internal] simulates asdf for plugins that call "asdf" internally

### Arguments

#### `[ARGS]…`

all arguments

## `mise backends`

- **Usage**: `mise backends <SUBCOMMAND>`
- **Aliases**: `b`
- **Source code**: [`src/cli/backends/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/backends/mod.rs)

Manage backends

## `mise backends ls`

- **Usage**: `mise backends ls`
- **Aliases**: `list`
- **Source code**: [`src/cli/backends/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/backends/ls.rs)

List built-in backends

Examples:

    $ mise backends ls
    aqua
    asdf
    cargo
    core
    dotnet
    gem
    go
    npm
    pipx
    spm
    ubi
    vfox

## `mise bin-paths`

- **Usage**: `mise bin-paths [TOOL@VERSION]…`
- **Source code**: [`src/cli/bin_paths.rs`](https://github.com/jdx/mise/blob/main/src/cli/bin_paths.rs)

List all the active runtime bin paths

### Arguments

#### `[TOOL@VERSION]…`

Tool(s) to look up
e.g.: ruby@3

## `mise cache`

- **Usage**: `mise cache <SUBCOMMAND>`
- **Source code**: [`src/cli/cache/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/cache/mod.rs)

Manage the mise cache

Run `mise cache` with no args to view the current cache directory.

## `mise cache clear`

- **Usage**: `mise cache clear [PLUGIN]…`
- **Aliases**: `c`
- **Source code**: [`src/cli/cache/clear.rs`](https://github.com/jdx/mise/blob/main/src/cli/cache/clear.rs)

Deletes all cache files in mise

### Arguments

#### `[PLUGIN]…`

Plugin(s) to clear cache for e.g.: node, python

## `mise cache prune`

- **Usage**: `mise cache prune [--dry-run] [-v --verbose…] [PLUGIN]…`
- **Aliases**: `p`
- **Source code**: [`src/cli/cache/prune.rs`](https://github.com/jdx/mise/blob/main/src/cli/cache/prune.rs)

Removes stale mise cache files

By default, this command will remove files that have not been accessed in 30 days.
Change this with the MISE_CACHE_PRUNE_AGE environment variable.

### Arguments

#### `[PLUGIN]…`

Plugin(s) to clear cache for e.g.: node, python

### Flags

#### `--dry-run`

Just show what would be pruned

#### `-v --verbose…`

Show pruned files

## `mise completion`

- **Usage**: `mise completion [--include-bash-completion-lib] [SHELL]`
- **Source code**: [`src/cli/completion.rs`](https://github.com/jdx/mise/blob/main/src/cli/completion.rs)

Generate shell completions

### Arguments

#### `[SHELL]`

Shell type to generate completions for

**Choices:**

- `bash`
- `fish`
- `zsh`

### Flags

#### `--include-bash-completion-lib`

Include the bash completion library in the bash completion script

This is required for completions to work in bash, but it is not included by default
you may source it separately or enable this flag to include it in the script.

Examples:

    $ mise completion bash > /etc/bash_completion.d/mise
    $ mise completion zsh  > /usr/local/share/zsh/site-functions/_mise
    $ mise completion fish > ~/.config/fish/completions/mise.fish

## `mise config`

- **Usage**: `mise config [--no-header] [-J --json] <SUBCOMMAND>`
- **Aliases**: `cfg`
- **Source code**: [`src/cli/config/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/config/mod.rs)

Manage config files

### Flags

#### `--no-header`

Do not print table header

#### `-J --json`

Output in JSON format

## `mise config generate`

- **Usage**: `mise config generate [-t --tool-versions <TOOL_VERSIONS>] [-o --output <OUTPUT>]`
- **Aliases**: `g`
- **Source code**: [`src/cli/config/generate.rs`](https://github.com/jdx/mise/blob/main/src/cli/config/generate.rs)

[experimental] Generate a mise.toml file

### Flags

#### `-t --tool-versions <TOOL_VERSIONS>`

Path to a .tool-versions file to import tools from

#### `-o --output <OUTPUT>`

Output to file instead of stdout

Examples:

    $ mise cf generate > mise.toml
    $ mise cf generate --output=mise.toml

## `mise config get`

- **Usage**: `mise config get [-f --file <FILE>] [KEY]`
- **Source code**: [`src/cli/config/get.rs`](https://github.com/jdx/mise/blob/main/src/cli/config/get.rs)

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
- **Aliases**: `list`
- **Source code**: [`src/cli/config/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/config/ls.rs)

List config files currently in use

### Flags

#### `--no-header`

Do not print table header

#### `-J --json`

Output in JSON format

Examples:

    $ mise config ls
    Path                        Tools
    ~/.config/mise/config.toml  pitchfork
    ~/src/mise/mise.toml        actionlint, bun, cargo-binstall, cargo:cargo-edit, cargo:cargo-insta

## `mise config set`

- **Usage**: `mise config set [-f --file <FILE>] [-t --type <TYPE>] <KEY> <VALUE>`
- **Source code**: [`src/cli/config/set.rs`](https://github.com/jdx/mise/blob/main/src/cli/config/set.rs)

Set the value of a setting in a mise.toml file

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

- `infer`
- `string`
- `integer`
- `float`
- `bool`
- `list`

Examples:

    $ mise config set tools.python 3.12
    $ mise config set settings.always_keep_download true
    $ mise config set env.TEST_ENV_VAR ABC
    $ mise config set settings.disable_tools --type list node,rust

    # Type for `settings` is inferred
    $ mise config set settings.jobs 4

## `mise current`

- **Usage**: `mise current [PLUGIN]`
- **Source code**: [`src/cli/current.rs`](https://github.com/jdx/mise/blob/main/src/cli/current.rs)

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
- **Source code**: [`src/cli/deactivate.rs`](https://github.com/jdx/mise/blob/main/src/cli/deactivate.rs)

Disable mise for current shell session

This can be used to temporarily disable mise in a shell session.

Examples:

    $ mise deactivate

## `mise direnv`

- **Usage**: `mise direnv <SUBCOMMAND>`
- **Source code**: [`src/cli/direnv/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/direnv/mod.rs)

Output direnv function to use mise inside direnv

See https://mise.jdx.dev/direnv.html for more information

Because this generates the idiomatic files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when idiomatic file versions change.

## `mise direnv envrc`

- **Usage**: `mise direnv envrc`
- **Source code**: [`src/cli/direnv/envrc.rs`](https://github.com/jdx/mise/blob/main/src/cli/direnv/envrc.rs)

[internal] This is an internal command that writes an envrc file
for direnv to consume.

## `mise direnv exec`

- **Usage**: `mise direnv exec`
- **Source code**: [`src/cli/direnv/exec.rs`](https://github.com/jdx/mise/blob/main/src/cli/direnv/exec.rs)

[internal] This is an internal command that writes an envrc file
for direnv to consume.

## `mise direnv activate`

- **Usage**: `mise direnv activate`
- **Source code**: [`src/cli/direnv/activate.rs`](https://github.com/jdx/mise/blob/main/src/cli/direnv/activate.rs)

Output direnv function to use mise inside direnv

See https://mise.jdx.dev/direnv.html for more information

Because this generates the idiomatic files based on currently installed plugins,
you should run this command after installing new plugins. Otherwise
direnv may not know to update environment variables when idiomatic file versions change.

Examples:

    $ mise direnv activate > ~/.config/direnv/lib/use_mise.sh
    $ echo 'use mise' > .envrc
    $ direnv allow

## `mise doctor`

- **Usage**: `mise doctor [-J --json] <SUBCOMMAND>`
- **Aliases**: `dr`
- **Source code**: [`src/cli/doctor/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/doctor/mod.rs)

Check mise installation for possible problems

### Flags

#### `-J --json`

Examples:

    $ mise doctor
    [WARN] plugin node is not installed

## `mise doctor path`

- **Usage**: `mise doctor path [-f --full]`
- **Source code**: [`src/cli/doctor/path.rs`](https://github.com/jdx/mise/blob/main/src/cli/doctor/path.rs)

Print the current PATH entries mise is providing

### Flags

#### `-f --full`

Print all entries including those not provided by mise

Examples:

    Get the current PATH entries mise is providing
    $ mise path
    /home/user/.local/share/mise/installs/node/24.0.0/bin
    /home/user/.local/share/mise/installs/rust/1.90.0/bin
    /home/user/.local/share/mise/installs/python/3.10.0/bin

## `mise en`

- **Usage**: `mise en [-s --shell <SHELL>] [DIR]`
- **Source code**: [`src/cli/en.rs`](https://github.com/jdx/mise/blob/main/src/cli/en.rs)

[experimental] starts a new shell with the mise environment built from the current configuration

This is an alternative to `mise activate` that allows you to explicitly start a mise session.
It will have the tools and environment variables in the configs loaded.
Note that changing directories will not update the mise environment.

### Arguments

#### `[DIR]`

Directory to start the shell in

**Default:** `.`

### Flags

#### `-s --shell <SHELL>`

Shell to start

Defaults to $SHELL

Examples:

    $ mise en .
    $ node -v
    v20.0.0

    Skip loading bashrc:
    $ mise en -s "bash --norc"

    Skip loading zshrc:
    $ mise en -s "zsh -f"

## `mise env`

- **Usage**: `mise env [FLAGS] [TOOL@VERSION]…`
- **Aliases**: `e`
- **Source code**: [`src/cli/env.rs`](https://github.com/jdx/mise/blob/main/src/cli/env.rs)

Exports env vars to activate mise a single time

Use this if you don't want to permanently install mise. It's not necessary to
use this if you have `mise activate` in your shell rc file.

### Arguments

#### `[TOOL@VERSION]…`

Tool(s) to use

### Flags

#### `-J --json`

Output in JSON format

#### `--json-extended`

Output in JSON format with additional information (source, tool)

#### `-D --dotenv`

Output in dotenv format

#### `-s --shell <SHELL>`

Shell type to generate environment variables for

**Choices:**

- `bash`
- `elvish`
- `fish`
- `nu`
- `xonsh`
- `zsh`
- `pwsh`

Examples:

    $ eval "$(mise env -s bash)"
    $ eval "$(mise env -s zsh)"
    $ mise env -s fish | source
    $ execx($(mise env -s xonsh))

## `mise exec`

- **Usage**: `mise exec [FLAGS] [TOOL@VERSION]… [-- COMMAND]…`
- **Aliases**: `x`
- **Source code**: [`src/cli/exec.rs`](https://github.com/jdx/mise/blob/main/src/cli/exec.rs)

Execute a command with tool(s) set

use this to avoid modifying the shell session or running ad-hoc commands with mise tools set.

Tools will be loaded from mise.toml, though they can be overridden with <RUNTIME> args
Note that only the plugin specified will be overridden, so if a `mise.toml` file
includes "node 20" but you run `mise exec python@3.11`; it will still load node@20.

The "--" separates runtimes from the commands to pass along to the subprocess.

### Arguments

#### `[TOOL@VERSION]…`

Tool(s) to start e.g.: node@20 python@3.10

#### `[-- COMMAND]…`

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

## `mise fmt`

- **Usage**: `mise fmt [-a --all]`
- **Source code**: [`src/cli/fmt.rs`](https://github.com/jdx/mise/blob/main/src/cli/fmt.rs)

Formats mise.toml

Sorts keys and cleans up whitespace in mise.toml

### Flags

#### `-a --all`

Format all files from the current directory

Examples:

    $ mise fmt

## `mise generate`

- **Usage**: `mise generate <SUBCOMMAND>`
- **Aliases**: `gen`
- **Source code**: [`src/cli/generate/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/mod.rs)

[experimental] Generate files for various tools/services

## `mise generate bootstrap`

- **Usage**: `mise generate bootstrap [FLAGS]`
- **Source code**: [`src/cli/generate/bootstrap.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/bootstrap.rs)

[experimental] Generate a script to download+execute mise

This is designed to be used in a project where contributors may not have mise installed.

### Flags

#### `-l --localize`

Sandboxes mise internal directories like MISE_DATA_DIR and MISE_CACHE_DIR into a `.mise` directory in the project

This is necessary if users may use a different version of mise outside the project.

#### `--localized-dir <LOCALIZED_DIR>`

Directory to put localized data into

#### `-V --version <VERSION>`

Specify mise version to fetch

#### `-w --write <WRITE>`

instead of outputting the script to stdout, write to a file and make it executable

Examples:

    $ mise generate bootstrap >./bin/mise
    $ chmod +x ./bin/mise
    $ ./bin/mise install – automatically downloads mise to .mise if not already installed

## `mise generate config`

- **Usage**: `mise generate config [-t --tool-versions <TOOL_VERSIONS>] [-o --output <OUTPUT>]`
- **Aliases**: `g`
- **Source code**: [`src/cli/generate/config.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/config.rs)

[experimental] Generate a mise.toml file

### Flags

#### `-t --tool-versions <TOOL_VERSIONS>`

Path to a .tool-versions file to import tools from

#### `-o --output <OUTPUT>`

Output to file instead of stdout

Examples:

    $ mise cf generate > mise.toml
    $ mise cf generate --output=mise.toml

## `mise generate git-pre-commit`

- **Usage**: `mise generate git-pre-commit [FLAGS]`
- **Aliases**: `pre-commit`
- **Source code**: [`src/cli/generate/git_pre_commit.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/git_pre_commit.rs)

[experimental] Generate a git pre-commit hook

This command generates a git pre-commit hook that runs a mise task like `mise run pre-commit`
when you commit changes to your repository.

Staged files are passed to the task as `STAGED`.

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
- **Source code**: [`src/cli/generate/github_action.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/github_action.rs)

[experimental] Generate a GitHub Action workflow file

This command generates a GitHub Action workflow file that runs a mise task like `mise run ci`
when you push changes to your repository.

### Flags

#### `--name <NAME>`

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
- **Source code**: [`src/cli/generate/task_docs.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/task_docs.rs)

Generate documentation for tasks in a project

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

## `mise generate task-stubs`

- **Usage**: `mise generate task-stubs [-m --mise-bin <MISE_BIN>] [-d --dir <DIR>]`
- **Source code**: [`src/cli/generate/task_stubs.rs`](https://github.com/jdx/mise/blob/main/src/cli/generate/task_stubs.rs)

[experimental] Generates shims to run mise tasks

By default, this will build shims like ./bin/<task>. These can be paired with `mise generate bootstrap`
so contributors to a project can execute mise tasks without installing mise into their system.

### Flags

#### `-m --mise-bin <MISE_BIN>`

Path to a mise bin to use when running the task stub.

Use `--mise-bin=./bin/mise` to use a mise bin generated from `mise generate bootstrap`

#### `-d --dir <DIR>`

Directory to create task stubs inside of

Examples:

    $ mise task add test -- echo 'running tests'
    $ mise generate task-stubs
    $ ./bin/test
    running tests

## `mise global`

- **Usage**: `mise global [FLAGS] [TOOL@VERSION]…`
- **Source code**: [`src/cli/global.rs`](https://github.com/jdx/mise/blob/main/src/cli/global.rs)

Sets/gets the global tool version(s)

Displays the contents of global config after writing.
The file is `$HOME/.config/mise/config.toml` by default. It can be changed with `$MISE_GLOBAL_CONFIG_FILE`.
If `$MISE_GLOBAL_CONFIG_FILE` is set to anything that ends in `.toml`, it will be parsed as `mise.toml`.
Otherwise, it will be parsed as a `.tool-versions` file.

Use MISE_ASDF_COMPAT=1 to default the global config to ~/.tool-versions

Use `mise local` to set a tool version locally in the current directory.

### Arguments

#### `[TOOL@VERSION]…`

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

#### `--remove… <PLUGIN>`

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

- **Usage**: `mise hook-env [FLAGS]`
- **Source code**: [`src/cli/hook_env.rs`](https://github.com/jdx/mise/blob/main/src/cli/hook_env.rs)

[internal] called by activate hook to update env vars directory change

### Flags

#### `-s --shell <SHELL>`

Shell type to generate script for

**Choices:**

- `bash`
- `elvish`
- `fish`
- `nu`
- `xonsh`
- `zsh`
- `pwsh`

#### `-f --force`

Skip early exit check

#### `-q --quiet`

Hide warnings such as when a tool is not installed

## `mise hook-not-found`

- **Usage**: `mise hook-not-found [-s --shell <SHELL>] <BIN>`
- **Source code**: [`src/cli/hook_not_found.rs`](https://github.com/jdx/mise/blob/main/src/cli/hook_not_found.rs)

[internal] called by shell when a command is not found

### Arguments

#### `<BIN>`

Attempted bin to run

### Flags

#### `-s --shell <SHELL>`

Shell type to generate script for

**Choices:**

- `bash`
- `elvish`
- `fish`
- `nu`
- `xonsh`
- `zsh`
- `pwsh`

## `mise implode`

- **Usage**: `mise implode [--config] [-n --dry-run]`
- **Source code**: [`src/cli/implode.rs`](https://github.com/jdx/mise/blob/main/src/cli/implode.rs)

Removes mise CLI and all related data

Skips config directory by default.

### Flags

#### `--config`

Also remove config directory

#### `-n --dry-run`

List directories that would be removed without actually removing them

## `mise install`

- **Usage**: `mise install [FLAGS] [TOOL@VERSION]…`
- **Aliases**: `i`
- **Source code**: [`src/cli/install.rs`](https://github.com/jdx/mise/blob/main/src/cli/install.rs)

Install a tool version

Installs a tool version to `~/.local/share/mise/installs/<PLUGIN>/<VERSION>`
Installing alone will not activate the tools so they won't be in PATH.
To install and/or activate in one command, use `mise use` which will create a `mise.toml` file
in the current directory to activate this tool when inside the directory.
Alternatively, run `mise exec <TOOL>@<VERSION> -- <COMMAND>` to execute a tool without creating config files.

Tools will be installed in parallel. To disable, set `--jobs=1` or `MISE_JOBS=1`

### Arguments

#### `[TOOL@VERSION]…`

Tool(s) to install e.g.: node@20

### Flags

#### `-f --force`

Force reinstall even if already installed

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

#### `-v --verbose…`

Show installation output

This argument will print plugin output such as download, configuration, and compilation output.

Examples:

    $ mise install node@20.0.0  # install specific node version
    $ mise install node@20      # install fuzzy node version
    $ mise install node         # install version specified in mise.toml
    $ mise install              # installs everything specified in mise.toml

## `mise install-into`

- **Usage**: `mise install-into <TOOL@VERSION> <PATH>`
- **Source code**: [`src/cli/install_into.rs`](https://github.com/jdx/mise/blob/main/src/cli/install_into.rs)

Install a tool version to a specific path

Used for building a tool to a directory for use outside of mise

### Arguments

#### `<TOOL@VERSION>`

Tool to install e.g.: node@20

#### `<PATH>`

Path to install the tool into

Examples:

    # install node@20.0.0 into ./mynode
    $ mise install-into node@20.0.0 ./mynode && ./mynode/bin/node -v
    20.0.0

## `mise latest`

- **Usage**: `mise latest [-i --installed] <TOOL@VERSION>`
- **Source code**: [`src/cli/latest.rs`](https://github.com/jdx/mise/blob/main/src/cli/latest.rs)

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
- **Source code**: [`src/cli/link.rs`](https://github.com/jdx/mise/blob/main/src/cli/link.rs)

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

- **Usage**: `mise local [FLAGS] [TOOL@VERSION]…`
- **Source code**: [`src/cli/local.rs`](https://github.com/jdx/mise/blob/main/src/cli/local.rs)

Sets/gets tool version in local .tool-versions or mise.toml

Use this to set a tool's version when within a directory
Use `mise global` to set a tool version globally
This uses `.tool-version` by default unless there is a `mise.toml` file or if `MISE_USE_TOML`
is set. A future v2 release of mise will default to using `mise.toml`.

### Arguments

#### `[TOOL@VERSION]…`

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

#### `--remove… <PLUGIN>`

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

- **Usage**: `mise ls [FLAGS] [INSTALLED_TOOL]…`
- **Aliases**: `list`
- **Source code**: [`src/cli/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/ls.rs)

List installed and active tool versions

This command lists tools that mise "knows about".
These may be tools that are currently installed, or those
that are in a config file (active) but may or may not be installed.

It's a useful command to get the current state of your tools.

### Arguments

#### `[INSTALLED_TOOL]…`

Only show tool versions from [TOOL]

### Flags

#### `-c --current`

Only show tool versions currently specified in a mise.toml

#### `-g --global`

Only show tool versions currently specified in the global mise.toml

#### `-i --installed`

Only show tool versions that are installed (Hides tools defined in mise.toml but not installed)

#### `-o --offline`

Don't fetch information such as outdated versions

#### `--outdated`

Display whether a version is outdated

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
- **Source code**: [`src/cli/ls_remote.rs`](https://github.com/jdx/mise/blob/main/src/cli/ls_remote.rs)

List runtime versions available for install.

Note that the results may be cached, run `mise cache clean` to clear the cache and get fresh results.

### Arguments

#### `[TOOL@VERSION]`

Tool to get versions for

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

- **Usage**: `mise outdated [FLAGS] [TOOL@VERSION]…`
- **Source code**: [`src/cli/outdated.rs`](https://github.com/jdx/mise/blob/main/src/cli/outdated.rs)

Shows outdated tool versions

See `mise upgrade` to upgrade these versions.

### Arguments

#### `[TOOL@VERSION]…`

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
- **Source code**: [`src/cli/plugins/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/mod.rs)

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
- **Source code**: [`src/cli/plugins/install.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/install.rs)

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

#### `-v --verbose…`

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

- **Usage**: `mise plugins link [-f --force] <NAME> [DIR]`
- **Aliases**: `ln`
- **Source code**: [`src/cli/plugins/link.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/link.rs)

Symlinks a plugin into mise

This is used for developing a plugin.

### Arguments

#### `<NAME>`

The name of the plugin
e.g.: node, ruby

#### `[DIR]`

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

- **Usage**: `mise plugins ls [-u --urls]`
- **Aliases**: `list`
- **Source code**: [`src/cli/plugins/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/ls.rs)

List installed plugins

Can also show remotely available plugins to install.

### Flags

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
- **Source code**: [`src/cli/plugins/ls_remote.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/ls_remote.rs)


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

- **Usage**: `mise plugins uninstall [-p --purge] [-a --all] [PLUGIN]…`
- **Aliases**: `remove`, `rm`
- **Source code**: [`src/cli/plugins/uninstall.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/uninstall.rs)

Removes a plugin

### Arguments

#### `[PLUGIN]…`

Plugin(s) to remove

### Flags

#### `-p --purge`

Also remove the plugin's installs, downloads, and cache

#### `-a --all`

Remove all plugins

Examples:

    $ mise uninstall node

## `mise plugins update`

- **Usage**: `mise plugins update [-j --jobs <JOBS>] [PLUGIN]…`
- **Aliases**: `up`, `upgrade`
- **Source code**: [`src/cli/plugins/update.rs`](https://github.com/jdx/mise/blob/main/src/cli/plugins/update.rs)

Updates a plugin to the latest version

note: this updates the plugin itself, not the runtime versions

### Arguments

#### `[PLUGIN]…`

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

- **Usage**: `mise prune [FLAGS] [INSTALLED_TOOL]…`
- **Source code**: [`src/cli/prune.rs`](https://github.com/jdx/mise/blob/main/src/cli/prune.rs)

Delete unused versions of tools

mise tracks which config files have been used in ~/.local/state/mise/tracked-configs
Versions which are no longer the latest specified in any of those configs are deleted.
Versions installed only with environment variables `MISE_<PLUGIN>_VERSION` will be deleted,
as will versions only referenced on the command line `mise exec <PLUGIN>@<VERSION>`.

### Arguments

#### `[INSTALLED_TOOL]…`

Prune only these tools

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

- **Usage**: `mise registry [-b --backend <BACKEND>] [--hide-aliased] [NAME]`
- **Source code**: [`src/cli/registry.rs`](https://github.com/jdx/mise/blob/main/src/cli/registry.rs)

List available tools to install

This command lists the tools available in the registry as shorthand names.

For example, `poetry` is shorthand for `asdf:mise-plugins/mise-poetry`.

### Arguments

#### `[NAME]`

Show only the specified tool's full name

### Flags

#### `-b --backend <BACKEND>`

Show only tools for this backend

#### `--hide-aliased`

Hide aliased tools

Examples:

    $ mise registry
    node    core:node
    poetry  asdf:mise-plugins/mise-poetry
    ubi     cargo:ubi-cli

    $ mise registry poetry
    asdf:mise-plugins/mise-poetry

## `mise reshim`

- **Usage**: `mise reshim [-f --force]`
- **Source code**: [`src/cli/reshim.rs`](https://github.com/jdx/mise/blob/main/src/cli/reshim.rs)

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
- **Source code**: [`src/cli/run.rs`](https://github.com/jdx/mise/blob/main/src/cli/run.rs)

Run task(s)

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

#### `-c --continue-on-error`

Continue running tasks even if one fails

#### `-n --dry-run`

Don't actually run the tasks(s), just print them in order of execution

#### `-f --force`

Force the tasks to run even if outputs are up to date

#### `-s --shell <SHELL>`

Shell to use to run toml tasks

Defaults to `sh -c -o errexit -o pipefail` on unix, and `cmd /c` on Windows
Can also be set with the setting `MISE_UNIX_DEFAULT_INLINE_SHELL_ARGS` or `MISE_WINDOWS_DEFAULT_INLINE_SHELL_ARGS`
Or it can be overridden with the `shell` property on a task.

#### `-t --tool… <TOOL@VERSION>`

Tool(s) to run in addition to what is in mise.toml files e.g.: node@20 python@3.10

#### `-j --jobs <JOBS>`

Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var

#### `-r --raw`

Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

#### `--no-timings`

Hides elapsed time after each task completes

Default to always hide with `MISE_TASK_TIMINGS=0`

#### `-q --quiet`

Don't show extra output

#### `-S --silent`

Don't show any output except for errors

#### `-o --output <OUTPUT>`

Change how tasks information is output when running tasks

- `prefix` - Print stdout/stderr by line, prefixed with the task's label
- `interleave` - Print directly to stdout/stderr instead of by line
- `replacing` - Stdout is replaced each time, stderr is printed as is
- `timed` - Only show stdout lines if they are displayed for more than 1 second
- `keep-order` - Print stdout/stderr by line, prefixed with the task's label, but keep the order of the output
- `quiet` - Don't show extra output
- `silent` - Don't show any output including stdout and stderr from the task except for errors

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
- **Source code**: [`src/cli/self_update.rs`](https://github.com/jdx/mise/blob/main/src/cli/self_update.rs)

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

- **Usage**: `mise set [--file <FILE>] [-g --global] [ENV_VAR]…`
- **Source code**: [`src/cli/set.rs`](https://github.com/jdx/mise/blob/main/src/cli/set.rs)

Set environment variables in mise.toml

By default, this command modifies `mise.toml` in the current directory.

### Arguments

#### `[ENV_VAR]…`

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

- **Usage**: `mise settings [FLAGS] [SETTING] [VALUE] <SUBCOMMAND>`
- **Source code**: [`src/cli/settings/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/settings/mod.rs)

Show current settings

This is the contents of ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases`

### Arguments

#### `[SETTING]`

Name of setting

#### `[VALUE]`

Setting value to set

### Global Flags

#### `-l --local`

Use the local config file instead of the global one

### Flags

#### `-a --all`

List all settings

#### `-J --json`

Output in JSON format

#### `--json-extended`

Output in JSON format with sources

#### `-T --toml`

Output in TOML format

Examples:
    # list all settings
    $ mise settings

    # get the value of the setting "always_keep_download"
    $ mise settings always_keep_download

    # set the value of the setting "always_keep_download" to "true"
    $ mise settings always_keep_download=true

    # set the value of the setting "node.mirror_url" to "https://npm.taobao.org/mirrors/node"
    $ mise settings node.mirror_url https://npm.taobao.org/mirrors/node

## `mise settings add`

- **Usage**: `mise settings add [-l --local] <SETTING> <VALUE>`
- **Source code**: [`src/cli/settings/add.rs`](https://github.com/jdx/mise/blob/main/src/cli/settings/add.rs)

Adds a setting to the configuration file

Used with an array setting, this will append the value to the array.
This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<SETTING>`

The setting to set

#### `<VALUE>`

The value to set

### Flags

#### `-l --local`

Use the local config file instead of the global one

Examples:

    $ mise settings add disable_hints python_multi

## `mise settings get`

- **Usage**: `mise settings get [-l --local] <SETTING>`
- **Source code**: [`src/cli/settings/get.rs`](https://github.com/jdx/mise/blob/main/src/cli/settings/get.rs)

Show a current setting

This is the contents of a single entry in ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases get`

### Arguments

#### `<SETTING>`

The setting to show

### Flags

#### `-l --local`

Use the local config file instead of the global one

Examples:

    $ mise settings get idiomatic_version_file
    true

## `mise settings ls`

- **Usage**: `mise settings ls [FLAGS] [SETTING]`
- **Aliases**: `list`
- **Source code**: [`src/cli/settings/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/settings/ls.rs)

Show current settings

This is the contents of ~/.config/mise/config.toml

Note that aliases are also stored in this file
but managed separately with `mise aliases`

### Arguments

#### `[SETTING]`

Name of setting

### Flags

#### `-a --all`

List all settings

#### `-l --local`

Use the local config file instead of the global one

#### `-J --json`

Output in JSON format

#### `--json-extended`

Output in JSON format with sources

#### `-T --toml`

Output in TOML format

Examples:

    $ mise settings ls
    idiomatic_version_file = false
    ...

    $ mise settings ls python
    default_packages_file = "~/.default-python-packages"
    ...

## `mise settings set`

- **Usage**: `mise settings set [-l --local] <SETTING> <VALUE>`
- **Aliases**: `create`
- **Source code**: [`src/cli/settings/set.rs`](https://github.com/jdx/mise/blob/main/src/cli/settings/set.rs)

Add/update a setting

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<SETTING>`

The setting to set

#### `<VALUE>`

The value to set

### Flags

#### `-l --local`

Use the local config file instead of the global one

Examples:

    $ mise settings idiomatic_version_file=true

## `mise settings unset`

- **Usage**: `mise settings unset [-l --local] <KEY>`
- **Aliases**: `rm`, `remove`, `delete`, `del`
- **Source code**: [`src/cli/settings/unset.rs`](https://github.com/jdx/mise/blob/main/src/cli/settings/unset.rs)

Clears a setting

This modifies the contents of ~/.config/mise/config.toml

### Arguments

#### `<KEY>`

The setting to remove

### Flags

#### `-l --local`

Use the local config file instead of the global one

Examples:

    $ mise settings unset idiomatic_version_file

## `mise shell`

- **Usage**: `mise shell [FLAGS] <TOOL@VERSION>…`
- **Aliases**: `sh`
- **Source code**: [`src/cli/shell.rs`](https://github.com/jdx/mise/blob/main/src/cli/shell.rs)

Sets a tool version for the current session.

Only works in a session where mise is already activated.

This works by setting environment variables for the current shell session
such as `MISE_NODE_VERSION=20` which is "eval"ed as a shell function created by `mise activate`.

### Arguments

#### `<TOOL@VERSION>…`

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
- **Source code**: [`src/cli/sync/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/sync/mod.rs)

Synchronize tools from other version managers with mise

## `mise sync node`

- **Usage**: `mise sync node [FLAGS]`
- **Source code**: [`src/cli/sync/node.rs`](https://github.com/jdx/mise/blob/main/src/cli/sync/node.rs)

Symlinks all tool versions from an external tool into mise

For example, use this to import all Homebrew node installs into mise

This won't overwrite any existing installs but will overwrite any existing symlinks

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

- **Usage**: `mise sync python [--pyenv] [--uv]`
- **Source code**: [`src/cli/sync/python.rs`](https://github.com/jdx/mise/blob/main/src/cli/sync/python.rs)

Symlinks all tool versions from an external tool into mise

For example, use this to import all pyenv installs into mise

This won't overwrite any existing installs but will overwrite any existing symlinks

### Flags

#### `--pyenv`

Get tool versions from pyenv

#### `--uv`

Sync tool versions with uv (2-way sync)

Examples:

    $ pyenv install 3.11.0
    $ mise sync python --pyenv
    $ mise use -g python@3.11.0 - uses pyenv-provided python
    
    $ uv python install 3.11.0
    $ mise install python@3.10.0
    $ mise sync python --uv
    $ mise x python@3.11.0 -- python -V - uses uv-provided python
    $ uv run -p 3.10.0 -- python -V - uses mise-provided python

## `mise sync ruby`

- **Usage**: `mise sync ruby [--brew]`
- **Source code**: [`src/cli/sync/ruby.rs`](https://github.com/jdx/mise/blob/main/src/cli/sync/ruby.rs)

Symlinks all ruby tool versions from an external tool into mise

### Flags

#### `--brew`

Get tool versions from Homebrew

Examples:

    $ brew install ruby
    $ mise sync ruby --brew
    $ mise use -g ruby - Use the latest version of Ruby installed by Homebrew

## `mise tasks`

- **Usage**: `mise tasks [FLAGS] [TASK] <SUBCOMMAND>`
- **Aliases**: `t`
- **Source code**: [`src/cli/tasks/mod.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/mod.rs)

Manage tasks

### Arguments

#### `[TASK]`

Task name to get info of

### Global Flags

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

## `mise tasks add`

- **Usage**: `mise tasks add [FLAGS] <TASK> [-- RUN]…`
- **Source code**: [`src/cli/tasks/add.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/add.rs)

Create a new task

### Arguments

#### `<TASK>`

Tasks name to add

#### `[-- RUN]…`

### Flags

#### `--description <DESCRIPTION>`

Description of the task

#### `-a --alias… <ALIAS>`

Other names for the task

#### `--depends-post… <DEPENDS_POST>`

Dependencies to run after the task runs

#### `-w --wait-for… <WAIT_FOR>`

Wait for these tasks to complete if they are to run

#### `-D --dir <DIR>`

Run the task in a specific directory

#### `-H --hide`

Hide the task from `mise task` and completions

#### `-r --raw`

Directly connect stdin/stdout/stderr

#### `-s --sources… <SOURCES>`

Glob patterns of files this task uses as input

#### `--outputs… <OUTPUTS>`

Glob patterns of files this task creates, to skip if they are not modified

#### `--shell <SHELL>`

Run the task in a specific shell

#### `-q --quiet`

Do not print the command before running

#### `--silent`

Do not print the command or its output

#### `-d --depends… <DEPENDS>`

Add dependencies to the task

#### `--run-windows <RUN_WINDOWS>`

Command to run on windows

#### `-f --file`

Create a file task instead of a toml task

Examples:

    $ mise task add pre-commit --depends "test" --depends "render" -- echo pre-commit

## `mise tasks deps`

- **Usage**: `mise tasks deps [--hidden] [--dot] [TASKS]…`
- **Source code**: [`src/cli/tasks/deps.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/deps.rs)

Display a tree visualization of a dependency graph

### Arguments

#### `[TASKS]…`

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
- **Source code**: [`src/cli/tasks/edit.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/edit.rs)

Edit a tasks with $EDITOR

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
- **Source code**: [`src/cli/tasks/info.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/info.rs)

Get information about a task

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
- **Source code**: [`src/cli/tasks/ls.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/ls.rs)

List available tasks to execute
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

- **Usage**: `mise tasks run [FLAGS] [TASK] [ARGS]…`
- **Aliases**: `r`
- **Source code**: [`src/cli/tasks/run.rs`](https://github.com/jdx/mise/blob/main/src/cli/tasks/run.rs)

Run task(s)

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

#### `[ARGS]…`

Arguments to pass to the tasks. Use ":::" to separate tasks

### Flags

#### `-C --cd <CD>`

Change to this directory before executing the command

#### `-c --continue-on-error`

Continue running tasks even if one fails

#### `-n --dry-run`

Don't actually run the tasks(s), just print them in order of execution

#### `-f --force`

Force the tasks to run even if outputs are up to date

#### `-s --shell <SHELL>`

Shell to use to run toml tasks

Defaults to `sh -c -o errexit -o pipefail` on unix, and `cmd /c` on Windows
Can also be set with the setting `MISE_UNIX_DEFAULT_INLINE_SHELL_ARGS` or `MISE_WINDOWS_DEFAULT_INLINE_SHELL_ARGS`
Or it can be overridden with the `shell` property on a task.

#### `-t --tool… <TOOL@VERSION>`

Tool(s) to run in addition to what is in mise.toml files e.g.: node@20 python@3.10

#### `-j --jobs <JOBS>`

Number of tasks to run in parallel
[default: 4]
Configure with `jobs` config or `MISE_JOBS` env var

#### `-r --raw`

Read/write directly to stdin/stdout/stderr instead of by line
Configure with `raw` config or `MISE_RAW` env var

#### `--no-timings`

Hides elapsed time after each task completes

Default to always hide with `MISE_TASK_TIMINGS=0`

#### `-q --quiet`

Don't show extra output

#### `-S --silent`

Don't show any output except for errors

#### `-o --output <OUTPUT>`

Change how tasks information is output when running tasks

- `prefix` - Print stdout/stderr by line, prefixed with the task's label
- `interleave` - Print directly to stdout/stderr instead of by line
- `replacing` - Stdout is replaced each time, stderr is printed as is
- `timed` - Only show stdout lines if they are displayed for more than 1 second
- `keep-order` - Print stdout/stderr by line, prefixed with the task's label, but keep the order of the output
- `quiet` - Don't show extra output
- `silent` - Don't show any output including stdout and stderr from the task except for errors

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

## `mise test-tool`

- **Usage**: `mise test-tool [FLAGS] [TOOL]`
- **Source code**: [`src/cli/test_tool.rs`](https://github.com/jdx/mise/blob/main/src/cli/test_tool.rs)

Test a tool installs and executes

### Arguments

#### `[TOOL]`

### Flags

#### `-a --all`

#### `--include-non-defined`

Also test tools not defined in registry.toml, guessing how to test it

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets --jobs=1

Examples:

    $ mise test-tool ripgrep

## `mise tool`

- **Usage**: `mise tool [FLAGS] <TOOL>`
- **Source code**: [`src/cli/tool.rs`](https://github.com/jdx/mise/blob/main/src/cli/tool.rs)

Gets information about a tool

### Arguments

#### `<TOOL>`

Tool name to get information about

### Flags

#### `-J --json`

Output in JSON format

#### `--backend`

Only show backend field

#### `--description`

Only show description field

#### `--installed`

Only show installed versions

#### `--active`

Only show active versions

#### `--requested`

Only show requested versions

#### `--config-source`

Only show config source

#### `--tool-options`

Only show tool options

Examples:

    $ mise tool node
    Backend:            core
    Installed Versions: 20.0.0 22.0.0
    Active Version:     20.0.0
    Requested Version:  20
    Config Source:      ~/.config/mise/mise.toml
    Tool Options:       [none]

## `mise trust`

- **Usage**: `mise trust [FLAGS] [CONFIG_FILE]`
- **Source code**: [`src/cli/trust.rs`](https://github.com/jdx/mise/blob/main/src/cli/trust.rs)

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

#### `--ignore`

Do not trust this config and ignore it in the future

#### `--untrust`

No longer trust this config, will prompt in the future

#### `--show`

Show the trusted status of config files from the current directory and its parents.
Does not trust or untrust any files.

Examples:

    # trusts ~/some_dir/mise.toml
    $ mise trust ~/some_dir/mise.toml

    # trusts mise.toml in the current or parent directory
    $ mise trust

## `mise uninstall`

- **Usage**: `mise uninstall [-a --all] [-n --dry-run] [INSTALLED_TOOL@VERSION]…`
- **Source code**: [`src/cli/uninstall.rs`](https://github.com/jdx/mise/blob/main/src/cli/uninstall.rs)

Removes installed tool versions

This only removes the installed version, it does not modify mise.toml.

### Arguments

#### `[INSTALLED_TOOL@VERSION]…`

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

- **Usage**: `mise unset [-f --file <FILE>] [-g --global] [ENV_KEY]…`
- **Source code**: [`src/cli/unset.rs`](https://github.com/jdx/mise/blob/main/src/cli/unset.rs)

Remove environment variable(s) from the config file.

By default, this command modifies `mise.toml` in the current directory.

### Arguments

#### `[ENV_KEY]…`

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

## `mise unuse`

- **Usage**: `mise unuse [--no-prune] [--global] <INSTALLED_TOOL@VERSION>…`
- **Aliases**: `rm`, `remove`
- **Source code**: [`src/cli/unuse.rs`](https://github.com/jdx/mise/blob/main/src/cli/unuse.rs)

Removes installed tool versions from mise.toml

Will also prune the installed version if no other configurations are using it.

### Arguments

#### `<INSTALLED_TOOL@VERSION>…`

Tool(s) to remove

### Flags

#### `--no-prune`

Do not also prune the installed version

#### `--global`

Remove tool from global config

Examples:

    # will uninstall specific version
    $ mise remove node@18.0.0

## `mise upgrade`

- **Usage**: `mise upgrade [FLAGS] [TOOL@VERSION]…`
- **Aliases**: `up`
- **Source code**: [`src/cli/upgrade.rs`](https://github.com/jdx/mise/blob/main/src/cli/upgrade.rs)

Upgrades outdated tools

By default, this keeps the range specified in mise.toml. So if you have node@20 set, it will
upgrade to the latest 20.x.x version available. See the `--bump` flag to use the latest version
and bump the version in mise.toml.

This will update mise.lock if it is enabled, see https://mise.jdx.dev/configuration/settings.html#lockfile

### Arguments

#### `[TOOL@VERSION]…`

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
- **Source code**: [`src/cli/usage.rs`](https://github.com/jdx/mise/blob/main/src/cli/usage.rs)

Generate a usage CLI spec

See https://usage.jdx.dev for more information on this specification.

## `mise use`

- **Usage**: `mise use [FLAGS] [TOOL@VERSION]…`
- **Aliases**: `u`
- **Source code**: [`src/cli/use.rs`](https://github.com/jdx/mise/blob/main/src/cli/use.rs)

Installs a tool and adds the version to mise.toml.

This will install the tool version if it is not already installed.
By default, this will use a `mise.toml` file in the current directory.

In the following order:
  - If `MISE_DEFAULT_CONFIG_FILENAME` is set, it will use that instead.
  - If `MISE_OVERRIDE_CONFIG_FILENAMES` is set, it will the first from that list.
  - If `MISE_ENV` is set, it will use a `mise.<env>.toml` instead.
  - Otherwise just "mise.toml"

Use the `--global` flag to use the global config file instead.

### Arguments

#### `[TOOL@VERSION]…`

Tool(s) to add to config file

e.g.: node@20, cargo:ripgrep@latest npm:prettier@3
If no version is specified, it will default to @latest

Tool options can be set with this syntax:

    mise use ubi:BurntSushi/ripgrep[exe=rg]

### Flags

#### `-f --force`

Force reinstall even if already installed

#### `--fuzzy`

Save fuzzy version to config file

e.g.: `mise use --fuzzy node@20` will save 20 as the version
this is the default behavior unless `MISE_PIN=1`

#### `-g --global`

Use the global config file (`~/.config/mise/config.toml`) instead of the local one

#### `-e --env <ENV>`

Create/modify an environment-specific config file like .mise.<env>.toml

#### `-j --jobs <JOBS>`

Number of jobs to run in parallel
[default: 4]

#### `--raw`

Directly pipe stdin/stdout/stderr from plugin to user Sets `--jobs=1`

#### `--remove… <PLUGIN>`

Remove the plugin(s) from config file

#### `-p --path <PATH>`

Specify a path to a config file or directory

If a directory is specified, it will look for a config file in that directory following the rules above.

#### `--pin`

Save exact version to config file
e.g.: `mise use --pin node@20` will save 20.0.0 as the version
Set `MISE_PIN=1` to make this the default behavior

Consider using mise.lock as a better alternative to pinning in mise.toml:
https://mise.jdx.dev/configuration/settings.html#lockfile

Examples:
    
    # run with no arguments to use the interactive selector
    $ mise use

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
- **Source code**: [`src/cli/version.rs`](https://github.com/jdx/mise/blob/main/src/cli/version.rs)

Display the version of mise

Displays the version, os, architecture, and the date of the build.

If the version is out of date, it will display a warning.

Examples:

    $ mise version
    $ mise --version
    $ mise -v
    $ mise -V

## `mise watch`

- **Usage**: `mise watch [FLAGS] [TASK] [ARGS]…`
- **Aliases**: `w`
- **Source code**: [`src/cli/watch.rs`](https://github.com/jdx/mise/blob/main/src/cli/watch.rs)

Run task(s) and watch for changes to rerun it

This command uses the `watchexec` tool to watch for changes to files and rerun the specified task(s).
It must be installed for this command to work, but you can install it with `mise use -g watchexec@latest`.

### Arguments

#### `[TASK]`

Tasks to run
Can specify multiple tasks by separating with `:::`
e.g.: `mise run task1 arg1 arg2 ::: task2 arg1 arg2`

#### `[ARGS]…`

Task and arguments to run

### Flags

#### `-w --watch… <PATH>`

Watch a specific file or directory

By default, Watchexec watches the current directory.

When watching a single file, it's often better to watch the containing directory instead, and filter on the filename. Some editors may replace the file with a new one when saving, and some platforms may not detect that or further changes.

Upon starting, Watchexec resolves a "project origin" from the watched paths. See the help for '--project-origin' for more information.

This option can be specified multiple times to watch multiple files or directories.

The special value '/dev/null', provided as the only path watched, will cause Watchexec to not watch any paths. Other event sources (like signals or key events) may still be used.

#### `-W --watch-non-recursive… <PATH>`

Watch a specific directory, non-recursively

Unlike '-w', folders watched with this option are not recursed into.

This option can be specified multiple times to watch multiple directories non-recursively.

#### `-F --watch-file <PATH>`

Watch files and directories from a file

Each line in the file will be interpreted as if given to '-w'.

For more complex uses (like watching non-recursively), use the argfile capability: build a file containing command-line options and pass it to watchexec with `@path/to/argfile`.

The special value '-' will read from STDIN; this in incompatible with '--stdin-quit'.

#### `-c --clear <MODE>`

Clear screen before running command

If this doesn't completely clear the screen, try '--clear=reset'.

**Choices:**

- `clear`
- `reset`

#### `-o --on-busy-update <MODE>`

What to do when receiving events while the command is running

Default is to 'do-nothing', which ignores events while the command is running, so that changes that occur due to the command are ignored, like compilation outputs. You can also use 'queue' which will run the command once again when the current run has finished if any events occur while it's running, or 'restart', which terminates the running command and starts a new one. Finally, there's 'signal', which only sends a signal; this can be useful with programs that can reload their configuration without a full restart.

The signal can be specified with the '--signal' option.

**Choices:**

- `queue`
- `do-nothing`
- `restart`
- `signal`

#### `-r --restart`

Restart the process if it's still running

This is a shorthand for '--on-busy-update=restart'.

#### `-s --signal <SIGNAL>`

Send a signal to the process when it's still running

Specify a signal to send to the process when it's still running. This implies '--on-busy-update=signal'; otherwise the signal used when that mode is 'restart' is controlled by '--stop-signal'.

See the long documentation for '--stop-signal' for syntax.

Signals are not supported on Windows at the moment, and will always be overridden to 'kill'. See '--stop-signal' for more on Windows "signals".

#### `--stop-signal <SIGNAL>`

Signal to send to stop the command

This is used by 'restart' and 'signal' modes of '--on-busy-update' (unless '--signal' is provided). The restart behaviour is to send the signal, wait for the command to exit, and if it hasn't exited after some time (see '--timeout-stop'), forcefully terminate it.

The default on unix is "SIGTERM".

Input is parsed as a full signal name (like "SIGTERM"), a short signal name (like "TERM"), or a signal number (like "15"). All input is case-insensitive.

On Windows this option is technically supported but only supports the "KILL" event, as Watchexec cannot yet deliver other events. Windows doesn't have signals as such; instead it has termination (here called "KILL" or "STOP") and "CTRL+C", "CTRL+BREAK", and "CTRL+CLOSE" events. For portability the unix signals "SIGKILL", "SIGINT", "SIGTERM", and "SIGHUP" are respectively mapped to these.

#### `--stop-timeout <TIMEOUT>`

Time to wait for the command to exit gracefully

This is used by the 'restart' mode of '--on-busy-update'. After the graceful stop signal is sent, Watchexec will wait for the command to exit. If it hasn't exited after this time, it is forcefully terminated.

Takes a unit-less value in seconds, or a time span value such as "5min 20s". Providing a unit-less value is deprecated and will warn; it will be an error in the future.

The default is 10 seconds. Set to 0 to immediately force-kill the command.

This has no practical effect on Windows as the command is always forcefully terminated; see '--stop-signal' for why.

#### `--map-signal… <SIGNAL:SIGNAL>`

Translate signals from the OS to signals to send to the command

Takes a pair of signal names, separated by a colon, such as "TERM:INT" to map SIGTERM to SIGINT. The first signal is the one received by watchexec, and the second is the one sent to the command. The second can be omitted to discard the first signal, such as "TERM:" to not do anything on SIGTERM.

If SIGINT or SIGTERM are mapped, then they no longer quit Watchexec. Besides making it hard to quit Watchexec itself, this is useful to send pass a Ctrl-C to the command without also terminating Watchexec and the underlying program with it, e.g. with "INT:INT".

This option can be specified multiple times to map multiple signals.

Signal syntax is case-insensitive for short names (like "TERM", "USR2") and long names (like "SIGKILL", "SIGHUP"). Signal numbers are also supported (like "15", "31"). On Windows, the forms "STOP", "CTRL+C", and "CTRL+BREAK" are also supported to receive, but Watchexec cannot yet deliver other "signals" than a STOP.

#### `-d --debounce <TIMEOUT>`

Time to wait for new events before taking action

When an event is received, Watchexec will wait for up to this amount of time before handling it (such as running the command). This is essential as what you might perceive as a single change may actually emit many events, and without this behaviour, Watchexec would run much too often. Additionally, it's not infrequent that file writes are not atomic, and each write may emit an event, so this is a good way to avoid running a command while a file is partially written.

An alternative use is to set a high value (like "30min" or longer), to save power or bandwidth on intensive tasks, like an ad-hoc backup script. In those use cases, note that every accumulated event will build up in memory.

Takes a unit-less value in milliseconds, or a time span value such as "5sec 20ms". Providing a unit-less value is deprecated and will warn; it will be an error in the future.

The default is 50 milliseconds. Setting to 0 is highly discouraged.

#### `--stdin-quit`

Exit when stdin closes

This watches the stdin file descriptor for EOF, and exits Watchexec gracefully when it is closed. This is used by some process managers to avoid leaving zombie processes around.

#### `--no-vcs-ignore`

Don't load gitignores

Among other VCS exclude files, like for Mercurial, Subversion, Bazaar, DARCS, Fossil. Note that Watchexec will detect which of these is in use, if any, and only load the relevant files. Both global (like '~/.gitignore') and local (like '.gitignore') files are considered.

This option is useful if you want to watch files that are ignored by Git.

#### `--no-project-ignore`

Don't load project-local ignores

This disables loading of project-local ignore files, like '.gitignore' or '.ignore' in the
watched project. This is contrasted with '--no-vcs-ignore', which disables loading of Git
and other VCS ignore files, and with '--no-global-ignore', which disables loading of global
or user ignore files, like '~/.gitignore' or '~/.config/watchexec/ignore'.

Supported project ignore files:

  - Git: .gitignore at project root and child directories, .git/info/exclude, and the file pointed to by `core.excludesFile` in .git/config.
  - Mercurial: .hgignore at project root and child directories.
  - Bazaar: .bzrignore at project root.
  - Darcs: _darcs/prefs/boring
  - Fossil: .fossil-settings/ignore-glob
  - Ripgrep/Watchexec/generic: .ignore at project root and child directories.

VCS ignore files (Git, Mercurial, Bazaar, Darcs, Fossil) are only used if the corresponding
VCS is discovered to be in use for the project/origin. For example, a .bzrignore in a Git
repository will be discarded.

#### `--no-global-ignore`

Don't load global ignores

This disables loading of global or user ignore files, like '~/.gitignore',
'~/.config/watchexec/ignore', or '%APPDATA%\Bazzar\2.0\ignore'. Contrast with
'--no-vcs-ignore' and '--no-project-ignore'.

Supported global ignore files

  - Git (if core.excludesFile is set): the file at that path
  - Git (otherwise): the first found of $XDG_CONFIG_HOME/git/ignore, %APPDATA%/.gitignore, %USERPROFILE%/.gitignore, $HOME/.config/git/ignore, $HOME/.gitignore.
  - Bazaar: the first found of %APPDATA%/Bazzar/2.0/ignore, $HOME/.bazaar/ignore.
  - Watchexec: the first found of $XDG_CONFIG_HOME/watchexec/ignore, %APPDATA%/watchexec/ignore, %USERPROFILE%/.watchexec/ignore, $HOME/.watchexec/ignore.

Like for project files, Git and Bazaar global files will only be used for the corresponding
VCS as used in the project.

#### `--no-default-ignore`

Don't use internal default ignores

Watchexec has a set of default ignore patterns, such as editor swap files, `*.pyc`, `*.pyo`, `.DS_Store`, `.bzr`, `_darcs`, `.fossil-settings`, `.git`, `.hg`, `.pijul`, `.svn`, and Watchexec log files.

#### `--no-discover-ignore`

Don't discover ignore files at all

This is a shorthand for '--no-global-ignore', '--no-vcs-ignore', '--no-project-ignore', but even more efficient as it will skip all the ignore discovery mechanisms from the get go.

Note that default ignores are still loaded, see '--no-default-ignore'.

#### `--ignore-nothing`

Don't ignore anything at all

This is a shorthand for '--no-discover-ignore', '--no-default-ignore'.

Note that ignores explicitly loaded via other command line options, such as '--ignore' or '--ignore-file', will still be used.

#### `-p --postpone`

Wait until first change before running command

By default, Watchexec will run the command once immediately. With this option, it will instead wait until an event is detected before running the command as normal.

#### `--delay-run <DURATION>`

Sleep before running the command

This option will cause Watchexec to sleep for the specified amount of time before running the command, after an event is detected. This is like using "sleep 5 && command" in a shell, but portable and slightly more efficient.

Takes a unit-less value in seconds, or a time span value such as "2min 5s". Providing a unit-less value is deprecated and will warn; it will be an error in the future.

#### `--poll <INTERVAL>`

Poll for filesystem changes

By default, and where available, Watchexec uses the operating system's native file system watching capabilities. This option disables that and instead uses a polling mechanism, which is less efficient but can work around issues with some file systems (like network shares) or edge cases.

Optionally takes a unit-less value in milliseconds, or a time span value such as "2s 500ms", to use as the polling interval. If not specified, the default is 30 seconds. Providing a unit-less value is deprecated and will warn; it will be an error in the future.

Aliased as '--force-poll'.

#### `--shell <SHELL>`

Use a different shell

By default, Watchexec will use '$SHELL' if it's defined or a default of 'sh' on Unix-likes, and either 'pwsh', 'powershell', or 'cmd' (CMD.EXE) on Windows, depending on what Watchexec detects is the running shell.

With this option, you can override that and use a different shell, for example one with more features or one which has your custom aliases and functions.

If the value has spaces, it is parsed as a command line, and the first word used as the shell program, with the rest as arguments to the shell.

The command is run with the '-c' flag (except for 'cmd' on Windows, where it's '/C').

The special value 'none' can be used to disable shell use entirely. In that case, the command provided to Watchexec will be parsed, with the first word being the executable and the rest being the arguments, and executed directly. Note that this parsing is rudimentary, and may not work as expected in all cases.

Using 'none' is a little more efficient and can enable a stricter interpretation of the input, but it also means that you can't use shell features like globbing, redirection, control flow, logic, or pipes.

Examples:

Use without shell:

$ watchexec -n -- zsh -x -o shwordsplit scr

Use with powershell core:

$ watchexec --shell=pwsh -- Test-Connection localhost

Use with CMD.exe:

$ watchexec --shell=cmd -- dir

Use with a different unix shell:

$ watchexec --shell=bash -- 'echo $BASH_VERSION'

Use with a unix shell and options:

$ watchexec --shell='zsh -x -o shwordsplit' -- scr

#### `-n`

Shorthand for '--shell=none'

#### `--emit-events-to <MODE>`

Configure event emission

Watchexec can emit event information when running a command, which can be used by the child
process to target specific changed files.

One thing to take care with is assuming inherent behaviour where there is only chance.
Notably, it could appear as if the `RENAMED` variable contains both the original and the new
path being renamed. In previous versions, it would even appear on some platforms as if the
original always came before the new. However, none of this was true. It's impossible to
reliably and portably know which changed path is the old or new, "half" renames may appear
(only the original, only the new), "unknown" renames may appear (change was a rename, but
whether it was the old or new isn't known), rename events might split across two debouncing
boundaries, and so on.

This option controls where that information is emitted. It defaults to 'none', which doesn't
emit event information at all. The other options are 'environment' (deprecated), 'stdio',
'file', 'json-stdio', and 'json-file'.

The 'stdio' and 'file' modes are text-based: 'stdio' writes absolute paths to the stdin of
the command, one per line, each prefixed with `create:`, `remove:`, `rename:`, `modify:`,
or `other:`, then closes the handle; 'file' writes the same thing to a temporary file, and
its path is given with the $WATCHEXEC_EVENTS_FILE environment variable.

There are also two JSON modes, which are based on JSON objects and can represent the full
set of events Watchexec handles. Here's an example of a folder being created on Linux:

```json
  {
    "tags": [
      {
        "kind": "path",
        "absolute": "/home/user/your/new-folder",
        "filetype": "dir"
      },
      {
        "kind": "fs",
        "simple": "create",
        "full": "Create(Folder)"
      },
      {
        "kind": "source",
        "source": "filesystem",
      }
    ],
    "metadata": {
      "notify-backend": "inotify"
    }
  }
```

The fields are as follows:

  - `tags`, structured event data.
  - `tags[].kind`, which can be:
    * 'path', along with:
      + `absolute`, an absolute path.
      + `filetype`, a file type if known ('dir', 'file', 'symlink', 'other').
    * 'fs':
      + `simple`, the "simple" event type ('access', 'create', 'modify', 'remove', or 'other').
      + `full`, the "full" event type, which is too complex to fully describe here, but looks like 'General(Precise(Specific))'.
    * 'source', along with:
      + `source`, the source of the event ('filesystem', 'keyboard', 'mouse', 'os', 'time', 'internal').
    * 'keyboard', along with:
      + `keycode`. Currently only the value 'eof' is supported.
    * 'process', for events caused by processes:
      + `pid`, the process ID.
    * 'signal', for signals sent to Watchexec:
      + `signal`, the normalised signal name ('hangup', 'interrupt', 'quit', 'terminate', 'user1', 'user2').
    * 'completion', for when a command ends:
      + `disposition`, the exit disposition ('success', 'error', 'signal', 'stop', 'exception', 'continued').
      + `code`, the exit, signal, stop, or exception code.
  - `metadata`, additional information about the event.

The 'json-stdio' mode will emit JSON events to the standard input of the command, one per
line, then close stdin. The 'json-file' mode will create a temporary file, write the
events to it, and provide the path to the file with the $WATCHEXEC_EVENTS_FILE
environment variable.

Finally, the 'environment' mode was the default until 2.0. It sets environment variables
with the paths of the affected files, for filesystem events:

$WATCHEXEC_COMMON_PATH is set to the longest common path of all of the below variables,
and so should be prepended to each path to obtain the full/real path. Then:

  - $WATCHEXEC_CREATED_PATH is set when files/folders were created
  - $WATCHEXEC_REMOVED_PATH is set when files/folders were removed
  - $WATCHEXEC_RENAMED_PATH is set when files/folders were renamed
  - $WATCHEXEC_WRITTEN_PATH is set when files/folders were modified
  - $WATCHEXEC_META_CHANGED_PATH is set when files/folders' metadata were modified
  - $WATCHEXEC_OTHERWISE_CHANGED_PATH is set for every other kind of pathed event

Multiple paths are separated by the system path separator, ';' on Windows and ':' on unix.
Within each variable, paths are deduplicated and sorted in binary order (i.e. neither
Unicode nor locale aware).

This is the legacy mode, is deprecated, and will be removed in the future. The environment
is a very restricted space, while also limited in what it can usefully represent. Large
numbers of files will either cause the environment to be truncated, or may error or crash
the process entirely. The $WATCHEXEC_COMMON_PATH is also unintuitive, as demonstrated by the
multiple confused queries that have landed in my inbox over the years.

**Choices:**

- `environment`
- `stdio`
- `file`
- `json-stdio`
- `json-file`
- `none`

#### `--only-emit-events`

Only emit events to stdout, run no commands.

This is a convenience option for using Watchexec as a file watcher, without running any commands. It is almost equivalent to using `cat` as the command, except that it will not spawn a new process for each event.

This option requires `--emit-events-to` to be set, and restricts the available modes to `stdio` and `json-stdio`, modifying their behaviour to write to stdout instead of the stdin of the command.

#### `-E --env… <KEY=VALUE>`

Add env vars to the command

This is a convenience option for setting environment variables for the command, without setting them for the Watchexec process itself.

Use key=value syntax. Multiple variables can be set by repeating the option.

#### `--wrap-process <MODE>`

Configure how the process is wrapped

By default, Watchexec will run the command in a process group in Unix, and in a Job Object in Windows.

Some Unix programs prefer running in a session, while others do not work in a process group.

Use 'group' to use a process group, 'session' to use a process session, and 'none' to run the command directly. On Windows, either of 'group' or 'session' will use a Job Object.

**Choices:**

- `group`
- `session`
- `none`

#### `-N --notify`

Alert when commands start and end

With this, Watchexec will emit a desktop notification when a command starts and ends, on supported platforms. On unsupported platforms, it may silently do nothing, or log a warning.

#### `--color <MODE>`

When to use terminal colours

Setting the environment variable `NO_COLOR` to any value is equivalent to `--color=never`.

**Choices:**

- `auto`
- `always`
- `never`

#### `--timings`

Print how long the command took to run

This may not be exactly accurate, as it includes some overhead from Watchexec itself. Use the `time` utility, high-precision timers, or benchmarking tools for more accurate results.

#### `-q --quiet`

Don't print starting and stopping messages

By default Watchexec will print a message when the command starts and stops. This option disables this behaviour, so only the command's output, warnings, and errors will be printed.

#### `--bell`

Ring the terminal bell on command completion

#### `--project-origin <DIRECTORY>`

Set the project origin

Watchexec will attempt to discover the project's "origin" (or "root") by searching for a variety of markers, like files or directory patterns. It does its best but sometimes gets it it wrong, and you can override that with this option.

The project origin is used to determine the path of certain ignore files, which VCS is being used, the meaning of a leading '/' in filtering patterns, and maybe more in the future.

When set, Watchexec will also not bother searching, which can be significantly faster.

#### `--workdir <DIRECTORY>`

Set the working directory

By default, the working directory of the command is the working directory of Watchexec. You can change that with this option. Note that paths may be less intuitive to use with this.

#### `-e --exts… <EXTENSIONS>`

Filename extensions to filter to

This is a quick filter to only emit events for files with the given extensions. Extensions can be given with or without the leading dot (e.g. 'js' or '.js'). Multiple extensions can be given by repeating the option or by separating them with commas.

#### `-f --filter… <PATTERN>`

Filename patterns to filter to

Provide a glob-like filter pattern, and only events for files matching the pattern will be emitted. Multiple patterns can be given by repeating the option. Events that are not from files (e.g. signals, keyboard events) will pass through untouched.

#### `--filter-file… <PATH>`

Files to load filters from

Provide a path to a file containing filters, one per line. Empty lines and lines starting with '#' are ignored. Uses the same pattern format as the '--filter' option.

This can also be used via the $WATCHEXEC_FILTER_FILES environment variable.

#### `-J --filter-prog… <EXPRESSION>`

[experimental] Filter programs.

/!\ This option is EXPERIMENTAL and may change and/or vanish without notice.

Provide your own custom filter programs in jaq (similar to jq) syntax. Programs are given an event in the same format as described in '--emit-events-to' and must return a boolean. Invalid programs will make watchexec fail to start; use '-v' to see program runtime errors.

In addition to the jaq stdlib, watchexec adds some custom filter definitions:

- 'path | file_meta' returns file metadata or null if the file does not exist.

- 'path | file_size' returns the size of the file at path, or null if it does not exist.

- 'path | file_read(bytes)' returns a string with the first n bytes of the file at path. If the file is smaller than n bytes, the whole file is returned. There is no filter to read the whole file at once to encourage limiting the amount of data read and processed.

- 'string | hash', and 'path | file_hash' return the hash of the string or file at path. No guarantee is made about the algorithm used: treat it as an opaque value.

- 'any | kv_store(key)', 'kv_fetch(key)', and 'kv_clear' provide a simple key-value store. Data is kept in memory only, there is no persistence. Consistency is not guaranteed.

- 'any | printout', 'any | printerr', and 'any | log(level)' will print or log any given value to stdout, stderr, or the log (levels = error, warn, info, debug, trace), and pass the value through (so '[1] | log("debug") | .[]' will produce a '1' and log '[1]').

All filtering done with such programs, and especially those using kv or filesystem access, is much slower than the other filtering methods. If filtering is too slow, events will back up and stall watchexec. Take care when designing your filters.

If the argument to this option starts with an '@', the rest of the argument is taken to be the path to a file containing a jaq program.

Jaq programs are run in order, after all other filters, and short-circuit: if a filter (jaq or not) rejects an event, execution stops there, and no other filters are run. Additionally, they stop after outputting the first value, so you'll want to use 'any' or 'all' when iterating, otherwise only the first item will be processed, which can be quite confusing!

Find user-contributed programs or submit your own useful ones at <https://github.com/watchexec/watchexec/discussions/592>.

## Examples:

Regexp ignore filter on paths:

'all(.tags[] | select(.kind == "path"); .absolute | test("[.]test[.]js$")) | not'

Pass any event that creates a file:

'any(.tags[] | select(.kind == "fs"); .simple == "create")'

Pass events that touch executable files:

'any(.tags[] | select(.kind == "path" && .filetype == "file"); .absolute | metadata | .executable)'

Ignore files that start with shebangs:

'any(.tags[] | select(.kind == "path" && .filetype == "file"); .absolute | read(2) == "#!") | not'

#### `-i --ignore… <PATTERN>`

Filename patterns to filter out

Provide a glob-like filter pattern, and events for files matching the pattern will be excluded. Multiple patterns can be given by repeating the option. Events that are not from files (e.g. signals, keyboard events) will pass through untouched.

#### `--ignore-file… <PATH>`

Files to load ignores from

Provide a path to a file containing ignores, one per line. Empty lines and lines starting with '#' are ignored. Uses the same pattern format as the '--ignore' option.

This can also be used via the $WATCHEXEC_IGNORE_FILES environment variable.

#### `--fs-events… <EVENTS>`

Filesystem events to filter to

This is a quick filter to only emit events for the given types of filesystem changes. Choose from 'access', 'create', 'remove', 'rename', 'modify', 'metadata'. Multiple types can be given by repeating the option or by separating them with commas. By default, this is all types except for 'access'.

This may apply filtering at the kernel level when possible, which can be more efficient, but may be more confusing when reading the logs.

**Choices:**

- `access`
- `create`
- `remove`
- `rename`
- `modify`
- `metadata`

#### `--no-meta`

Don't emit fs events for metadata changes

This is a shorthand for '--fs-events create,remove,rename,modify'. Using it alongside the '--fs-events' option is non-sensical and not allowed.

#### `--print-events`

Print events that trigger actions

This prints the events that triggered the action when handling it (after debouncing), in a human readable form. This is useful for debugging filters.

Use '-vvv' instead when you need more diagnostic information.

#### `--manual`

Show the manual page

This shows the manual page for Watchexec, if the output is a terminal and the 'man' program is available. If not, the manual page is printed to stdout in ROFF format (suitable for writing to a watchexec.1 file).

Examples:

    $ mise watch build
    Runs the "build" tasks. Will re-run the tasks when any of its sources change.
    Uses "sources" from the tasks definition to determine which files to watch.

    $ mise watch build --glob src/**/*.rs
    Runs the "build" tasks but specify the files to watch with a glob pattern.
    This overrides the "sources" from the tasks definition.

    $ mise watch build --clear
    Extra arguments are passed to watchexec. See `watchexec --help` for details.

    $ mise watch serve --watch src --exts rs --restart
    Starts an api server, watching for changes to "*.rs" files in "./src" and kills/restarts the server when they change.

## `mise where`

- **Usage**: `mise where <TOOL@VERSION>`
- **Source code**: [`src/cli/where.rs`](https://github.com/jdx/mise/blob/main/src/cli/where.rs)

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

- **Usage**: `mise which [FLAGS] [BIN_NAME]`
- **Source code**: [`src/cli/which.rs`](https://github.com/jdx/mise/blob/main/src/cli/which.rs)

Shows the path that a tool's bin points to.

Use this to figure out what version of a tool is currently active.

### Arguments

#### `[BIN_NAME]`

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
- **Source code**: [`src/cli/render_help.rs`](https://github.com/jdx/mise/blob/main/src/cli/render_help.rs)

internal command to generate markdown from help

## `mise render-mangen`

- **Usage**: `mise render-mangen`
- **Source code**: [`src/cli/render_mangen.rs`](https://github.com/jdx/mise/blob/main/src/cli/render_mangen.rs)

internal command to generate markdown from help
