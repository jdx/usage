# `mise shell`

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

Removes a previously set versionExamples:
  $ mise shell node@20
  $ node -v
  v20.0.0
