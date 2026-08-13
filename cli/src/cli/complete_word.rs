use std::collections::BTreeMap;
use std::env;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Args;
use itertools::Itertools;
use miette::IntoDiagnostic;
use std::sync::LazyLock;
use xx::regex;

use usage::parse::{ParseOutput, ParseValue};
use usage::sh::sh;
use usage::spec::config::SpecConfigProp;
use usage::spec::config_type::{Base, SpecConfigType};
use usage::{Spec, SpecArg, SpecCommand, SpecComplete, SpecDoubleDashChoices, SpecFlag};

use crate::cli::generate;

/// Generate shell completion candidates for a partial command line
///
/// This is used internally by shell completion scripts to provide
/// intelligent completions for commands, flags, and arguments.
#[derive(Debug, Args)]
#[clap(visible_alias = "cw")]
pub struct CompleteWord {
    /// User's input from the command line
    words: Vec<String>,

    /// Usage spec file or script with usage shebang, use "-" to read from stdin
    #[clap(short, long)]
    file: Option<PathBuf>,

    /// Raw string spec input
    #[clap(short, long, required_unless_present = "file", overrides_with = "file")]
    spec: Option<String>,

    /// Current word index
    #[clap(long, allow_hyphen_values = true)]
    cword: Option<usize>,

    #[clap(long, default_value = "bash", value_parser = ["bash", "fish", "nu", "powershell", "zsh"])]
    shell: String,
}

impl CompleteWord {
    pub fn run(&self) -> miette::Result<()> {
        let spec = generate::file_or_spec(&self.file, &self.spec)?;
        let choices = self.complete_word(&spec)?;
        let shell = self.shell.as_ref();
        let any_descriptions = choices.iter().any(|(_, d)| !d.is_empty());
        for (c, description) in choices {
            match shell {
                "bash" => println!("{c}"),
                "fish" | "nu" | "powershell" => {
                    if any_descriptions {
                        println!("{c}\t{description}")
                    } else {
                        println!("{c}")
                    }
                }
                "zsh" => {
                    // Three tab-separated columns per line:
                    //   1. The raw value (used as the menu display label).
                    //   2. The description (may be empty).
                    //   3. The shell-quoted form that `compadd -Q` should
                    //      insert verbatim — wrapped in single quotes when
                    //      the value contains shell metacharacters, raw
                    //      otherwise.
                    // The generated zsh script builds the formatted display
                    // (`value -- description`) from columns 1 and 2 and uses
                    // column 3 as the inserted match. Keeping these as three
                    // distinct fields avoids the `\:`-escaping acrobatics
                    // that `_describe`'s `value:description` format required.
                    let insert = zsh_shell_quote(&c);
                    println!("{c}\t{description}\t{insert}")
                }
                _ => {
                    miette::bail!("unsupported shell: {}", shell);
                }
            }
        }

        Ok(())
    }

