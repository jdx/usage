# `mise tasks deps`

[experimental] Display a tree visualization of a dependency graph

###### Arg `[TASKS]...`

Tasks to show dependencies for
Can specify multiple tasks by separating with spaces
e.g.: mise tasks deps lint test check

##### Flag `--dot`

Display dependencies in DOT formatExamples:
  $ mise tasks deps
  Shows dependencies for all tasks

  $ mise tasks deps lint test check
  Shows dependencies for the "lint", "test" and "check" tasks

  $ mise tasks deps --dot
  Shows dependencies in DOT format
