# `mise tasks`

###### Aliases: `t`

[experimental] Manage tasks
## Subcommands

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
