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
use std::time::Duration;

use crate::env;

/// How long the shell gets to answer. A Tab that stalls is worse than one that offers nothing.
const TIMEOUT: Duration = Duration::from_secs(3);
/// How often the wait checks whether the reader has finished.
const POLL: Duration = Duration::from_millis(5);

/// fish completes a command line string, so the words are escaped back into one. The words
/// before the cursor are quoted, so an empty one stays a word (`''`) instead of collapsing
/// into a space and shifting every later word one position left. The last word is escaped
/// with `--no-quoted` instead, which keeps a partial word open (`'foo'` would read as a closed
/// string), and when it is empty leaves the trailing space that says "a new word starts here".
const FISH: &str = r#"complete -C (string join -- " " (string escape -- $argv[1..-2]) (string escape --no-quoted -- $argv[-1]))"#;

/// bash-completion's `_command_offset` is how its own `sudo`/`xargs` completions hand the rest of
/// a line to the command it names: it finds that command's compspec (loading it on demand from
/// the completion directories), then runs a `-F` function or a `-C` command against the
/// `COMP_*` variables set here.
///
/// Those are built the way readline builds them, because bash-completion reassembles words by
/// comparing the two: `COMP_LINE` is the text, and `COMP_WORDS` splits `-out=pl` into `-out`,
/// `=`, `pl`, since `=` is a word break. The generated bash script already hands usage that
/// split form, so a word that is just `=` is glued back to its neighbours in the text. Like
/// readline's, the words before the cursor are the text as typed, quotes included, so an empty
/// one is `''` and keeps its place instead of vanishing.
const BASH: &str = r#"
if ! declare -F _command_offset >/dev/null; then
  for f in "${BASH_COMPLETION:-}" /usr/share/bash-completion/bash_completion \
    /etc/bash_completion /usr/local/share/bash-completion/bash_completion \
    /opt/homebrew/share/bash-completion/bash_completion; do
    if [[ -n $f && -r $f ]]; then . "$f"; break; fi
  done
