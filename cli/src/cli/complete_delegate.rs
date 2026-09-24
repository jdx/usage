//! `complete "<arg>" delegate="<command>"`: the candidates another program's own shell
//! completion offers.
//!
//! A wrapper that forwards its trailing words to another tool (`wrapper <layer> plan -out=…`
//! running `terraform plan -out=…`) wants those words completed the way the shell would
//! complete them after `terraform`. The shell that asked is started once, non-interactively,
//! and asked what it would offer for that command line; its answer comes back as ordinary
//! candidates.
//!
//! The typed words never pass through a shell parser: each script receives them as its own
//! positional arguments and quotes them itself, so a word like `$(rm -rf ~)` is completed,
//! not run.
//!
//! Anything that goes wrong — a shell that is not installed, a command with no completion
//! registered, a shell this does not support, a completion that hangs — yields no candidates
//! rather than an error, and the caller's ordinary fallback applies.

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use crate::env;

/// How long the shell gets to answer. A Tab that stalls is worse than one that offers nothing.
const TIMEOUT: Duration = Duration::from_secs(3);

/// fish completes a command line string, so the words are escaped back into one.
/// `--no-quoted` keeps a partial last word open (`'foo` would read as a closed string), and an
/// empty last word leaves the trailing space that says "a new word starts here".
const FISH: &str = r#"complete -C (string join -- " " (string escape --no-quoted -- $argv))"#;

/// bash-completion's `_command_offset` is how its own `sudo`/`xargs` completions hand the rest of
/// a line to the command it names: it finds that command's compspec (loading it on demand from
/// the completion directories), then runs a `-F` function or a `-C` command against the
/// `COMP_*` variables set here.
const BASH: &str = r#"
if ! declare -F _command_offset >/dev/null; then
  for f in "${BASH_COMPLETION:-}" /usr/share/bash-completion/bash_completion \
    /etc/bash_completion /usr/local/share/bash-completion/bash_completion \
    /opt/homebrew/share/bash-completion/bash_completion; do
    if [[ -n $f && -r $f ]]; then . "$f"; break; fi
  done
