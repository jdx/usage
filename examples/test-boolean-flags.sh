#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#
# Test fixture for boolean flag handling with subcommands
#
# This script tests two important behaviors:
# 1. Boolean flags don't consume the next word (which could be a subcommand)
#    Example: "--verbose run" should recognize "run" as a subcommand
# 2. Global vs non-global flags affect subcommand discovery
#    - Global flags (--verbose, --debug) allow subcommand search to continue
#    - Non-global flags (--local) stop subcommand search
#
# Example usage:
#   test-boolean-flags.sh --verbose run         # Verbose mode
#   test-boolean-flags.sh -v -d run             # Verbose + debug mode
#   test-boolean-flags.sh --local run           # Error: --local stops search
#
#USAGE bin "test"
#USAGE flag "-v --verbose" help="Verbose output" global=#true
#USAGE flag "-d --debug" help="Debug mode" global=#true
#USAGE flag "--local" help="Local flag (not global)"
#USAGE flag "--mount" help="Display mounted spec"
#USAGE cmd "run" {
#USAGE   mount run="test-boolean-flags.sh --mount"
#USAGE }
set -eo pipefail

if [ "${usage_mount:-}" = "true" ]; then
	# Check if global flags were passed through
	if [ "${usage_verbose:-}" = "true" ] && [ "${usage_debug:-}" = "true" ]; then
		cat <<EOF
cmd "task-verbose-debug" {
  help "Task with verbose and debug"
}
EOF
	elif [ "${usage_verbose:-}" = "true" ]; then
		cat <<EOF
cmd "task-verbose" {
  help "Task with verbose"
}
EOF
	else
		cat <<EOF
cmd "task-default" {
  help "Task default"
}
EOF
	fi
	exit
fi

echo "Running with verbose: ${usage_verbose:-false}, debug: ${usage_debug:-false}"