fi
declare -F _command_offset >/dev/null || exit 0
COMP_WORDS=()
COMP_LINE=
i=0 prev=
for w in "$@"; do
  i=$((i + 1))
  if ((i < $#)); then printf -v q '%q' "$w"; else q=$w; fi
  if [[ -n $COMP_LINE && $q != = && $prev != = ]]; then COMP_LINE+=' '; fi
  COMP_LINE+=$q
  rest=$q
  while [[ $rest == *=* ]]; do
    [[ -n ${rest%%=*} ]] && COMP_WORDS+=("${rest%%=*}")
    COMP_WORDS+=(=)
    rest=${rest#*=}
  done
  if [[ -n $rest ]] || ((i == $# && ${#q} == 0)); then COMP_WORDS+=("$rest"); fi
  prev=$q
done
COMP_CWORD=$((${#COMP_WORDS[@]} - 1))
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
/// a widget, so nothing typed is ever read as keystrokes. `(q)` leaves a plain word as typed
/// (completion functions compare `$words` with names like `commit`) and turns an empty one
/// into `''`, so it keeps its place instead of becoming a gap the line editor skips.
///
/// Every marker carries a nonce chosen for the run, and the end marker must be the whole line,
/// so a candidate that happens to contain marker text is returned rather than ending the read.
///
/// The widget is bound to `^G` because it is no prefix of another binding: on `^X`, which is,
/// the line editor waited out `KEYTIMEOUT` (0.4s) before running it. `compinit` keeps its dump
/// in usage's cache directory, the one the generated scripts already use, which takes its
/// share of each Tab from about 250ms to a few.
const ZSH: &str = r#"
zmodload zsh/zpty || exit 0
zmodload zsh/datetime 2>/dev/null
export __USAGE_DELEGATE_NONCE=__usage_delegate_$$_${RANDOM}${RANDOM}_${EPOCHREALTIME//[^0-9]/}
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
    print -r -- "$__USAGE_DELEGATE_NONCE:HIT:${(Q)IPREFIX}${__p[2]}${__hp[2]}${(Q)__h[__i]}${__hs[2]}${__s[2]}"$'"'"'\t'"'"'"${${__d[__i]#$__h[__i]}##[[:space:]]#(--|:)[[:space:]]#}"
  done
}
__usage_delegate() {
  BUFFER=$__USAGE_DELEGATE_LINE; CURSOR=$#BUFFER
  zle complete-word
  print -r -- $'"'"'\n'"'"'"$__USAGE_DELEGATE_NONCE:DONE"; exit
}
zle -N __usage_delegate; bindkey "^G" __usage_delegate
print -r -- "$__USAGE_DELEGATE_NONCE:READY"'
zpty __usage_delegate zsh -f -i
zpty -w __usage_delegate 'eval "$__USAGE_DELEGATE_SETUP"'
local out n=$__USAGE_DELEGATE_NONCE
while zpty -r __usage_delegate out; do
  [[ ${${out//$'\r'/}%$'\n'} == "$n:READY" ]] && break
done
zpty -w -n __usage_delegate $'\x07'
while zpty -r __usage_delegate out; do
  out=${${out//$'\r'/}%$'\n'}
  [[ $out == "$n:DONE" ]] && break
  [[ $out == *"$n:HIT:"* ]] && print -r -- ${out#*"$n:HIT:"}
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
    // Its own process group, so a timeout can take down whatever the completion started along
    // with the shell (see `run_with_timeout`).
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
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
        .inspect_err(|err| {
            let program = command.get_program().to_string_lossy();
            debug!("delegate: {program} failed to start: {err}")
        })
        .ok()?;
    let mut stdout = child.stdout.take()?;
    // Read on another thread so the wait can give up: end of file arrives once the shell and
    // anything it started have let go of the pipe, which a hung completion never does.
    //
    // Polled rather than sent over a channel: a channel costs the binary some 15 KB of
    // `std::sync::mpmc` for one message, and a few milliseconds of polling is nothing beside
    // a shell starting up.
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        let _ = stdout.read_to_end(&mut out);
        out
    });
    let deadline = std::time::Instant::now() + TIMEOUT;
    while !reader.is_finished() && std::time::Instant::now() < deadline {
        std::thread::sleep(POLL);
    }
    let out = if reader.is_finished() {
        reader.join().ok()
    } else {
        debug!("delegate: gave up after {}s", TIMEOUT.as_secs());
        kill_tree(&mut child);
        // Not joined: it may still be blocked on a pipe that some process outside the group
        // holds, and `complete-word` must not wait for that to exit.
        None
    };
    let _ = child.wait();
    String::from_utf8(out?).ok()
}

/// Kill the shell and everything else in its process group.
///
/// Killing the shell alone left a completion's own child running when that child held the
/// pipe open — the very thing that made the read time out — and every further Tab added
/// another. The group is gone even after its leader has exited, as long as a member remains,
/// which is exactly that case.
///
/// `killpg` rather than the `kill` program, whose spellings disagree: procps needs a `--`
/// before `-<pgid>` (without it, once the leader has exited it reports success and kills
/// nothing), and the BSD `kill` on macOS does not take one.
#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) {
    let Ok(group) = libc::pid_t::try_from(child.id()) else {
        let _ = child.kill();
        return;
    };
    // SAFETY: `killpg` takes plain integers and touches no memory. The group is the one
    // `process_group(0)` created for this child, whose id is the child's pid; the child has
    // not been waited on yet, so that id cannot have been reused.
    if unsafe { libc::killpg(group, libc::SIGKILL) } != 0 {
        let _ = child.kill();
    }
}

/// Windows has no process groups to reach for here; the shell itself is what can be killed.
#[cfg(not(unix))]
fn kill_tree(child: &mut std::process::Child) {
    let _ = child.kill();
}
