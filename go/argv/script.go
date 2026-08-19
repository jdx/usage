package argv

import (
	"fmt"
	"strings"
)

// The shell scripts that call the hidden completion command.
//
// Each is a handful of lines, because the thinking is on the other side: the
// binary answers with candidates and, when paths belong there, a marker. So a
// script's whole job is to hand over the line and the cursor, present what comes
// back the way its shell presents things, and pass the position to the shell's
// own path completion if the marker appeared.
//
// What that replaces is worth stating. mise's current scripts hard-fail unless
// the separate `usage` CLI is installed, dump a spec into `$XDG_CACHE_HOME`,
// prune stale spec files by age, and shell out to `usage complete-word` on every
// Tab. None of that is here: the binary was compiled with the tables.
//
// Ported from usage-argv's `script.rs`, script for script. They call the same
// hidden command with the same arguments, so a shell cannot tell which language
// answered — and a fix to one shell's quirks is a fix to both.

// Script is the completion script for `bin` in `shell`, ready to be written to a
// file or sourced.
//
// The binary is named rather than found: a script that resolved the binary itself
// would complete against whichever copy came first on `PATH`, which is not always
// the one the user is typing.
//
// Every invocation quotes the name, so one containing a space still *runs*.
// Registering it is another matter: zsh's `#compdef` line is a magic comment read
// by `compinit` before any shell quoting happens, and there is nowhere to put a
// quote in it. A binary whose name is not a single shell word therefore cannot be
// completed in zsh by anyone, which the panic below says out loud rather than
// leaving to be discovered at a prompt.
func Script(bin string, shell Shell) string {
	// A panic, not a returned error: the alternative is a program quietly writing
	// a script that registers half a name, or one whose apostrophe closes the
	// quoting around it and turns the rest into something else entirely. The name
	// comes from the spec its author wrote, not from anything a user typed, so
	// this is a mistake surfacing where it can be fixed.
	if !plainWord(bin) {
		panic(fmt.Sprintf("a completion script cannot register %q: a binary's name has to be "+
			"one plain shell word, and zsh's `#compdef` line has nowhere to put a quote even "+
			"if it were quoted everywhere else", bin))
	}
	switch shell {
	case Zsh:
		// The header goes *after* the magic comment here, not before it. `compinit`
		// reads only the first line of a file in `$fpath` looking for `#compdef`,
		// so a script that leads with a comment of its own is autoloaded and never
		// registers — and this one documents being dropped in `$fpath`.
		return strings.NewReplacer("{bin}", bin, "{shell}", "zsh").
			Replace("#compdef {bin}\n" + header + zshScript)
	case Fish:
		return fill(fishScript, bin)
	case Nu:
		return strings.NewReplacer("{bin}", bin, "{ident}", nuIdent(bin),
			"{shell}", "nu").Replace(header + nuScript)
	case PowerShell:
		return fill(powershellScript, bin)
	}
	return fill(bashScript, bin)
}

// plainWord reports whether a name is one the five shells can all register. The
// accepted set is what binaries are actually called.
func plainWord(bin string) bool {
	if bin == "" {
		return false
	}
	for _, c := range bin {
		switch {
		case c >= 'a' && c <= 'z', c >= 'A' && c <= 'Z', c >= '0' && c <= '9':
		case c == '-' || c == '_' || c == '.' || c == '+':
		default:
			return false
		}
	}
	return true
}

// nuIdent is a nushell identifier for a binary's name, one name to one
// identifier.
//
// The other four shells take a binary's name verbatim as part of a function name
// — bash, zsh and fish all accept `-`, `.` and `+` there, which is worth knowing
// because sanitizing them away is what makes two names collide. nushell binds a
// variable, where `-` would be read as subtraction, so its name has to be escaped
// rather than flattened: flattening mapped `foo-bar` and `foo+bar` both to
// `foo_bar`, and two scripts loaded together would each have completed the
// other's binary.
func nuIdent(bin string) string {
	var out strings.Builder
	for _, c := range bin {
		if c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' {
			out.WriteRune(c)
			continue
		}
		// The underscore is escaped too, so that no escape can be spelled by hand
		// into a name and collide with the character it stands for.
		fmt.Fprintf(&out, "_x%02x", c)
	}
	return out.String()
}

func fill(script, bin string) string {
	return strings.NewReplacer("{bin}", bin, "{shell}", shellFlag(script)).
		Replace(header + script)
}

