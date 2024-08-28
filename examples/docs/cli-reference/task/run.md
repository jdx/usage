# `mise task run`

* Aliases: `r`

#### Args

* `[TASK]` – Task to run
  Can specify multiple tasks by separating with `:::`
  e.g.: mise run task1 arg1 arg2 ::: task2 arg1 arg2
* `[ARGS]...` – Arguments to pass to the task. Use ":::" to separate tasks

#### Flags

* `-C --cd <CD>` – Change to this directory before executing the command
* `-n --dry-run` – Don't actually run the task(s), just print them in order of execution
* `-f --force` – Force the task to run even if outputs are up to date
* `-p --prefix` – Print stdout/stderr by line, prefixed with the task's label
  Defaults to true if --jobs > 1
  Configure with `task_output` config or `MISE_TASK_OUTPUT` env var
* `-i --interleave` – Print directly to stdout/stderr instead of by line
  Defaults to true if --jobs == 1
  Configure with `task_output` config or `MISE_TASK_OUTPUT` env var
* `-t --tool <TOOL@VERSION>` – Tool(s) to also add e.g.: node@20 python@3.10
* `-j --jobs <JOBS>` – Number of tasks to run in parallel
  [default: 4]
  Configure with `jobs` config or `MISE_JOBS` env var
* `-r --raw` – Read/write directly to stdin/stdout/stderr instead of by line
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
