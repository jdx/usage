# `mise trust`
#### Args

* `[CONFIG_FILE]` – The config file to trust

#### Flags

* `-a,--all` – Trust all config files in the current directory and its parents
* `--untrust` – No longer trust this config
Marks a config file as trusted

This means mise will parse the file with potentially dangerous
features enabled.

This includes:
- environment variables
- templates
- `path:` plugin versions
Examples:
  # trusts ~/some_dir/.mise.toml
  $ mise trust ~/some_dir/.mise.toml

  # trusts .mise.toml in the current or parent directory
  $ mise trust