    fn complete_word(&self, spec: &Spec) -> miette::Result<Vec<(String, String)>> {
        let cword = self.cword.unwrap_or(self.words.len().max(1) - 1);
        let ctoken = self.words.get(cword).cloned().unwrap_or_default();
        let words: Vec<_> = self.words.iter().take(cword).cloned().collect();

        trace!(
            "cword: {cword} ctoken: {ctoken} words: {}",
            words.iter().join(" ")
        );

        let mut ctx = tera::Context::new();
        ctx.insert("words", &self.words);
        ctx.insert("CURRENT", &cword);
        if cword > 0 {
            ctx.insert("PREV", &(cword - 1));
        }

        let parsed = usage::parse::parse_partial(spec, &words)?;
        debug!("parsed cmd: {}", parsed.cmd.full_cmd.join(" "));

        // Check if previous token was a restart_token - if so, complete from first arg
        let prev_token = if cword > 0 {
            self.words.get(cword - 1).map(|s| s.as_str())
        } else {
            None
        };
        let after_restart_token = parsed
            .cmd
            .restart_token
            .as_ref()
            .is_some_and(|rt| prev_token == Some(rt.as_str()));

        let cx = Ctx {
            tera: &ctx,
            spec,
            parsed: &parsed,
        };
        let mut has_explicit_choices = false;
        // Not `available_flags`: inside a mounted command, the mounting CLI's flags stay
        // recognized for parsing but are not accepted there, so they must not be offered.
        let flags = parsed.completion_flags();
        // An explicit `--` stops the parser reading flags, so past one there is no such thing
        // as a flag to complete — a dash-prefixed word is a positional value.
        let flags_possible = !parsed.double_dash_seen;
        let mut choices = if flags_possible && ctoken == "-" {
            let shorts = self.complete_short_flag_names(&flags, "");
            let longs = self.complete_long_flag_names(&flags, "");
            shorts.into_iter().chain(longs).collect::<Vec<_>>()
        } else if flags_possible && ctoken.starts_with("--") {
            self.complete_long_flag_names(&flags, &ctoken)
        } else if flags_possible && ctoken.starts_with('-') {
            self.complete_short_flag_names(&flags, &ctoken)
        } else if after_restart_token {
            // After a restart_token, complete from the first arg of the current command
            // This must be checked after flag checks (to allow --flag after :::)
            // but before flag_awaiting_value (since restart clears pending flag values)
            let mut choices = vec![];
            if let Some(arg) = parsed.cmd.args.first() {
                let (found, constrained) = self.complete_positional(
                    &cx,
                    &parsed.cmd,
                    arg,
                    &ctoken,
                    parsed.double_dash_seen,
                )?;
                has_explicit_choices = constrained;
                choices.extend(found);
            }
            choices
        } else if let Some(flag) = parsed.flag_awaiting_value.first() {
            let arg = flag.arg.as_ref().unwrap();
            let (found, closed) = self.complete_arg(&cx, &parsed.cmd, arg, &ctoken)?;
            has_explicit_choices = closed || arg.choices.is_some();
            found
        } else {
            let mut choices = vec![];
            if let Some(arg) = parsed.next_arg.as_deref() {
                let (found, constrained) = self.complete_positional(
                    &cx,
                    &parsed.cmd,
                    arg,
                    &ctoken,
                    parsed.double_dash_seen,
                )?;
                has_explicit_choices = constrained;
                choices.extend(found);
            }
            if !parsed.cmd.subcommands.is_empty() {
                choices.extend(self.complete_subcommands(&parsed.cmd, &ctoken));
            }
            // If at root command with default_subcommand, also include completions from it
            if parsed.cmd.name == spec.cmd.name {
                if let Some(default_name) = &spec.default_subcommand {
                    if let Some(default_cmd) = spec.cmd.find_subcommand(default_name) {
                        // Include completions from default subcommand's first arg.
                        //
                        // The `constrained` half is dropped on purpose: unlike the two call
                        // sites above, this arg belongs to a *different* command and is only
                        // a guess that the user means to elide the subcommand name. Letting
                        // its choices set `has_explicit_choices` would suppress the root
                        // command's own file fallback whenever the token failed to match
                        // them — see `complete_word_default_subcommand_choices_do_not_block_
                        // root_file_fallback`. The `double_dash="required"` rule does apply,
                        // which is why this goes through the helper at all.
                        if let Some(arg) = default_cmd.args.first() {
                            let (found, _) = self.complete_positional(
                                &cx,
                                default_cmd,
                                arg,
                                &ctoken,
                                parsed.double_dash_seen,
                            )?;
                            choices.extend(found);
                        }
                    }
                }
            }
            choices
        };
        // Fallback to file completions if nothing is known about this argument and it's not a
        // flag. Past a `--` a dash-prefixed word is not a flag but a value, so a path like
        // `-input` still gets completed there.
        let looks_like_a_flag = flags_possible && ctoken.starts_with('-');
        if choices.is_empty() && !looks_like_a_flag && !has_explicit_choices {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let files = self.complete_path(&cwd, &ctoken, |_| true);
            choices = files.into_iter().map(|n| (n, String::new())).collect();
        }
        trace!("choices: {}", choices.iter().map(|(c, _)| c).join(", "));
        Ok(choices)
    }

    fn complete_subcommands(&self, cmd: &SpecCommand, ctoken: &str) -> Vec<(String, String)> {
        trace!("complete_subcommands: {ctoken}");
        let mut choices = vec![];
        for subcommand in cmd.subcommands.values() {
            if subcommand.hide {
                continue;
            }
            choices.push((
                subcommand.name.clone(),
                subcommand.help.clone().unwrap_or_default(),
            ));
            for alias in &subcommand.aliases {
                choices.push((alias.clone(), subcommand.help.clone().unwrap_or_default()));
            }
        }
        choices
            .into_iter()
            .filter(|(c, _)| c.starts_with(ctoken))
            .sorted()
            .collect()
    }

