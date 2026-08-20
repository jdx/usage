# The fleet

One spec per jdx CLI, checked in as a fixture.

Each is the output of that CLI's own `usage` subcommand, copied verbatim. The fixtures now come
from the typed `usage-rs` fleet rewrites; they therefore exercise the metadata the replacement
actually emits instead of the subset an older `clap_usage` bridge could recover. They are here
for the same reason `benches/mise.usage.kdl` is: the comparison needs a fixed input, and a
reviewer needs to be able to read the diff when one of them changes.

## Why more than mise

mise is 211 commands and was the only spec the parity gate read, which made the gate blind to
anything mise's CLI happens not to do. Three bugs lived in that blind spot:

- **The version banner.** usage-lib prints `hk 1.55.0` above the description on the root page.
  The shadow generator declared no `version`, so five of the seven CLIs rendered a different
  first line. mise declares no version at all, so with nothing to print both renderers agreed.
- **Optional flag values.** pitchfork's `--bump` is `arg "[BUMP]" required=#false`. usage-argv
  had nowhere to put it and rendered `<BUMP>`. mise has no such flag.
- **Descriptions ending in a break.** pitchfork's `daemons add` ends its examples block with a
  newline, which put a stray blank line in the middle of `Commands:`.

None of the three is exotic. Each was simply outside one CLI's vocabulary.

## Keeping them current

These are snapshots, not links. A CLI that changes its own flags will not update the copy here,
and that is fine — the fixture's job is to hold a shape the renderers must agree on, not to
track a moving target. Refresh one from its usage-migration branch by re-running its `usage`
subcommand and committing the diff, the same way `benches/mise.usage.kdl` is refreshed.
