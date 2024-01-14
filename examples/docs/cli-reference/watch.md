# `mise watch`
* Aliases: `w`
#### Args

* `[ARGS]...` – Extra arguments

#### Flags

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