    fn complete_long_flag_names(
        &self,
        flags: &BTreeMap<String, Arc<SpecFlag>>,
        ctoken: &str,
    ) -> Vec<(String, String)> {
        debug!("complete_long_flag_names: {ctoken}");
        trace!("flags: {}", flags.keys().join(", "));
        flags
            .values()
            .filter(|f| !f.hide)
            .flat_map(|f| {
                let mut flags = f
                    .long
                    .iter()
                    .map(|l| (format!("--{l}"), f.help.clone().unwrap_or_default()))
                    .collect::<Vec<_>>();
                if let Some(negate) = &f.negate {
                    flags.push((negate.clone(), String::new()))
                }
                flags
            })
            .unique_by(|(f, _)| f.to_string())
            .filter(|(f, _)| f.starts_with(ctoken))
            // TODO: get flag description
            .sorted()
            .collect()
    }

    fn complete_short_flag_names(
        &self,
        flags: &BTreeMap<String, Arc<SpecFlag>>,
        ctoken: &str,
    ) -> Vec<(String, String)> {
        debug!("complete_short_flag_names: {ctoken}");
        let cur = ctoken.chars().nth(1);
        flags
            .values()
            .filter(|f| !f.hide)
            .flat_map(|f| &f.short)
            .unique()
            .filter(|c| cur.is_none() || cur == Some(**c))
            // TODO: get flag description
            .map(|c| (format!("-{c}"), String::new()))
            .sorted()
            .collect()
    }

    /// Completions for a reserved `type=`, and whether the set they came from is *closed*.
    ///
    /// Closed means an unmatched prefix has no completions at all, rather than falling
    /// through to the file fallback: there is a known set of settings, and `config set
    /// log_leve<TAB>` offering the contents of the working directory is worse than offering
    /// nothing. `file`/`path`/`dir` are the opposite — they *are* the fallback — so they stay
    /// open and an empty result there means only that the directory had no match.
    fn complete_builtin(
        &self,
        cx: &Ctx<'_>,
        type_: &str,
        ctoken: &str,
    ) -> (Vec<(String, String)>, bool) {
        // The two config completers describe values, so they carry their own descriptions
        // rather than going through the path branch's empty ones.
        match type_ {
            "config_keys" => return (self.complete_config_keys(cx.spec, ctoken), true),
            "config_values" => return self.complete_config_values(cx, ctoken),
            _ => {}
        }
        let names = match (type_, env::current_dir()) {
            ("path" | "file", Ok(cwd)) => self.complete_path(&cwd, ctoken, |_| true),
            ("dir", Ok(cwd)) => self.complete_path(&cwd, ctoken, |p| p.is_dir()),
            // ("file", Ok(cwd)) => self.complete_path(&cwd, ctoken, |p| p.is_file()),
            _ => vec![],
        };
        (
            names.into_iter().map(|n| (n, String::new())).collect(),
            false,
        )
    }

    /// The settings a `config` block declares, for the key argument of a `config get`/`set`.
    ///
    /// Every CLI in the fleet writes this by hand — a `run=` shell command that asks the
    /// binary for its own settings list. Declared as `type="config_keys"`, the spec already
    /// says what the keys are, so the completion needs no subprocess.
    ///
    /// `hide` filters here, which is what distinguishes this from the JSON schema: a hidden
    /// setting is still settable, so a schema must accept it, but nothing should suggest it.
    fn complete_config_keys(&self, spec: &Spec, ctoken: &str) -> Vec<(String, String)> {
        spec.config
            .props
            .iter()
            .filter(|(_, prop)| !prop.hide)
            .filter(|(key, _)| key.starts_with(ctoken))
            .map(|(key, prop)| {
                let help = one_line(prop.help.as_deref());
                let help = help.as_str();
                // Still offered when deprecated — it remains settable, and a config file in
                // the wild still names it — but never without saying so.
                let description = match &prop.deprecated {
                    Some(_) if help.is_empty() => "deprecated".to_string(),
                    Some(_) => format!("deprecated — {help}"),
                    None => help.to_string(),
                };
                (key.clone(), description)
            })
            .collect()
    }

