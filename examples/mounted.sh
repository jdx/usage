#!/usr/bin/env -S usage bash
# shellcheck disable=SC2154
#USAGE bin "ex"
#USAGE flag "--mount" help="Display kdl spec for mounted tasks"
#USAGE cmd "exec-task" {
#USAGE   mount run="mounted.sh --mount"
#USAGE }
set -eo pipefail

if [ "${usage_mount:-}" = "true" ]; then
	cat <<EOF
cmd "task-a" {
  flag "--flag1" help="Flag 1"
  flag "--flag2 <flag2>" help="Flag 2"
}
cmd "task-b" {
	flag "--flag3" help="Flag 3"
	flag "--flag4 <flag4>" help="Flag 4"
}
EOF
	exit
fi

echo foo: $usage_foo
echo bar: $usage_bar
echo baz: $usage_baz
