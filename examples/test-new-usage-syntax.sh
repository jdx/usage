#!/usr/bin/env -S usage bash
#
# Test script for the bracketed USAGE syntax: # [USAGE]
# [USAGE] bin "test-new-syntax"
# [USAGE] flag "--foo" help="Flag value"
# [USAGE] flag "--bar <bar>" help="Option value"
# [USAGE] arg "baz" help="Positional value" default="mydefault"
set -eo pipefail

# Declare variables set by usage to avoid SC2154
: "${usage_foo:=}"
: "${usage_bar:=}"
: "${usage_baz:=}"

echo "foo: $usage_foo"
echo "bar: $usage_bar"
echo "baz: $usage_baz"

