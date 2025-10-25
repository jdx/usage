#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#USAGE bin "test"
#USAGE flag "--verbose" help="Verbose output" global=#true
#USAGE flag "--debug" help="Debug mode" global=#true
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
