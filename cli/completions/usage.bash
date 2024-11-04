_usage() {
    if ! command -v usage &> /dev/null; then
        echo >&2
        echo "Error: usage CLI not found. This is required for completions to work in usage." >&2
        echo "See https://usage.jdx.dev for more information." >&2
        return 1
    fi

    if [[ -z ${_USAGE_SPEC_USAGE:-} ]]; then
        _USAGE_SPEC_USAGE="$(usage --usage-spec)"
    fi

    COMPREPLY=( $(usage complete-word --shell bash -s "${_USAGE_SPEC_USAGE}" --cword="$COMP_CWORD" -- "${COMP_WORDS[@]}" ) )
    if [[ $? -ne 0 ]]; then
        unset COMPREPLY
    fi
    return 0
}

shopt -u hostcomplete && complete -o nospace -o bashdefault -o nosort -F _usage usage
# vim: noet ci pi sts=0 sw=4 ts=4 ft=sh