// shellFlag is the `--shell` name a script passes, read out of the script itself
// so the header cannot disagree with the call below it.
func shellFlag(script string) string {
	const marker = "--shell "
	i := strings.Index(script, marker)
	if i < 0 {
		return ""
	}
	rest := script[i+len(marker):]
	if j := strings.IndexAny(rest, " \\\n\t"); j >= 0 {
		return rest[:j]
	}
	return rest
}

const header = `# @generated by usage for ` + "`" + `{bin} __complete_word__ --shell {shell}` + "`" + `
# Do not edit: regenerate it. Needs no other program, and no cached spec —
# the binary answers from the tables it was compiled with.
`

const bashScript = `
_usage_complete_{bin}() {
    local __usage_out __usage_line __usage_files=
    # Truncated here rather than passed with an offset: every shell counts a cursor in its own
    # units — characters in a UTF-8 locale for bash and zsh, characters for fish and PowerShell —
    # and a number that means one thing here and another there is a bug waiting for a non-ASCII
    # command line. Cut with the shell's own offset, the units cancel out and what arrives is
    # exactly the text before the cursor.
    __usage_out="$(command '{bin}' __complete_word__ --shell bash \
        --line "${COMP_LINE:0:$COMP_POINT}" 2>/dev/null)" || return 1

    COMPREPLY=()
    while IFS= read -r __usage_line; do
        case "$__usage_line" in
            $'\001files') __usage_files=any ;;
            $'\001dirs') __usage_files=dirs ;;
            $'\001executables') __usage_files=executables ;;
            $'\001commands') __usage_files=commands ;;
            '') ;;
            *) COMPREPLY+=("$__usage_line") ;;
        esac
    done <<< "$__usage_out"

    if [[ -n $__usage_files ]]; then
        # Set here rather than on ` + "`" + `complete` + "`" + `, because whether this position takes a path is not
        # known until the answer comes back. It is what makes bash append a ` + "`" + `/` + "`" + ` to a directory
        # and stop escaping what it should not.
        [[ $__usage_files == commands ]] || compopt -o filenames 2>/dev/null
        local __usage_cur="${COMP_WORDS[COMP_CWORD]}" __usage_path
        local -a __usage_paths=()
        if [[ $__usage_files == commands ]]; then
            while IFS= read -r __usage_path; do __usage_paths+=("$__usage_path"); done \
                < <(compgen -c -- "$__usage_cur")
        elif [[ $__usage_files == executables ]]; then
            while IFS= read -r __usage_path; do
                [[ -d "$__usage_path" || -x "$__usage_path" ]] && __usage_paths+=("$__usage_path")
            done < <(compgen -f -- "$__usage_cur")
        elif [[ $__usage_files == dirs ]]; then
            while IFS= read -r __usage_path; do __usage_paths+=("$__usage_path"); done \
                < <(compgen -d -- "$__usage_cur")
        else
            while IFS= read -r __usage_path; do __usage_paths+=("$__usage_path"); done \
                < <(compgen -f -- "$__usage_cur")
        fi
        # Guarded, because an empty array expands to one empty word in older bash.
        (( ${#__usage_paths[@]} )) && COMPREPLY+=("${__usage_paths[@]}")
    fi
}
complete -F _usage_complete_{bin} '{bin}'
`

