#!/usr/bin/env -S usage bash
#
# Test script for new USAGE syntax: #[USAGE] (no space)
#
#[USAGE] bin "test-bracket-no-space"
#[USAGE] flag "--verbose" help="Verbose output"
#[USAGE] flag "--output <file>" help="Output file"
#[USAGE] arg "input" help="Input file"
set -eo pipefail

# Declare variables set by usage to avoid SC2154
: "${usage_verbose:=}"
: "${usage_output:=}"
: "${usage_input:=}"

echo "verbose: $usage_verbose"
echo "output: $usage_output"
echo "input: $usage_input"

