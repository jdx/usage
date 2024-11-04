# if "usage" is not installed show an error
if ! command -v usage &> /dev/null
    echo >&2
    echo "Error: usage CLI not found. This is required for completions to work in usage." >&2
    echo "See https://usage.jdx.dev for more information." >&2
    return 1
end

set _usage_spec_usage (usage --usage-spec | string collect)
complete -xc usage -a '(usage complete-word --shell fish -s "$_usage_spec_usage" -- (commandline -cop) (commandline -t))'