const zshScript = `
_{bin}() {
    local -a values=() descriptions=() inserts=()
    local __usage_files= __usage_line __usage_menu=0
    # ` + "`" + `$BUFFER[1,CURSOR]` + "`" + ` is the text before the cursor, cut with zsh's own offset — see the
    # bash script on why the cutting happens here rather than through a ` + "`" + `--cursor` + "`" + ` argument.
    while IFS= read -r __usage_line; do
        case "$__usage_line" in
            $'\001files') __usage_files=any; continue ;;
            $'\001dirs') __usage_files=dirs; continue ;;
            $'\001executables') __usage_files=executables; continue ;;
            $'\001commands') __usage_files=commands; continue ;;
            '') continue ;;
        esac
        local -a parts=("${(@ps:\t:)__usage_line}")
        values+=("${parts[1]}")
        descriptions+=("${parts[2]}")
        inserts+=("${parts[3]}")
        # A quoted insert means the value needed quoting, and zsh should offer a menu rather
        # than silently inserting one of several possibilities.
        [[ "${parts[3]}" == "'"* ]] && __usage_menu=1
    done < <(command '{bin}' __complete_word__ --shell zsh \
        --line "${BUFFER[1,CURSOR]}" 2>/dev/null)

    local __usage_ret=1
    (( __usage_menu )) && compstate[insert]=menu
    if (( ${#inserts[@]} )); then
        local -a display=()
        local i max=0 value pad
        for value in "${values[@]}"; do
            (( ${#value} > max )) && max=${#value}
        done
        for (( i = 1; i <= ${#values[@]}; i++ )); do
            if [[ -n "${descriptions[i]}" ]]; then
                pad=$(( max - ${#values[i]} ))
                display+=("${values[i]}${(l:pad:: :)}  -- ${descriptions[i]}")
            else
                display+=("${values[i]}")
            fi
        done
        # ` + "`" + `-U` + "`" + ` because the binary already filtered by the typed prefix, and ` + "`" + `-Q` + "`" + ` because the
        # inserts are quoted already.
        compadd -l -d display -U -Q -S '' -a inserts && __usage_ret=0
    fi

    case "$__usage_files" in
        any) _files && __usage_ret=0 ;;
        dirs) _files -/ && __usage_ret=0 ;;
        executables) _files -g '*(*)' && __usage_ret=0 ;;
        commands) _command_names && __usage_ret=0 ;;
    esac
    return $__usage_ret
}

# Installed either way. Dropped in ` + "`" + `$fpath` + "`" + ` as ` + "`" + `_{bin}` + "`" + `, compinit autoloads the file and calls
# the function named after it — which is why the function is ` + "`" + `_{bin}` + "`" + ` and not something tidier.
# Sourced from a config instead, nothing has called it yet, so it registers itself.
if [ "$funcstack[1]" = "_{bin}" ]; then
    _{bin} "$@"
else
    compdef _{bin} '{bin}'
fi
`

const fishScript = `
function __usage_complete_{bin}
    set -l line (commandline -cp)
    # ` + "`" + `commandline -cp` + "`" + ` is already cut at the cursor, so there is nothing to say about where
    # the cursor is: the end of what it gives is where the cursor was.
    set -l out (command '{bin}' __complete_word__ --shell fish --line "$line" 2>/dev/null)
    # Built with printf rather than written literally: fish's ` + "`" + `case` + "`" + ` takes patterns, not
    # computed values, and a control byte is not something to spell twice.
    set -l marker_any (printf '\x01files')
    set -l marker_dirs (printf '\x01dirs')
    set -l marker_executables (printf '\x01executables')
    set -l marker_commands (printf '\x01commands')
    set -l files ""
    for entry in $out
        if test "$entry" = "$marker_any"
            set files any
        else if test "$entry" = "$marker_dirs"
            set files dirs
        else if test "$entry" = "$marker_executables"
            set files executables
        else if test "$entry" = "$marker_commands"
            set files commands
        else if test -n "$entry"
            # printf, not echo: fish's echo reads a leading -n, -e, -s or -E as
            # its own option, so those flags would be swallowed or mangled rather
            # than offered — and a CLI with a -n is not unusual.
            printf '%s\n' $entry
        end
    end
    # fish's own path completion, which knows about ` + "`" + `~` + "`" + `, variables and remote paths.
    switch $files
        case any
            __fish_complete_path (commandline -ct)
        case dirs
            __fish_complete_directories (commandline -ct)
        case executables
            for candidate in (__fish_complete_path (commandline -ct))
                set -l value (string split -m 1 (printf '\t') -- $candidate)[1]
                if test -d "$value"; or test -x "$value"
                    printf '%s\n' $candidate
                end
            end
        case commands
            __fish_complete_command (commandline -ct)
    end
end

# ` + "`" + `-f` + "`" + ` so fish offers no filenames of its own: this CLI says when they belong, and the
# function produces them itself when they do.
complete -c '{bin}' -f -a '(__usage_complete_{bin})'
`

