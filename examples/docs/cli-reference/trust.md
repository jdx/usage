# `mise trust`

Marks a config file as trusted

This means mise will parse the file with potentially dangerous
features enabled.

This includes:
- environment variables
- templates
- `path:` plugin versions

###### Arg `[CONFIG_FILE]`

The config file to trust

##### Flag `-a --all`

Trust all config files in the current directory and its parents

##### Flag `--untrust`

No longer trust this config

Examples:
    # trusts ~/some_dir/.mise.toml
    $ mise trust ~/some_dir/.mise.toml

    # trusts .mise.toml in the current or parent directory
    $ mise trust