    /// The values the setting named earlier on the command line accepts.
    ///
    /// Its `choices` when it declares them, each with its own help; `true`/`false` for a
    /// boolean. Anything else returns nothing, which lets the file fallback do the obvious
    /// thing for a path-valued setting.
    fn complete_config_values(&self, cx: &Ctx<'_>, ctoken: &str) -> (Vec<(String, String)>, bool) {
        let Some(prop) = self.config_key_before_cursor(cx) else {
            // Not a setting at all, so there is nothing to be authoritative about and the
            // usual fallback is as good an answer as any.
            return (vec![], false);
        };
        if !prop.choices.is_empty() {
            return (
                prop.choices
                    .iter()
                    .map(|choice| (choice.value.display(), one_line(choice.help.as_deref())))
                    .filter(|(value, _)| value.starts_with(ctoken))
                    .collect(),
                // Closed: a spec that lists `choices` is declaring what the setting accepts,
                // whatever its base type says. `mise`'s `python.uv_venv_auto` is `bool|string`
                // and lists all four of its values.
                true,
            );
        }
        // Any boolean anywhere in the type, so `string|bool` behaves like `bool|string`:
        // `simplified()` returns a union's *first* member, which made the two words appear or
        // not depending on the order the spec happened to list them in.
        if holds_a_boolean(&prop.value_type.clone().unwrap_or_default()) {
            let declared = prop.value_type.clone().unwrap_or_default();
            return (
                ["false", "true"]
                    .into_iter()
                    .filter(|value| value.starts_with(ctoken))
                    .map(|value| (value.to_string(), String::new()))
                    .collect(),
                // `bool|path` accepts both words *and* any path, so the two words are worth
                // offering but they are not the whole set: claiming they were meant a prefix
                // like `src/` completed to nothing at all.
                !accepts_unenumerable_values(&declared),
            );
        }
        // A path, a number, free text: the spec does not enumerate what belongs here, so the
        // file fallback is left to do what it does for any other unconstrained argument.
        (vec![], false)
    }

