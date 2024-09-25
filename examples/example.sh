#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#USAGE bin "ex"
#USAGE flag "--foo" help="Flag value"
#USAGE flag "--bar <bar>" help="Option value"
#USAGE arg "baz" help="Positional values"
set -euo pipefail

echo foo: $usage_foo
echo bar: $usage_bar
echo baz: $usage_baz
