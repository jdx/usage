# Updating an existing value

::: warning Draft
This page is a draft and has not yet been human reviewed. Details may change.
:::

Long-running programs such as REPLs and daemons can merge another command line into a value they
already hold:

```rust
// Print help/version/errors and exit as `parse()` does.
cli.update_from(&argv);

// Return errors to the caller.
cli.try_update_from(&argv)?;
```

`update_from_argv` and `try_update_from_argv` also strip argv0 and apply multicall applet
selection.

An update is atomic: if parsing or validation fails, the original value is unchanged. Otherwise:

- Existing values count as present for relationships such as `required` and `conflicts`.
- Environment variables and defaults fill only empty fields.
- A collection mentioned in the new argv replaces the old collection; an unmentioned one stays
  unchanged.
- Selecting a different subcommand replaces the old variant. Selecting the same one merges its
  fields.

Checks that need the original bytes cannot be rerun for an existing value. Choices, portable
validation expressions, and value-conditional relationships therefore apply only to values
supplied by the new argv.
