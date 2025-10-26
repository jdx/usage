#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#
# Test fixture for global flag handling with mount points
#
# This script demonstrates that global flags (--dir/-d) are correctly
# passed through to mount commands, allowing mounted subcommands to
# access the same global context as the parent command.
#
# Example usage:
#   mounted-global-flags.sh run               # Uses default dir
#   mounted-global-flags.sh --dir=dir2 run    # Uses dir2
#   mounted-global-flags.sh -d dir2 run       # Same, using short flag
#
#USAGE bin "ex"
#USAGE flag "-d --dir <dir>" help="Working directory" global=#true
#USAGE flag "--mount" help="Display kdl spec for mounted tasks"
#USAGE cmd "run" {
#USAGE   mount run="mounted-global-flags.sh --mount"
#USAGE }
set -eo pipefail

if [ "${usage_mount:-}" = "true" ]; then
	# Check if --dir flag was passed through to mount
	if [ "${usage_dir:-}" = "dir2" ]; then
		cat <<EOF
cmd "task-bar" {
  help "Task from dir2"
}
cmd "task-foo" {
  help "Task from dir2"
}
EOF
	else
		cat <<EOF
cmd "task-a" {
  help "Task from default dir"
}
cmd "task-b" {
  help "Task from default dir"
}
EOF
	fi
	exit
fi

echo "Running with dir: ${usage_dir:-default}"
