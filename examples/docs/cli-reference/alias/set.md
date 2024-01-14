### `alias set`

* Aliases: `add`, `create`
#### Args

* `<PLUGIN>` – The plugin to set the alias for
* `<ALIAS>` – The alias to set
* `<VALUE>` – The value to set the alias to

Add/update an alias for a plugin

This modifies the contents of ~/.config/mise/config.toml
Examples:
  $ mise alias set node lts-hydrogen 18.0.0