    /// The setting a `config_values` completion is for: the most recent *positional* value
    /// before the cursor that names one.
    ///
    /// Taken from the parser's own bindings rather than by scanning the raw words, because the
    /// key's place on the line is the CLI's business — `config set jobs 4`,
    /// `config --global set jobs 4` and `config set --toml jobs 4` all have it somewhere
    /// different — and a raw scan cannot tell a positional from the value of a flag. Given
    /// `config set jobs --tag color <TAB>`, scanning words found `color`, a setting in its own
    /// right, and offered its booleans as though the user were setting it.
    fn config_key_before_cursor<'a>(&self, cx: &Ctx<'a>) -> Option<&'a SpecConfigProp> {
        // The argument the *spec* says holds a key — the one completed with `config_keys` —
        // rather than whichever positional happens to name a setting. Both guesses were wrong
        // in their own direction: scanning backwards took a variadic's own last value
        // (`set-many log_level color <TAB>` offered `color`'s booleans), and scanning forwards
        // would take an unrelated positional that happened to name one. The spec already says
        // which argument is which, so there is nothing to guess.
        cx.parsed
            .args
            .iter()
            .filter(|(arg, _)| self.completer_type(cx, arg) == Some("config_keys"))
            .filter_map(|(_, value)| match value {
                ParseValue::String(word) => Some(word.as_str()),
                ParseValue::MultiString(words) => words.last().map(String::as_str),
                ParseValue::Bool(_) | ParseValue::MultiBool(_) => None,
            })
            // The nearest one, for a command with more than one key argument — the same rule as
            // the last element of a variadic. Taking the first offered values for whichever key
            // came earliest on the line.
            //
            // No filter for "did the user type this": a key argument bound from its `default=`
            // rather than from the line would win the nearest-wins rule below, but a partial
            // parse does not produce such a binding — measured against a spec shaped exactly
            // that way, with the defaulted argument *after* the value being completed, where
            // the guard would have been the only thing standing between them. Unreachable code
            // in a completion path is worse than the case it defends against;
            // `complete_word_the_key_is_the_argument_the_spec_says_holds_one` pins the
            // behaviour so that if a partial parse ever starts filling defaults, this fails.
            //
            // Only the nearest: looking further back when it names no setting
            // offered another key's values for a line whose own key is a typo, where an unknown
            // key on its own correctly offers nothing.
            .next_back()
            .and_then(|word| cx.spec.config.props.get(word))
    }

    /// The reserved `type=` of the completer for an argument, if it has one.
    fn completer_type<'a>(&self, cx: &Ctx<'a>, arg: &SpecArg) -> Option<&'a str> {
        let name = arg.name.to_lowercase();
        cx.spec
            .complete
            .get(&name)
            .or_else(|| cx.parsed.cmd.complete.get(&name))
            .and_then(|complete| complete.type_.as_deref())
    }

    /// Completions for a positional argument, under the rule the parser enforces for
    /// `double_dash="required"`: nothing reaches such an argument until an explicit `--` has
    /// been typed, so before that the separator is the only useful candidate.
    ///
    /// Every path that completes a positional goes through here — the one at the parser's
    /// cursor, the first argument after a `restart_token`, and the default subcommand's — so
    /// the rule cannot be honoured in one and forgotten in another.
    ///
    /// The second return value says whether the argument constrains what may go there, which
    /// is what suppresses the file-path fallback.
    fn complete_positional(
        &self,
        cx: &Ctx<'_>,
        cmd: &SpecCommand,
        arg: &SpecArg,
        ctoken: &str,
        double_dash_seen: bool,
    ) -> miette::Result<(Vec<(String, String)>, bool)> {
        if arg.double_dash == SpecDoubleDashChoices::Required && !double_dash_seen {
            // No filename is valid here either, so the fallback stays off.
            let separator = ctoken.is_empty().then(|| ("--".to_string(), String::new()));
            return Ok((separator.into_iter().collect(), true));
        }
        let (found, closed) = self.complete_arg(cx, cmd, arg, ctoken)?;
        Ok((found, closed || arg.choices.is_some()))
    }

    fn complete_arg(
        &self,
        cx: &Ctx<'_>,
        cmd: &SpecCommand,
        arg: &SpecArg,
        ctoken: &str,
    ) -> miette::Result<(Vec<(String, String)>, bool)> {
        static EMPTY_COMPL: LazyLock<SpecComplete> = LazyLock::new(SpecComplete::default);

        trace!("complete_arg: {arg} {ctoken}");
        let name = arg.name.to_lowercase();
        let complete = cx
            .spec
            .complete
            .get(&name)
            .or(cmd.complete.get(&name))
            .unwrap_or(&EMPTY_COMPL);
        let type_ = complete.type_.as_ref().unwrap_or(&name);

        // A closed completer answers even when its answer is nothing: it knows the whole set
        // of candidates, so an unmatched prefix means no matches rather than "ask somebody
        // else". Returning empty-and-open here is what let a mistyped setting name complete
        // to the contents of the working directory.
        let (builtin, closed) = self.complete_builtin(cx, type_, ctoken);
        if !builtin.is_empty() || closed {
            return Ok((builtin, closed));
        }

        if let Some(choices) = &arg.choices {
            let values = choices.values();
            return Ok((
                values
                    .into_iter()
                    .map(|c| (c, String::new()))
                    .filter(|(c, _)| c.starts_with(ctoken))
                    .collect(),
                true,
            ));
        }
        if let Some(run) = &complete.run {
            let run = tera::Tera::one_off(run, cx.tera, false).into_diagnostic()?;
            trace!("run: {run}");
            let stdout = sh(&run)?;
            // trace!("stdout: {stdout}");
            let re = regex!(r"[^\\]:");
            return Ok((
                stdout
                    .lines()
                    .map(|l| {
                        if complete.descriptions {
                            match re.find(l).map(|m| l.split_at(m.end() - 1)) {
                                Some((l, d)) if d.len() <= 1 => {
                                    (l.trim().replace("\\:", ":"), String::new())
                                }
                                Some((l, d)) => (
                                    l.trim().replace("\\:", ":"),
                                    d[1..].trim().replace("\\:", ":"),
                                ),
                                None => (l.trim().replace("\\:", ":"), String::new()),
                            }
                        } else {
                            (l.trim().to_string(), String::new())
                        }
                    })
                    .filter(|(name, _)| name.starts_with(ctoken))
                    .collect(),
                // Left open, as it always was: a script that prints nothing may simply have
                // had nothing to say about this prefix.
                false,
            ));
        }

        Ok((vec![], false))
    }

    fn complete_path(
        &self,
        base: &Path,
        ctoken: &str,
        filter: impl Fn(&Path) -> bool,
    ) -> Vec<String> {
        trace!("complete_path: {ctoken}");
        let path = PathBuf::from(ctoken);
        let mut dir = path.parent().unwrap_or(&path).to_path_buf();
        if dir.is_relative() {
            dir = base.join(dir);
        }
        let mut prefix = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if path.is_dir() && ctoken.ends_with('/') {
            dir = path.to_path_buf();
            prefix = "".to_string();
        };
        std::fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|de| {
                let name = de.file_name();
                let name = name.to_string_lossy();
                !name.starts_with('.') && name.starts_with(&prefix)
            })
            .filter(|de| filter(&de.path()))
            .map(|de| {
                let p = de.path();
                let is_dir = de
                    .file_type()
                    .map(|ft| ft.is_dir())
                    .unwrap_or_else(|_| p.is_dir());
                let mut s = p
                    .strip_prefix(base)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .to_string();
                if is_dir {
                    s.push('/');
                }
                s
            })
            .sorted()
            .collect()
    }
}

