#!/usr/bin/env usage
# shellcheck disable=all
bin "ex"
flag "--foo" help="Flag value"
flag "--bar <bar>" help="Option value"
arg "baz" help="Positional values"

#!/usr/bin/env bash
set -euo pipefail

echo foo: $usage_foo
echo bar: $usage_bar
echo baz: $usage_baz
