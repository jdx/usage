### `uninstall`

* Aliases: `remove`, `rm`
#### Args

* `[TOOL@VERSION]...` – Tool(s) to remove

#### Flags

* `-a,--all` – Delete all installed versions
* `-n,--dry-run` – Do not actually delete anything
Removes runtime versions
Examples:
  $ mise uninstall node@18.0.0 # will uninstall specific version
  $ mise uninstall node        # will uninstall current node version
  $ mise uninstall --all node@18.0.0 # will uninstall all node versions
