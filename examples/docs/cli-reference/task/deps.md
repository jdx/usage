# `mise task deps`
#### Args

* `[TASKS]...` – Tasks to show dependencies for
Can specify multiple tasks by separating with spaces
e.g.: mise task deps lint test check

#### Flags

* `--dot` – Display dependencies in DOT format
[experimental] Display a tree visualization of a dependency graph
Examples:
  $ mise task deps
  Shows dependencies for all tasks

  $ mise task deps lint test check
  Shows dependencies for the "lint", "test" and "check" tasks

  $ mise task deps --dot
  Shows dependencies in DOT format
