#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#USAGE bin "ex"
#USAGE flag "--foo" help="Flag value"
#USAGE flag "--bar <bar>" help="Option value"
#USAGE flag "--defaulted <defaulted>" default="mydefault" help="Defaulted value"
#USAGE arg "baz" help="Positional values" default="mydefault"
#USAGE complete "baz" run="echo val-1; echo val-2; echo val-3"
#USAGE min_usage_version "1"
set -eo pipefail

echo foo: $usage_foo
echo bar: $usage_bar
echo baz: $usage_baz
