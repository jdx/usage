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

Display the installation path for a runtime

Must be installed.
Examples:
  # Show the latest installed version of node
  # If it is is not installed, errors
  $ mise where node@20
  /home/jdx/.local/share/mise/installs/node/20.0.0

  # Show the current, active install directory of node
  # Errors if node is not referenced in any .tool-version file
  $ mise where node
  /home/jdx/.local/share/mise/installs/node/20.0.0
