#!/usr/bin/env -S usage bash
#
# Test fixture reproducing the parser-side root cause referenced by jdx/mise#10069.
#
# A value-taking global flag (-C/--cd) is placed before a mounted subcommand
# (`run`). The `run` subcommand re-declares the same flag as NON-global (mirroring
# what mise emits for `run`/`tasks run`), and mounts a task `sample:run` whose
# positional arg has `choices`.
#
# Before the parser fix, completing the task's positional arg produced:
#   Error: Invalid choice for arg profile: -C, expected one of alpha, beta, gamma
# because the global flag was dropped from the recognized flags when descending into
# the re-declaring subcommand, so the leftover token was mis-parsed as the positional.
#
# The mounted choices vary by the global flag's value so the test also verifies the
# global flag (and its value) still propagates to the mount despite the non-global
# re-declaration in `run` (i.e. it is not silently dropped from the env).
#
# Example usage:
#   mounted-global-flags-choices.sh -C dir2 run sample:run <TAB>   # -> alpha beta gamma
#   mounted-global-flags-choices.sh run sample:run <TAB>           # -> one two
#
#USAGE bin "ex"
#USAGE flag "-C --cd <dir>" help="Change directory" global=#true
#USAGE flag "--mount" help="Display kdl spec for mounted tasks"
#USAGE cmd "run" {
#USAGE   flag "-C --cd <dir>" help="Change directory (non-global re-declaration)"
#USAGE   mount run="mounted-global-flags-choices.sh --mount"
#USAGE }
set -eo pipefail

# Declare variables set by usage to avoid SC2154
: "${usage_mount:=}"
: "${usage_cd:=}"

if [ "${usage_mount:-}" = "true" ]; then
	# The choices depend on the global flag value to prove it reached the mount.
	if [ "${usage_cd:-}" = "dir2" ]; then
		cat <<EOF
cmd "sample:run" {
  arg "<profile>" {
    choices "alpha" "beta" "gamma"
  }
}
EOF
	else
		cat <<EOF
cmd "sample:run" {
  arg "<profile>" {
    choices "one" "two"
  }
}
EOF
	fi
	exit
fi

echo "Running with dir: ${usage_cd:-default}"
