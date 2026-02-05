#!/usr/bin/env bash
set -euo pipefail

# Generate rich release notes for GitHub releases using Claude Code
# Usage: ./scripts/gen-release-notes.sh <version> [prev_version]

version="${1:-}"
prev_version="${2:-}"

if [[ -z $version ]]; then
	echo "Usage: $0 <version> [prev_version]" >&2
	exit 1
fi

# Get the git-cliff changelog for context
# Use the tag range if prev_version provided, otherwise unreleased
if [[ -n $prev_version ]]; then
	changelog=$(git cliff --strip all "${prev_version}..${version}" 2>/dev/null || echo "")
else
	changelog=$(git cliff --unreleased --strip all 2>/dev/null || echo "")
fi

if [[ -z $changelog ]]; then
	echo "Error: No changes found for release" >&2
	exit 1
fi

# Build prompt safely using printf to avoid command substitution on backticks in changelog
prompt=$(
	printf '%s\n' "You are writing release notes for usage version ${version}${prev_version:+ (previous version: ${prev_version})}."
	printf '\n'
	printf '%s\n' "usage is a CLI argument parser library for Rust that generates completions, man pages, and markdown docs from a simple spec format."
	printf '\n'
	printf '%s\n' "Here is the raw changelog from git-cliff:"
	printf '%s\n' "$changelog"
	printf '\n'
	cat <<'INSTRUCTIONS'
Write user-friendly release notes in markdown:

1. First line: `# ` followed by a pithy, descriptive title (not just the version). For smaller or less impactful releases, keep the title understated and modest. Examples:
   - "# PowerShell Support & Bug Fixes"
   - "# Performance Improvements"
   - "# New Completion Features"

TONE CALIBRATION:
- Match the tone and length to the actual significance of the changes
- If the release is mostly small bug fixes or minor tweaks, be upfront about that—a sentence or two of summary is fine, don't write multiple paragraphs inflating the importance
- Reserve enthusiastic, detailed write-ups for releases with genuinely significant features or changes
- It's okay to say "This is a smaller release focused on bug fixes" when that's the case

2. Then a summary proportional to the significance of the changes
3. Organize into ## sections (Highlights, Bug Fixes, etc.) as needed
4. Explain WHY changes matter to users
5. Include PR links and documentation links (https://usage.jdx.dev/)
6. Include contributor usernames (@username). Do not thank @jdx since that is who is writing these notes.
7. Skip internal/trivial changes

Output markdown only, starting with the # title line.
INSTRUCTIONS
)

# Use Claude Code to generate the release notes
# Sandboxed: only read-only tools allowed (no Bash, Edit, Write)
echo "Generating release notes with Claude..." >&2
echo "Version: $version" >&2
echo "Previous version: ${prev_version:-none}" >&2
echo "Changelog length: ${#changelog} chars" >&2

# Capture stderr separately to avoid polluting output
stderr_file=$(mktemp)
trap 'rm -f "$stderr_file"' EXIT

if ! output=$(
	printf '%s' "$prompt" | claude -p \
		--model claude-opus-4-5-20251101 \
		--permission-mode bypassPermissions \
		--output-format text \
		--allowedTools "Read,Grep,Glob" 2>"$stderr_file"
); then
	echo "Error: Claude CLI failed" >&2
	if [[ -s $stderr_file ]]; then
		cat "$stderr_file" >&2
	else
		echo "(no stderr output from Claude CLI)" >&2
	fi
	exit 1
fi

# Validate we got non-empty output
if [[ -z $output ]]; then
	echo "Error: Claude returned empty output" >&2
	cat "$stderr_file" >&2
	exit 1
fi

# Parse title from first line (# Title) and body from rest
first_line=$(echo "$output" | head -n1)
if [[ $first_line == "# "* ]]; then
	title="${first_line#\# }"
	body=$(echo "$output" | tail -n +2)
else
	echo "Warning: First line not a title, using version" >&2
	title="$version"
	body="$output"
fi

# Output format: first line is title, rest is body
echo "$title"
echo "$body"
