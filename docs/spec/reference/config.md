# Configuration

The default priority for configuration properties in usage is the following:

- CLI flag (e.g. `--user alice`)
- Environment variable (e.g. `MYCLI_USER=alice`)
- Config file (e.g. `~/.mycli.toml`)
- Default value

## Environment Variables

TODO

## Config Files

```kdl
config {
    // system
    file "/etc/mycli.toml"
    file "/etc/mycli.json"

    // global
    file "~/.config/mycli.toml"
    file "~/.config/mycli.json"

    // local
    file ".config/mycli.toml" findup=true
    file ".config/mycli.json" findup=true
    file ".mycli.dist.toml" findup=true
    file ".mycli.dist.json" findup=true
    file ".mycli.toml" findup=true
    file ".mycli.json" findup=true
    file ".myclirc" findup=true format="ini"

    // e.g.: .mycli.dev.toml, .mycli.prod.toml
    file ".mycli.$MYCLI_ENV.toml" findup=true

    default "user" "admin"
    default "work_dir" "/tmp"
    default "yes" false

    alias "user" "username"
}
```

## Alias Config Keys

Config keys can be aliased to other keys. This is useful for backwards compatibility.

```kdl
config_file ".mycli.toml" findup=true
config_alias "user" "username"
```
