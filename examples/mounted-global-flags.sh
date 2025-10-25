#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#USAGE bin "ex"
#USAGE flag "--dir <dir>" help="Working directory" global=#true
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
