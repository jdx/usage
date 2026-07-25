#!/usr/bin/env -S usage bash
#
# Test fixture for jdx/mise#11282: the flags of the mounting CLI must not leak into a
# mounted command's completions.
#
# The root declares globals (`-E/--env <ENV>`, `--silent`) which are only accepted *before*
# the mounted command — everything after a task name is forwarded to the task itself. Two
# defects followed from inheriting those globals into the mounted command:
#
#   1. `run task <TAB>` offered `--env`/`--silent`, which the mounted program rejects.
#   2. The mounted task's own `--env` (with choices) was shadowed by the root's `--env`,
#      so completing its value fell back to file completion instead of the choices.
#
# Example usage:
#   mounted-global-flag-leak.sh run mytask --<TAB>        # -> --bump --env --output-dir
#   mounted-global-flag-leak.sh run mytask --env <TAB>    # -> dev stage prod
#   mounted-global-flag-leak.sh -E prod run mytask <TAB>  # global before the task still parses
#
#USAGE bin "ex"
#USAGE flag "-E --env <ENV>" help="Set the environment" global=#true
#USAGE flag "--silent" help="Silent output" global=#true
#USAGE flag "--mount" help="Display kdl spec for mounted tasks"
#USAGE cmd "run" {
#USAGE   flag "-f --force" help="Force the tasks to run"
#USAGE   mount run="mounted-global-flag-leak.sh --mount"
#USAGE }
set -eo pipefail

# Declare variables set by usage to avoid SC2154
: "${usage_mount:=}"
: "${usage_env:=}"

if [ "${usage_mount:-}" = "true" ]; then
	# `mytask` declares its own `--env`, colliding with the root global of the same name.
	cat <<EOF
cmd "mytask" {
  flag "--env <name>" help="Environment to deploy to" {
    choices "dev" "stage" "prod"
  }
  flag "--bump <type>" help="Version bump" {
    choices "auto" "major" "minor" "patch"
  }
  flag "--output-dir <path>" help="Where to write output"
  arg "[target]" {
    choices "alpha" "beta"
  }
}
cmd "grouped" help="A mounted task with subcommands of its own" {
  flag "--group-wide" help="Applies to the whole group" global=#true
  cmd "leaf" help="Nested inside the mounted command" {
    flag "--leaf-only" help="Only on the leaf"
  }
}
EOF
	exit
fi

echo "Running with env: ${usage_env:-default}"
