### `plugins update`

* Aliases: `upgrade`
#### Args

* `[PLUGIN]...` – Plugin(s) to update

#### Flags

* `-j,--jobs <JOBS>` – Number of jobs to run in parallel
Default: 4
Updates a plugin to the latest version

note: this updates the plugin itself, not the runtime versions
Examples:
  $ mise plugins update            # update all plugins
  $ mise plugins update node       # update only node
  $ mise plugins update node#beta  # specify a ref