/// Wrap a completion value in single quotes if any character would otherwise
/// be interpreted by the shell. The result is meant to be inserted by
/// `compadd -Q` verbatim, so the user sees consistent single-quote quoting
/// instead of zsh's default mix of backslash and single-quote styles.
/// What a completion is computed against.
///
/// These three always travel together — the template context a `run=` is rendered with, the
/// spec, and what the parser made of the words before the cursor — so they are one parameter
/// rather than three threaded through every helper.
struct Ctx<'a> {
    tera: &'a tera::Context,
    spec: &'a Spec,
    parsed: &'a ParseOutput,
}

/// A description reduced to one line.
///
/// A completion is one row in a menu, and the shells are handed one candidate per line with
/// tab-separated columns — so a description with a newline in it splits one candidate into
/// several rows of nonsense. `long_help` is where prose belongs.
fn one_line(text: Option<&str>) -> String {
    text.unwrap_or_default()
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Whether this type accepts values no list could enumerate.
///
/// A boolean has two values and `choices` names its own, so either can be offered in full.
/// A path, a number or free text cannot be, so a union containing one is never a closed set —
/// however many of its members are enumerable.
fn accepts_unenumerable_values(ty: &SpecConfigType) -> bool {
    match ty {
        SpecConfigType::Base(Base::Bool) => false,
        SpecConfigType::Option(inner) => accepts_unenumerable_values(inner),
        SpecConfigType::Union(members) => members.iter().any(accepts_unenumerable_values),
        // Anything else — a path, a number, a string, a list — takes values that cannot be
        // written down in advance.
        _ => true,
    }
}

/// Whether a boolean is one of the things this type accepts.
///
/// A union may list it anywhere, and `option<bool|string>` nests one. Recursive rather than a
/// look at the first member, because which member comes first is a spec author's formatting
/// choice and should not decide whether `true` and `false` are offered.
fn holds_a_boolean(ty: &SpecConfigType) -> bool {
    match ty {
        SpecConfigType::Base(Base::Bool) => true,
        SpecConfigType::Option(inner) => holds_a_boolean(inner),
        SpecConfigType::Union(members) => members.iter().any(holds_a_boolean),
        // A list or map *of* booleans is not itself one: what goes on the command line there
        // is a list, and offering `true` would be offering it in the wrong shape.
        _ => false,
    }
}

fn zsh_shell_quote(s: &str) -> String {
    fn safe(c: char) -> bool {
        matches!(c,
            'a'..='z' | 'A'..='Z' | '0'..='9'
            | '_' | '-' | '.' | '/' | ':' | '@' | '+' | '=' | '%' | ','
        )
    }
    if !s.is_empty() && s.chars().all(safe) {
        return s.to_string();
    }
    // Wrap in single quotes; close-open dance escapes any internal apostrophes.
    let escaped = s.replace('\'', "'\\''");
    format!("'{escaped}'")
}
