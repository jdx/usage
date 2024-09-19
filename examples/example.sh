#!/usr/bin/env -S usage bash
# |usage.jdx.dev|
# bin "ex"
# flag "--foo" help="Flag value"
# flag "--bar <bar>" help="Option value"
# arg "baz" help="Positional values"
# |usage.jdx.dev|
# shellcheck disable=all
set -euo pipefail

echo foo: $usage_foo
echo bar: $usage_bar
echo baz: $usage_baz