fi
declare -F _command_offset >/dev/null || exit 0
COMP_WORDS=("$@")
COMP_CWORD=$(($# - 1))
COMP_LINE=
for w in "${@:1:$#-1}"; do printf -v w '%q ' "$w"; COMP_LINE+=$w; done
COMP_LINE+=${!#}
COMP_POINT=${#COMP_LINE}
COMP_TYPE=9 COMP_KEY=9
export COMP_LINE COMP_POINT COMP_TYPE COMP_KEY
COMPREPLY=()
_command_offset 0
for c in "${COMPREPLY[@]}"; do printf '%s\n' "${c% }"; done
"#;

/// zsh's completion system only runs inside the line editor, so this drives one: an
/// interactive `zsh -f` on a pseudo-terminal, with `compadd` wrapped to print every match it
/// is handed. The line itself arrives in an environment variable and is put into the buffer by
/// a widget, so nothing typed is ever read as keystrokes.
///
/// The widget is bound to `^G` because it is no prefix of another binding: on `^X`, which is,
/// the line editor waited out `KEYTIMEOUT` (0.4s) before running it. `compinit` keeps its dump
/// in usage's cache directory, the one the generated scripts already use, which takes its
/// share of each Tab from about 250ms to a few.
const ZSH: &str = r#"
zmodload zsh/zpty || exit 0
local -a w=("$@")
export __USAGE_DELEGATE_LINE="${(j: :)${(@q)w[1,-2]}} ${(q)w[-1]}"
export __USAGE_DELEGATE_SETUP='
PROMPT= RPROMPT= PS2= ; unsetopt beep zle_bracketed_paste 2>/dev/null
autoload -Uz compinit
__usage_cache=${XDG_CACHE_HOME:-$HOME/.cache}/usage
if mkdir -p -m 700 -- "$__usage_cache" 2>/dev/null; then
  compinit -u -d "$__usage_cache/delegate.zcompdump"
else
  compinit -u -D
fi
compadd() {
  if [[ ${@[1,(i)(-|--)]} == *-(O|A|D)\ * ]]; then builtin compadd "$@"; return; fi
  local -a __h __d __o __p __hp __s __hs
  local __t __i
  __i=${@[(i)-(|l)d]}
  if (( __i < $# )); then
    __t=${@[__i+1]}
    if [[ $__t == \(* ]]; then eval "__d=$__t"; else __d=( "${(@P)__t}" ); fi
  fi
  zparseopts -E -a __o P:=__p p:=__hp S:=__s s:=__hs
  builtin compadd -A __h -D __d "$@"
  for (( __i = 1; __i <= $#__h; __i++ )); do
    print -r -- "__USAGE_DELEGATE_HIT:${(Q)IPREFIX}${__p[2]}${__hp[2]}${(Q)__h[__i]}${__hs[2]}${__s[2]}"$'"'"'\t'"'"'"${${__d[__i]#$__h[__i]}##[[:space:]]#(--|:)[[:space:]]#}"
  done
}
__usage_delegate() {
  BUFFER=$__USAGE_DELEGATE_LINE; CURSOR=$#BUFFER
  zle complete-word
  print -r -- $'"'"'\n'"'"'__USAGE_DELEGATE_DONE; exit
}
zle -N __usage_delegate; bindkey "^G" __usage_delegate
print -r -- __USAGE_DELEGATE_""READY'
zpty __usage_delegate zsh -f -i
zpty -w __usage_delegate 'eval "$__USAGE_DELEGATE_SETUP"'
local out
while zpty -r __usage_delegate out; do [[ $out == *__USAGE_DELEGATE_READY* ]] && break; done
zpty -w -n __usage_delegate $'\x07'
while zpty -r __usage_delegate out; do
  out=${${out//$'\r'/}%$'\n'}
  [[ $out == *__USAGE_DELEGATE_DONE* ]] && break
  [[ $out == *__USAGE_DELEGATE_HIT:* ]] && print -r -- ${out#*__USAGE_DELEGATE_HIT:}
done
zpty -d __usage_delegate
"#;

/// The candidates `shell` offers for `words`, the last of which is the one being completed.
///
/// `words[0]` is the delegated command. Returns nothing for a shell this cannot ask
/// (`powershell`, `nu`) or when the shell cannot answer.
pub(crate) fn complete(shell: &str, words: &[String]) -> Vec<(String, String)> {
    let args: &[&str] = match shell {
        "fish" => &["-c", FISH, "--"],
        "bash" => &["--norc", "--noprofile", "-c", BASH, "usage"],
        "zsh" => &["-f", "-c", ZSH, "usage"],
        _ => {
            debug!("delegate: completion is not delegated in {shell}");
            return vec![];
        }
    };
    // The same program `usage bash`/`usage zsh`/`usage fish` would run.
    let program = env::shell_program_override(shell, |key| std::env::var(key).ok())
        .unwrap_or_else(|| shell.to_string());
    let mut command = Command::new(program);
    command
        .args(args)
        .args(words)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let Some(stdout) = run_with_timeout(command) else {
        return vec![];
    };
    stdout
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once('\t') {
            Some((value, description)) => (value.to_string(), description.trim().to_string()),
            None => (line.to_string(), String::new()),
        })
        .collect()
}

fn run_with_timeout(mut command: Command) -> Option<String> {
    let mut child = command
        .spawn()
        .inspect_err(|err| debug!("delegate: {command:?} failed to start: {err}"))
        .ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on another thread so the wait can give up: end of file arrives once the shell and
    // anything it started have let go of the pipe, which a hung completion never does.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        let _ = tx.send(out);
    });
    let out = rx.recv_timeout(TIMEOUT);
    if out.is_err() {
        debug!("delegate: gave up after {TIMEOUT:?}");
        let _ = child.kill();
    }
    let _ = child.wait();
    String::from_utf8(out.ok()?).ok()
}