const nuScript = `
def --env __usage_complete_{ident} [spans: list<string>] {
    let line = ($spans | each {|span|
        if ($span | str contains " ") { $'"($span)"' } else { $span }
    } | str join " ")
    let out = (^{bin} __complete_word__ --shell nu --line $line | complete)
    if $out.exit_code != 0 { return null }
    let lines = ($out.stdout | lines | where {|l| $l != "" })
    let marker = "\u{1}"
    let wants_files = ($lines | any {|l| $l == $marker + "files" or $l == $marker + "dirs" or $l == $marker + "executables" })
    let wants_commands = ($lines | any {|l| $l == $marker + "commands" })
    let declared = (
        $lines
        | where {|l| not ($l | str starts-with $marker) }
        | each {|l|
            let parts = ($l | split row (char tab))
            {
                value: ($parts | get 0)
                description: (if ($parts | length) > 1 { $parts | get 1 } else { "" })
            }
        }
    )
    # which with no names returns nushell's commands and the executables on PATH.
    # Unlike commandline complete, it cannot reinterpret an exact command name as
    # the start of that command's arguments or re-enter this external completer.
    let commands = (if $wants_commands {
        let prefix = ($spans | last)
        which
        | where {|row| $row.command | str starts-with $prefix }
        | each {|row| { value: $row.command, description: $row.path } }
    } else {
        []
    })
    let candidates = ($declared | append $commands)
    # ` + "`" + `null` + "`" + ` is how a nushell completer says "you do this one", and what it does is complete
    # paths. So an answer that is only the marker returns null rather than nothing, which would
    # mean "there is nothing here".
    #
    # Where there are candidates *and* paths, nushell can be told one or the other and not both:
    # returning a list means "these are the completions", and there is no option beside it for
    # "and files too". The candidates win, because they are what this CLI knows and a path is
    # something the user can finish typing. Every other shell here appends both; this is
    # nushell's completer interface rather than a decision of this design.
    if ($candidates | is-empty) and $wants_files { null } else { $candidates }
}

# Slot this into the external completer nushell already has, if any, rather than replacing it:
# a config that completes several tools should keep completing all of them.
let __usage_previous_{ident} = ($env.config.completions.external.completer? | default null)
$env.config.completions.external.completer = {|spans|
    if ($spans | get 0) == "{bin}" {
        __usage_complete_{ident} $spans
    } else if $__usage_previous_{ident} != null {
        do $__usage_previous_{ident} $spans
    } else {
        null
    }
}
`

const powershellScript = `
Register-ArgumentCompleter -Native -CommandName '{bin}' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $marker = [char]1
    # ` + "`" + `$cursorPosition` + "`" + ` is an offset into the whole input buffer while ` + "`" + `Extent.Text` + "`" + ` is only
    # this command's span, so after a pipeline or a ` + "`" + `;` + "`" + ` the two do not share an origin.
    # Subtracting the extent's start puts them back on the same footing, and cutting here means
    # no offset has to travel — see the bash script on why.
    $extent = $commandAst.Extent
    $offset = $cursorPosition - $extent.StartOffset
    if ($offset -lt 0) { $offset = 0 }
    if ($offset -gt $extent.Text.Length) { $offset = $extent.Text.Length }
    $line = $extent.Text.Substring(0, $offset)
    $out = @(& '{bin}' __complete_word__ --shell powershell --line $line 2>$null)

    $files = $null
    $results = [System.Collections.Generic.List[System.Management.Automation.CompletionResult]]::new()
    foreach ($entry in $out) {
        if ([string]::IsNullOrEmpty($entry)) { continue }
        if ($entry -eq ($marker + 'files')) { $files = 'any'; continue }
        if ($entry -eq ($marker + 'dirs')) { $files = 'dirs'; continue }
        if ($entry -eq ($marker + 'executables')) { $files = 'executables'; continue }
        if ($entry -eq ($marker + 'commands')) { $files = 'commands'; continue }
        $parts = $entry -split "` + "`" + `t", 2
        $value = $parts[0]
        $description = if ($parts.Count -gt 1 -and $parts[1]) { $parts[1] } else { $value }
        $results.Add(
            [System.Management.Automation.CompletionResult]::new(
                $value, $value, 'ParameterValue', $description
            )
        )
    }

    if ($files -eq 'commands') {
        foreach ($command in Get-Command -Name ($wordToComplete + '*') -CommandType Application, ExternalScript -ErrorAction SilentlyContinue) {
            $results.Add(
                [System.Management.Automation.CompletionResult]::new(
                    $command.Name, $command.Name, 'Command', $command.Source
                )
            )
        }
    } elseif ($files) {
        # PowerShell's own, so that ` + "`" + `~` + "`" + `, drive-relative paths and provider paths behave as they
        # do everywhere else in the shell.
        foreach ($path in [System.Management.Automation.CompletionCompleters]::CompleteFilename($wordToComplete)) {
            # Trust PowerShell's result type for directories because CompletionText may already
            # carry quoting. Executable leaves are checked as commands after stripping only the
            # outer quote characters PowerShell added.
            if ($files -eq 'dirs' -and $path.ResultType -ne 'ProviderContainer') {
                continue
            }
            if ($files -eq 'executables' -and $path.ResultType -ne 'ProviderContainer') {
                $candidatePath = $path.CompletionText.Trim([char[]]@([char]39, [char]34))
                if (-not (Get-Command -Name $candidatePath -CommandType Application, ExternalScript -ErrorAction SilentlyContinue)) {
                    continue
                }
            }
            $results.Add($path)
        }
    }

    $results
}
`
