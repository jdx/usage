# `mise self-update`
#### Args

* `[VERSION]` – Update to a specific version

#### Flags

* `-f,--force` – Update even if already up to date
* `--no-plugins` – Disable auto-updating plugins
* `-y,--yes` – Skip confirmation prompt
Updates mise itself

Uses the GitHub Releases API to find the latest release and binary
By default, this will also update any installed plugins
