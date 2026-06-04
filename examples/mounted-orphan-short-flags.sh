#!/usr/bin/env -S usage bash
#
# Test fixture for the orphan-short follow-up to jdx/mise#10069.
#
# The root declares LONG-ONLY global flags (`--raw`, `--silent`, no short). The `run` and
# `tasks run` subcommands re-declare them as NON-global but ADD a short (`-r --raw`,
# `-S --silent`), mirroring what mise emits. Each mounts a task `sample:run` whose positional
# arg has `choices`.
#
# Before the parser fix, the merge that keeps the inherited long-only global discarded the
# re-declaration wholesale, dropping the orphan short. Completing the task with the short in
# front (`run -r sample:run <TAB>`) then failed with an "unexpected word" / "Invalid choice"
# bail because `-r` was no longer recognized and was mis-parsed against the positional.
#
# After the fix the orphan short is unioned onto the surviving global flag, so the mounted
# task's choices complete with the short (or long) prefix, over both `run` and `tasks run`.
#
# Example usage:
#   mounted-orphan-short-flags.sh run -r sample:run <TAB>          # -> alpha beta gamma
#   mounted-orphan-short-flags.sh tasks run -S sample:run <TAB>    # -> alpha beta gamma
#
#USAGE bin "ex"
#USAGE flag "--raw" help="Raw output" global=#true
#USAGE flag "--silent" help="Silent output" global=#true
#USAGE flag "--mount" help="Display kdl spec for mounted tasks"
#USAGE cmd "run" {
#USAGE   flag "-r --raw" help="Raw output (non-global re-declaration with added short)"
#USAGE   flag "-S --silent" help="Silent output (non-global re-declaration with added short)"
#USAGE   mount run="mounted-orphan-short-flags.sh --mount"
#USAGE }
#USAGE cmd "tasks" {
#USAGE   cmd "run" {
#USAGE     flag "-r --raw" help="Raw output (non-global re-declaration with added short)"
#USAGE     flag "-S --silent" help="Silent output (non-global re-declaration with added short)"
#USAGE     mount run="mounted-orphan-short-flags.sh --mount"
#USAGE   }
#USAGE }
set -eo pipefail

# Declare variables set by usage to avoid SC2154
: "${usage_mount:=}"
: "${usage_raw:=}"
: "${usage_silent:=}"

if [ "${usage_mount:-}" = "true" ]; then
	# The choices are unconditional: completing them at all proves the orphan short was
	# recognized as a flag, since an unrecognized `-r`/`-S` would break the subcommand
	# descent before `sample:run` is found.
	cat <<EOF
cmd "sample:run" {
  arg "<profile>" {
    choices "alpha" "beta" "gamma"
  }
}
EOF
	exit
fi

echo "Running raw=${usage_raw:-false} silent=${usage_silent:-false}"
