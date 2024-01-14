### `shell`

* Aliases: `sh`
#### Args

* `[TOOL@VERSION]...` – Tool(s) to use

#### Flags

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
