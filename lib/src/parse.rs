use heck::ToSnakeCase;
use indexmap::IndexMap;
use itertools::Itertools;
use log::trace;
use miette::bail;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use strum::EnumTryAs;

#[cfg(feature = "docs")]
use crate::docs;
use crate::error::UsageErr;
use crate::spec::arg::SpecDoubleDashChoices;
use crate::spec::unknown_flags::UnknownFlags;
use crate::{Spec, SpecArg, SpecChoices, SpecCommand, SpecFlag};

/// Merge a subcommand's flags into the currently available flags when descending
/// into that subcommand.
///
/// On descent we drop the parent's non-global flags (they are scoped to the parent)
/// but keep its global flags so they remain recognized further down. A subcommand may
/// re-declare a flag that the parent exposed as global (e.g. `-C/--cd`) but mark its own
/// copy as non-global. In that case we must NOT let the non-global re-declaration shadow
/// the inherited global flag, otherwise the next descent's `retain(global)` would drop it
/// entirely and later parsing would treat the already-consumed global token as an
/// unexpected positional/flag value.
///
/// Descending into a *mounted* subcommand (`crossing_mount`) is different: the mounted
/// command describes another program, which does not accept the mounting CLI's globals.
/// Those globals stay recognized (they may appear before the mounted command, and Phase 2
/// re-parses them), but the mounted command's own flags take precedence over them, so its
/// choices/completions are not replaced by a global's. Which flags a completion may offer
/// there is a separate question, answered by [`ParseOutput::completion_flags`].
fn merge_subcommand_flags(
    available: &mut BTreeMap<String, Arc<SpecFlag>>,
    new_flags: BTreeMap<String, Arc<SpecFlag>>,
    crossing_mount: bool,
) {
    // Keep only inherited global flags from the parent.
    available.retain(|_, f| f.global);

    if crossing_mount {
        // A mounted command owns its flags outright, including names an inherited global also
        // uses: a word after the mounted command belongs to the mounted program. Words before
        // it keep resolving to the global they were read as, via `prefix_bindings`. Aliases the
        // mounted command does not declare (e.g. a global's short) stay inherited.
        for (key, flag) in new_flags {
            available.insert(key, flag);
        }
        return;
    }

    // Cache the merged (global ∪ orphan-alias) flag per re-declared child so every alias key of
    // that flag ends up sharing one `Arc`. Keyed by the child `Arc`'s identity.
    let mut merged_cache: HashMap<usize, Arc<SpecFlag>> = HashMap::new();
    // Maps each merged flag produced below back to the inherited global it was merged from, so
    // the collision check can compare *origins*: a flag this loop already merged is not a
    // different global, even though it is a different `Arc`.
    let mut merged_origin: HashMap<usize, usize> = HashMap::new();
    // The inherited global a flag stands for: itself, or — for a merged flag — its source global.
    fn origin_of(merged_origin: &HashMap<usize, usize>, flag: &Arc<SpecFlag>) -> usize {
        let ptr = Arc::as_ptr(flag) as usize;
        *merged_origin.get(&ptr).unwrap_or(&ptr)
    }

    // Iterate the *flattened* child map directly (one entry per alias key). This preserves the
    // map's existing intra-subcommand collision resolution: when two flags in the same command
    // share an alias (e.g. `-x --alpha` then `-x --beta`), the BTreeMap already collapsed `-x`
    // to its last-declared owner, and we must not change which flag owns it.
    for (key, flag) in new_flags {
        if flag.global {
            // A child that re-declares (or adds) a global flag stays recognized everywhere.
            available.insert(key, flag);
            continue;
        }

        // A non-global re-declaration that shares a LONG name with an inherited global flag is
        // the SAME logical flag (e.g. mise's `-r --raw` re-declaring the long-only `--raw`
        // global). Keep the global flag (global precedence, so it survives the next descent's
        // `retain`), but union in any short/long aliases that exist only on the re-declaration,
        // otherwise those orphan aliases would be silently dropped. Matching on a shared long is
        // deliberate: a re-declaration sharing only a short letter with an unrelated global
        // (`-q --quiet` vs `-q --quoting`) is a genuine collision, not an alias addition, and is
        // handled by the `contains_key` skip below instead.
        let inherited_global = flag.long.iter().find_map(|l| {
            available
                .get(&format!("--{l}"))
                .filter(|f| f.global)
                .cloned()
        });
        if let Some(global_flag) = inherited_global {
            // Never clobber a *different* inherited global's alias. If this re-declaration's
            // orphan alias (e.g. `-r`) is already owned by some other global (e.g. an unrelated
            // `-r --restrict`), that is a genuine collision: keep the existing global, as global
            // precedence dictates, instead of stealing the alias for the merged flag.
            //
            // Compare origins, not `Arc`s: when the global has several aliases of its own, an
            // earlier key of this same child already replaced some of them with the merged flag,
            // which the lookups above may now resolve to. That is the same logical flag, so it
            // must not read as a collision and leave this key on the pre-merge global.
            let global_origin = origin_of(&merged_origin, &global_flag);
            if available.get(&key).is_some_and(|existing| {
                existing.global && origin_of(&merged_origin, existing) != global_origin
            }) {
                continue;
            }
            let merged = match merged_cache.get(&(Arc::as_ptr(&flag) as usize)) {
                Some(merged) => merged.clone(),
                None => {
                    let mut merged = (*global_flag).clone();
                    // `exclusive` is deliberately *not* reconciled here, in either direction.
                    // One object now answers to two alias sets that may disagree: the child
                    // owns the spellings it declared, the ancestor keeps the ones only it
                    // declared. A single bool cannot hold both, so the merged flag carries the
                    // ancestor's and validation resolves the occurrence by the spelling that
                    // was typed — the ledger it already consults to decide whether selecting
                    // the child is company.
                    for s in &flag.short {
                        if !merged.short.contains(s) {
                            merged.short.push(*s);
                        }
                    }
                    for l in &flag.long {
                        if !merged.long.contains(l) {
                            merged.long.push(l.clone());
                        }
                    }
                    let merged = Arc::new(merged);
                    merged_cache.insert(Arc::as_ptr(&flag) as usize, Arc::clone(&merged));
                    merged_origin.insert(Arc::as_ptr(&merged) as usize, global_origin);
                    // Rebind the global's *other* aliases onto the merged flag. The loop only
                    // visits keys the child declared, so an alias the child left out (the `-y` of
                    // a `-y --yes` global re-declared as just `--yes`) would otherwise keep
                    // pointing at the pre-merge flag and miss the aliases just unioned in. One
                    // logical flag must be one object under every key it answers to.
                    for existing in available.values_mut() {
                        if origin_of(&merged_origin, existing) == global_origin {
                            *existing = Arc::clone(&merged);
                        }
                    }
                    merged
                }
            };
            available.insert(key, merged);
            continue;
        }

        // Purely-local flag (shares nothing with an inherited global), or one that collides only
        // on a short with an unrelated global. Insert this alias but never shadow an inherited
        // global flag. Such non-global flags are dropped by the next descent's `retain`.
        if available.contains_key(&key) {
            continue;
        }
        available.insert(key, flag);
    }
}

/// Build the lookup keys a flag is registered under in `available_flags`:
/// `--<long>` for each long name, `-<short>` for each short char, plus the `negate` token.
fn flag_keys(flag: &SpecFlag) -> Vec<String> {
    let mut keys: Vec<String> = flag
        .long
        .iter()
        .map(|l| format!("--{l}"))
        .chain(flag.short.iter().map(|s| format!("-{s}")))
        .collect();
    if let Some(negate) = &flag.negate {
        keys.push(negate.clone());
    }
    keys
}

/// The flags a command declares, keyed by each of their aliases.
fn gather_flags(cmd: &SpecCommand) -> BTreeMap<String, Arc<SpecFlag>> {
    cmd.flags
        .iter()
        .flat_map(|f| {
            let f = Arc::new(f.clone()); // One clone per flag, then cheap Arc refs
            flag_keys(&f)
                .into_iter()
                .map(|key| (key, Arc::clone(&f)))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn unique_flags<'a>(
    flags: impl IntoIterator<Item = &'a Arc<SpecFlag>>,
) -> impl Iterator<Item = &'a Arc<SpecFlag>> {
    let mut seen = HashSet::new();
    flags
        .into_iter()
        .filter(move |flag| seen.insert(Arc::as_ptr(flag) as usize))
}

/// Every flag a command accepts, resolved the way parsing an invocation of it
/// resolves them.
///
/// `chain` runs from the root command (`spec.cmd`) down to the command in
/// question; an empty chain yields no flags.
///
/// This is not "the command's flags plus its ancestors' globals". A subcommand
/// that re-declares a global's long name is describing the *same* flag rather
/// than a new one, so the global's help, argument and effect survive and only
/// the re-declaration's extra aliases are added — see
/// [`merge_subcommand_flags`]. Anything that reports a command's flags without
/// going through this will disagree with what the parser actually accepts.
pub fn available_flags(chain: &[&SpecCommand]) -> Vec<Arc<SpecFlag>> {
    let Some((root, rest)) = chain.split_first() else {
        return vec![];
    };
    let mut available = gather_flags(root);
    for cmd in rest {
        merge_subcommand_flags(&mut available, gather_flags(cmd), false);
    }

    // Deduplicating by `Arc` identity is not enough. When a child re-declares a
    // global that has both a short and a long, the merged flag is written under
    // the long key while the short key keeps pointing at the pre-merge `Arc` —
    // two objects for one logical flag. That is harmless for parsing, which
    // looks flags up by key, but a caller listing flags would see it twice.
    //
    // Names break the tie because a long key always sorts before a short one
    // (`--x` < `-y` at the second byte), so the merged declaration is the one
    // reached first. Two genuinely distinct flags sharing a name is a spec bug
    // that `usage lint` reports as a duplicate flag.
    let mut seen_names = HashSet::new();
    unique_flags(available.values())
        .filter(|f| seen_names.insert(f.name.clone()))
        .cloned()
        .collect()
}

/// Extract the flag key from a flag word for lookup in available_flags map
/// Handles both long flags (--flag, --flag=value) and short flags (-f)
fn get_flag_key(word: &str) -> &str {
    if word.starts_with("--") {
        // Long flag: strip =value if present
        word.split_once('=').map(|(k, _)| k).unwrap_or(word)
    } else if let Some((end, _)) = word.char_indices().nth(2) {
        // Short flag: the dash and one letter, which is one character and not
        // necessarily one byte.
        &word[0..end]
    } else {
        word
    }
}

pub struct ParseOutput {
    pub cmd: SpecCommand,
    pub cmds: Vec<SpecCommand>,
    pub args: IndexMap<Arc<SpecArg>, ParseValue>,
    pub flags: IndexMap<Arc<SpecFlag>, ParseValue>,
    /// Every flag the parser recognizes at this point, keyed by each of its aliases
    /// (`--long`, `-s`, negations).
    ///
    /// This includes flags that only remain recognized because they may appear *before* a
    /// mounted command — see [`ParseOutput::completion_flags`] for the set a completion
    /// should offer.
    pub available_flags: BTreeMap<String, Arc<SpecFlag>>,
    pub flag_awaiting_value: Vec<Arc<SpecFlag>>,
    pub errors: Vec<UsageErr>,
    /// The positional argument the next word would have filled, i.e. where the parser's
    /// cursor stopped. `None` once every argument is satisfied.
    ///
    /// Completions need exactly this: the parser already accounts for `var_max`, for
    /// `restart_token` rewinds, and for the jump an explicit `--` performs onto a
    /// `double_dash="required"` argument, so re-deriving it from `args` would disagree.
    pub next_arg: Option<Arc<SpecArg>>,
    /// Whether an explicit `--` was consumed *as a separator*.
    ///
    /// A `--` that `double_dash="preserve"` keeps as a value does not count: it is a value
    /// of the variadic argument collecting it, not a separator, so it does not unlock a
    /// `double_dash="required"` argument.
    pub double_dash_seen: bool,
}

impl ParseOutput {
    /// The flags a completion should offer for the parsed command.
    ///
    /// Usually every recognized flag, i.e. [`ParseOutput::available_flags`]. Once a mounted
    /// command has been reached, though, the commands above it belong to the mounting CLI and
    /// their flags are not accepted there — mise, for example, forwards everything after a task
    /// name to the task itself — so only the flags declared from the mount boundary down are
    /// offered. Those globals stay in `available_flags` because they may legitimately appear
    /// *before* the mounted command.
    pub fn completion_flags(&self) -> BTreeMap<String, Arc<SpecFlag>> {
        let Some(boundary) = self.cmds.iter().position(|cmd| cmd.mounted) else {
            return self.available_flags.clone();
        };
        // A mount can also merge flags from its spec's root into the command it is mounted on
        // (`SpecCommand::flags_from_mount`). Those describe the mounted program too, so the
        // replay starts one level up to inherit its globals.
        let start = match boundary.checked_sub(1) {
            Some(prev) if self.cmds[prev].flags_from_mount => prev,
            _ => boundary,
        };
        // Re-run the descent from there, which starts with no inherited flags. Below the
        // boundary the mounted program's commands are ordinary commands, so the descents use
        // the same merge as the real parse.
        let mut offered = gather_flags(&self.cmds[start]);
        for cmd in &self.cmds[start + 1..] {
            merge_subcommand_flags(&mut offered, gather_flags(cmd), false);
        }
        offered
    }
}

#[derive(Debug, EnumTryAs, Clone)]
pub enum ParseValue {
    Bool(bool),
    String(String),
    MultiBool(Vec<bool>),
    MultiString(Vec<String>),
}

/// Builder for parsing command-line arguments with custom options.
///
/// Use this when you need to customize parsing behavior, such as providing
/// a custom environment variable map instead of using the process environment.
///
/// # Example
/// ```
/// use std::collections::HashMap;
/// use usage::Spec;
/// use usage::parse::Parser;
///
/// let spec: Spec = r#"flag "--name <name>" env="NAME""#.parse().unwrap();
/// let env: HashMap<String, String> = [("NAME".into(), "john".into())].into();
///
/// let result = Parser::new(&spec)
///     .with_env(env)
///     .parse(&["cmd".into()])
///     .unwrap();
/// ```
#[non_exhaustive]
pub struct Parser<'a> {
    spec: &'a Spec,
    env: Option<HashMap<String, String>>,
}

impl<'a> Parser<'a> {
    /// Create a new parser for the given spec.
    pub fn new(spec: &'a Spec) -> Self {
        Self { spec, env: None }
    }

    /// Use a custom environment variable map instead of the process environment.
    ///
    /// This is useful when parsing for tasks in a monorepo where the env vars
    /// come from a child config file rather than the current process environment.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = Some(env);
        self
    }

    /// Parse the input arguments.
    ///
    /// Returns the parsed arguments and flags, with defaults and env vars applied.
    pub fn parse(self, input: &[String]) -> Result<ParseOutput, miette::Error> {
        let custom_env = self.env.as_ref();
        let (mut out, overridden_flags) = parse_partial_with_env(
            self.spec,
            input,
            custom_env,
            MountTiming::WhenAWordIsUnknown,
        )?;
        trace!("{out:?}");

        // A flag still waiting for a value never got one, so the command line ended
        // mid-flag. `parse_partial` leaves this for completions to look at — a
        // half-typed `--jobs ` is exactly what a completion is asked about — but a
        // full parse has nothing left to wait for, and dropping the flag silently
        // made a forgotten value look like a working command.
        if let Some(flag) = out.flag_awaiting_value.first() {
            let token = flag
                .long
                .first()
                .map(|l| format!("--{l}"))
                .or_else(|| flag.short.first().map(|s| format!("-{s}")))
                .unwrap_or_else(|| flag.name.clone());
            let rendered = input.join(" ");
            let span = rendered
                .rfind(&token)
                .map(|at| (at, token.len()))
                .unwrap_or((0, 0));
            return Err(UsageErr::InvalidFlag {
                token,
                reason: "requires an argument".to_string(),
                span: span.into(),
                input: rendered,
            }
            .into());
        }

        let get_env = |key: &str| -> Option<String> {
            if let Some(env_map) = custom_env {
                env_map.get(key).cloned()
            } else {
                std::env::var(key).ok()
            }
        };

        // Apply env vars and defaults for args
        //
        // Not `skip(out.args.len())`: an explicit `--` can jump the parser's cursor past an arg
        // that stayed empty, leaving a gap that makes the fill count a wrong starting offset.
        for arg in out.cmd.args.iter() {
            if out.args.contains_key(arg) {
                continue;
            }
            if let Some(env_var) = arg.env.as_ref() {
                if let Some(env_value) = get_env(env_var) {
                    validate_choice_value(
                        ChoiceTarget::arg(arg),
                        &env_value,
                        arg.choices.as_ref(),
                        custom_env,
                    )?;
                    out.args
                        .insert(Arc::new(arg.clone()), ParseValue::String(env_value));
                    continue;
                }
            }
            if !arg.default.is_empty() {
                // Consider var when deciding the type of default return value
                if arg.var {
                    validate_choice_values(
                        ChoiceTarget::arg(arg),
                        &arg.default,
                        arg.choices.as_ref(),
                        custom_env,
                    )?;
                    // For var=true, always return a vec (MultiString)
                    out.args.insert(
                        Arc::new(arg.clone()),
                        ParseValue::MultiString(arg.default.clone()),
                    );
                } else {
                    validate_choice_value(
                        ChoiceTarget::arg(arg),
                        &arg.default[0],
                        arg.choices.as_ref(),
                        custom_env,
                    )?;
                    // For var=false, return the first default value as String
                    out.args.insert(
                        Arc::new(arg.clone()),
                        ParseValue::String(arg.default[0].clone()),
                    );
                }
            }
        }

        // Apply env vars and defaults for flags
        for flag in out.available_flags.values() {
            if out.flags.contains_key(flag) || overridden_flags.contains(&flag.name) {
                continue;
            }
            if let Some(env_var) = flag.env.as_ref() {
                if let Some(env_value) = get_env(env_var) {
                    if let Some(arg) = flag.arg.as_ref() {
                        validate_choice_value(
                            ChoiceTarget::option(flag),
                            &env_value,
                            arg.choices.as_ref(),
                            custom_env,
                        )?;
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::String(env_value));
                    } else {
                        // For boolean flags, check if env value is truthy
                        let is_true = matches!(env_value.as_str(), "1" | "true" | "True" | "TRUE");
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::Bool(is_true));
                    }
                    continue;
                }
            }
            // Apply flag default
            if !flag.default.is_empty() {
                // Consider var when deciding the type of default return value
                if flag.var {
                    // For var=true, always return a vec (MultiString for flags with args, MultiBool for boolean flags)
                    if let Some(arg) = flag.arg.as_ref() {
                        validate_choice_values(
                            ChoiceTarget::option(flag),
                            &flag.default,
                            arg.choices.as_ref(),
                            custom_env,
                        )?;
                        out.flags.insert(
                            Arc::clone(flag),
                            ParseValue::MultiString(flag.default.clone()),
                        );
                    } else {
                        // For boolean flags with var=true, convert default strings to bools
                        let bools: Vec<bool> = flag
                            .default
                            .iter()
                            .map(|s| matches!(s.as_str(), "1" | "true" | "True" | "TRUE"))
                            .collect();
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::MultiBool(bools));
                    }
                } else {
                    // For var=false, return the first default value
                    if let Some(arg) = flag.arg.as_ref() {
                        validate_choice_value(
                            ChoiceTarget::option(flag),
                            &flag.default[0],
                            arg.choices.as_ref(),
                            custom_env,
                        )?;
                        out.flags.insert(
                            Arc::clone(flag),
                            ParseValue::String(flag.default[0].clone()),
                        );
                    } else {
                        // For boolean flags, convert default string to bool
                        let is_true =
                            matches!(flag.default[0].as_str(), "1" | "true" | "True" | "TRUE");
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::Bool(is_true));
                    }
                }
            }
            // Also check nested arg defaults (for flags like --foo <arg> where the arg has a default)
            if let Some(arg) = flag.arg.as_ref() {
                if !out.flags.contains_key(flag) && !arg.default.is_empty() {
                    if flag.var {
                        validate_choice_values(
                            ChoiceTarget::option(flag),
                            &arg.default,
                            arg.choices.as_ref(),
                            custom_env,
                        )?;
                        out.flags.insert(
                            Arc::clone(flag),
                            ParseValue::MultiString(arg.default.clone()),
                        );
                    } else {
                        validate_choice_value(
                            ChoiceTarget::option(flag),
                            &arg.default[0],
                            arg.choices.as_ref(),
                            custom_env,
                        )?;
                        out.flags
                            .insert(Arc::clone(flag), ParseValue::String(arg.default[0].clone()));
                    }
                }
            }
        }
        if let Some(err) = out.errors.iter().find(|e| matches!(e, UsageErr::Help(_))) {
            bail!("{err}");
        }
        if !out.errors.is_empty() {
            bail!("{}", out.errors.iter().map(|e| e.to_string()).join("\n"));
        }
        Ok(out)
    }
}

/// Parse command-line arguments according to a spec.
///
/// Returns the parsed arguments and flags, with defaults and env vars applied.
/// Uses `std::env::var` for environment variable lookups.
///
/// For custom environment variable handling, use [`Parser`] instead.
#[must_use = "parsing result should be used"]
pub fn parse(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    Parser::new(spec).parse(input)
}

/// Parse command-line arguments without applying defaults.
///
/// Use this for help text generation or when you need the raw parsed values.
#[must_use = "parsing result should be used"]
pub fn parse_partial(spec: &Spec, input: &[String]) -> Result<ParseOutput, miette::Error> {
    parse_partial_with_env(spec, input, None, MountTiming::Eager).map(|(out, _)| out)
}

/// Internal version of parse_partial that accepts an optional custom env map.
/// When a command's own `mount` runs, for the root — which nothing descends into.
///
/// A completion has to know every command before it can offer one, even with
/// nothing typed yet, so it resolves up front. An execution knows the word it was
/// given, so it only pays for discovery when that word matches nothing declared —
/// and a CLI that declares its commands and mounts a few more does not spawn a
/// process on every invocation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MountTiming {
    Eager,
    WhenAWordIsUnknown,
}

fn parse_partial_with_env(
    spec: &Spec,
    input: &[String],
    custom_env: Option<&HashMap<String, String>>,
    mount_timing: MountTiming,
) -> Result<(ParseOutput, HashSet<String>), miette::Error> {
    trace!("parse_partial: {input:?}");
    let mut input = input.iter().cloned().collect::<VecDeque<_>>();
    input.pop_front();

    let mut out = ParseOutput {
        cmd: spec.cmd.clone(),
        cmds: vec![spec.cmd.clone()],
        args: IndexMap::new(),
        flags: IndexMap::new(),
        available_flags: gather_flags(&spec.cmd),
        flag_awaiting_value: vec![],
        errors: vec![],
        next_arg: None,
        double_dash_seen: false,
    };
    // Keep this internal so adding relationship support remains semver-compatible. The full
    // parser uses it to prevent defaults and environment values from restoring overridden flags.
    let mut overridden_flags = HashSet::new();
    // Which spelling supplied each parsed flag. A child may re-declare one long form of an
    // inherited global while the merge keeps the ancestor's other aliases on the same `Arc`.
    // The declaration object alone then cannot answer whether `--clean` belonged to the child
    // or an inherited `-c` belonged to the ancestor.
    let mut parsed_flag_spellings: HashMap<usize, HashSet<String>> = HashMap::new();

    // Phase 1: Scan for subcommands and collect global flags
    //
    // This phase identifies subcommands early because they may have mount points
    // that need to be executed with the global flags that appeared before them.
    //
    // Example: "usage --verbose run task"
    //   -> finds "run" subcommand, passes ["--verbose"] to its mount command
    //   -> then finds "task" as a subcommand of "run" (if it exists)
    //
    // We only collect global flags for mounts because:
    // - Non-global flags are specific to the current command, not subcommands
    // - Global flags affect all commands and should be passed to mount points
    let mut prefix_flags: Vec<(Arc<SpecFlag>, Vec<String>)> = vec![];
    // Which flag each word skipped here belongs to, aligned with the leading words left in
    // `input`: `Some(flag)` for a flag word, `None` for its value (or anything unresolved).
    //
    // The words stay in `input` for Phase 2 to re-parse — that is how they reach `out.flags`
    // and `as_env()` — but by then the recognized flags have changed, because each descent
    // drops the parent's non-global flags and a mounted command may declare the same name as
    // a global seen here. Recording the owner keeps a word bound to the flag it was read as.
    let mut prefix_bindings: VecDeque<Option<Arc<SpecFlag>>> = VecDeque::new();
    let mut idx = 0;
    // Track whether we've already applied the default_subcommand to prevent
    // multiple switches (e.g., if default is "run" and there's a task named "run")
    let mut used_default_subcommand = false;
    // Whether the command in scope has had its own mounts run. A mount on the root
    // is the case that needs this: a subcommand's mounts are run when the parser
    // descends into it, but nothing descends into the root.
    let mut mounts_resolved = false;
    // A completion needs the whole command list before it can offer anything, and
    // `mycli <tab>` has no word to trigger discovery with — so waiting for one would
    // mean a root mount never contributed to the very thing it exists for.
    //
    // The default-subcommand gate applies here too, and has to: offering a discovered
    // command that a real parse would hand to the default instead would be worse than
    // not offering it. A root mount under a `default_subcommand` that does not say
    // `overrides_default` therefore contributes nothing anywhere, which is what
    // "the default outranks discovery" means.
    let default_outranks_mounts =
        spec.default_subcommand.is_some() && !out.cmd.mounts.iter().any(|m| m.overrides_default);
    if mount_timing == MountTiming::Eager && !default_outranks_mounts && !out.cmd.mounts.is_empty()
    {
        mounts_resolved = true;
        let mut mounted = out.cmd.clone();
        mounted.mount(&[])?;
        merge_subcommand_flags(&mut out.available_flags, gather_flags(&mounted), false);
        if let Some(last) = out.cmds.last_mut() {
            *last = mounted.clone();
        }
        out.cmd = mounted;
    }

    while idx < input.len() {
        // Only for a word that could name a command, and only when it matches
        // nothing already declared. A CLI that declares its commands and mounts more
        // does not spawn a process for every invocation, and a flag — `--help`, or
        // anything unrecognized — never triggers discovery at all, which it would
        // otherwise do simply by not being a subcommand.
        // A declared `default_subcommand` already says what an unmatched word means,
        // and it costs nothing — so discovery waits behind it unless a mount asks to
        // outrank it. Without this, a task runner would spawn its discovery process
        // once per task invocation.
        let default_catches_it = spec.default_subcommand.is_some()
            && !out.cmd.mounts.iter().any(|m| m.overrides_default);
        if !mounts_resolved
            && !out.cmd.mounts.is_empty()
            && !default_catches_it
            && !input[idx].starts_with('-')
            && out.cmd.find_subcommand(&input[idx]).is_none()
        {
            mounts_resolved = true;
            let mut mounted = out.cmd.clone();
            mounted.mount(&mount_prefix_words(&prefix_flags))?;
            merge_subcommand_flags(&mut out.available_flags, gather_flags(&mounted), false);
            if let Some(last) = out.cmds.last_mut() {
                *last = mounted.clone();
            }
            out.cmd = mounted;
        }
        if let Some(subcommand) = out.cmd.find_subcommand(&input[idx]) {
            let mut subcommand = subcommand.clone();
            // Pass prefix words (global flags before this subcommand) to mount
            subcommand.mount(&mount_prefix_words(&prefix_flags))?;
            // Only the *boundary* is a mount crossing: below it, the mounted program's own
            // commands are ordinary commands relative to each other.
            let crossing_mount = subcommand.mounted && !out.cmd.mounted;
            merge_subcommand_flags(
                &mut out.available_flags,
                gather_flags(&subcommand),
                crossing_mount,
            );
            // Remove subcommand from input
            input.remove(idx);
            out.cmds.push(subcommand.clone());
            out.cmd = subcommand.clone();
            // A descent already ran the new command's mounts, above.
            mounts_resolved = true;
            prefix_flags.clear();
            // Continue from current position (don't reset to 0)
            // After remove(), idx now points to the next element
        } else if input[idx].starts_with('-') {
            // Check if this is a known flag
            let word = input[idx].clone();
            let flag_key = get_flag_key(&word);

            // A short token keys on its first letter, so `-az` would be recorded as
            // `-a` and its tail left over. Check the whole token here, where it is
            // first read: a token containing an unrecognized letter is not a bundle,
            // and recording it as one is what let `-a` be applied from a token that
            // never named it.
            let is_bundle =
                word.starts_with("--") || short_bundle_is_known(&out.available_flags, &word);
            if let Some(f) = out
                .available_flags
                .get(flag_key)
                .cloned()
                .filter(|_| is_bundle)
            {
                // Skip the flag and keep scanning. Both global and non-global flags may precede
                // a subcommand (`mycli --verbose run task`, `mycli run --force task`), and
                // stopping at one would hide the subcommand — and any mount on it — from the
                // parse, leaving the subcommand name to be mis-read as a positional argument.
                //
                // Only globals are forwarded to mounts: a non-global flag belongs to the
                // command that declared it, not to what is mounted below it.
                prefix_bindings.push_back(Some(Arc::clone(&f)));
                let mut forwarded = f.global.then(|| vec![word.clone()]);
                idx += 1;

                // Only consume next word if flag takes an argument AND value isn't embedded
                // Example: "--dir foo" consumes "foo", but "--dir=foo" or "--verbose" do not
                if f.arg.is_some()
                    && !word.contains('=')
                    && idx < input.len()
                    && !input[idx].starts_with('-')
                {
                    if let Some(words) = forwarded.as_mut() {
                        words.push(input[idx].clone());
                    }
                    prefix_bindings.push_back(None);
                    idx += 1;
                }
                if let Some(words) = forwarded {
                    apply_prefix_flag_overrides(&mut prefix_flags, Arc::clone(&f));
                    prefix_flags.push((f, words));
                }
            } else {
                // Unknown flag - stop looking for subcommands
                // Let the main parsing phase handle the error
                break;
            }
        } else {
            // Found a word that's not a flag or subcommand
            // Check if we should use the default_subcommand (only once, and only at the
            // root, which is the only place a spec can declare one — `out.cmds` holds just
            // the root until something descends). Without that second condition the one
            // declared name is looked up wherever the parser happens to be standing, so an
            // unrelated command acquires a default because a name matched one level down:
            // with `default_subcommand "ls"` at the top, `ex config zzz` descended into
            // `config ls`.
            if !used_default_subcommand && out.cmds.len() == 1 {
                if let Some(default_name) = &spec.default_subcommand {
                    if let Some(subcommand) = out.cmd.find_subcommand(default_name) {
                        let mut subcommand = subcommand.clone();
                        // Pass prefix words (global flags before this) to mount
                        subcommand.mount(&mount_prefix_words(&prefix_flags))?;
                        let crossing_mount = subcommand.mounted && !out.cmd.mounted;
                        merge_subcommand_flags(
                            &mut out.available_flags,
                            gather_flags(&subcommand),
                            crossing_mount,
                        );
                        out.cmds.push(subcommand.clone());
                        out.cmd = subcommand.clone();
                        prefix_flags.clear();
                        // This descent ran the new command's mounts, so lazy
                        // discovery must not run them a second time.
                        mounts_resolved = true;
                        used_default_subcommand = true;
                        // Continue the loop to check if this word is a subcommand of the
                        // default subcommand (e.g., a task name added via mount).
                        // If it's not a subcommand, the next iteration will break and
                        // Phase 2 will handle it as a positional arg.
                        continue;
                    }
                }
            }
            // This could be a positional argument, so stop subcommand search
            break;
        }
    }

    // Phase 2: Main argument and flag parsing
    //
    // Now that we've identified all subcommands and executed their mounts,
    // we can parse the remaining arguments, flags, and their values.

    // The cursor into `out.cmd.args`, kept as an index rather than a reference because an
    // explicit `--` may jump it *past* arguments that stay empty (see the `w == "--"` arm).
    // With such a gap `out.args.len()` no longer equals the cursor, so anything asking "is this
    // argument filled?" has to consult `out.args` by key instead of counting.
    let mut next_arg_idx: usize = 0;
    let mut enable_flags = true;
    let mut grouped_flag = false;
    // Whether an explicit `--` has been consumed *as a separator* (as opposed to being kept as a
    // value by `double_dash="preserve"`). Args declared `double_dash="required"` only accept
    // words that come after it — see `report_double_dash_violation`.
    let mut seen_double_dash = false;
    // Args already reported as having been offered a word before the `--` they require, so a
    // variadic one does not report the same violation for every word it is offered.
    let mut double_dash_violations: HashSet<String> = HashSet::new();

    while !input.is_empty() {
        let mut w = input.pop_front().unwrap();
        // The flag this word was read as in Phase 1, if it skipped it (see `prefix_bindings`).
        // Words pushed back below get a `None` so the two queues stay aligned.
        let binding = prefix_bindings.pop_front().flatten();

        // Check for restart_token - resets argument parsing for multiple command invocations
        // e.g., `mise run lint ::: test ::: check` with restart_token=":::"
        if let Some(ref restart_token) = out.cmd.restart_token {
            if w == *restart_token {
                // Reset argument parsing state for a fresh command invocation, keeping the
                // flags. `double_dash_violations` is deliberately *not* cleared: `out.errors`
                // is not cleared here either, so clearing it would let one arg report the same
                // violation once per invocation.
                out.args.clear();
                next_arg_idx = 0;
                out.flag_awaiting_value.clear(); // Clear any pending flag values
                enable_flags = true; // Reset -- separator effect
                seen_double_dash = false; // The next invocation needs its own `--`
                continue;
            }
        }

        // A flag declared `allow_hyphen_values` takes the next token whatever it looks
        // like, and that has to be asked before the separator arm below rather than
        // after it. Asked after, a `--` was consumed as a separator while the flag
        // stayed hungry, and the flag then ate the word past it: `ex -a -- -x` bound
        // `-x` and the separator was simply gone. Asked here, the flag takes the `--`
        // itself, which is what clap does with the same declaration — and no flag can
        // still be waiting once the separator has done its job, so the starvation rule
        // below has no path around it.
        if enable_flags
            && w.starts_with('-')
            && out
                .flag_awaiting_value
                .last()
                .is_some_and(|flag| flag.allow_hyphen_values())
        {
            // A variadic argument collects here too: which token supplied its first
            // value says nothing about how many it takes.
            let should_return = bind_pending_flag_value(
                spec,
                &out.cmd,
                &mut out.errors,
                &mut out.flags,
                &mut out.flag_awaiting_value,
                &mut w,
                &mut input,
                &mut prefix_bindings,
                custom_env,
            )?;
            if should_return {
                record_cursor(&mut out, next_arg_idx, seen_double_dash);
                return Ok((out, overridden_flags));
            }
            continue;
        }

        // Only while flags are still being read. Once a `--` has done its job, a
        // second one is an ordinary value: every parser worth comparing against
        // keeps it (POSIX getopt, argparse, clap, commander, yargs), and jdx/usage#229
        // was a user reporting the old behavior as the bug it is.
        if w == "--" && enable_flags {
            enable_flags = false;

            // Only preserve the double dash token if we're collecting values for a variadic arg
            // in double_dash == `preserve` mode
            let should_preserve = out
                .cmd
                .args
                .get(next_arg_idx)
                .map(|arg| arg.var && arg.double_dash == SpecDoubleDashChoices::Preserve)
                .unwrap_or(false);

            if should_preserve {
                // Fall through to arg parsing. This `--` is a *value*, not a separator, so it
                // neither counts as one nor unlocks a `double_dash="required"` arg.
            } else {
                seen_double_dash = true;

                // Everything after an explicit `--` belongs to the arg that requires one, so
                // jump the cursor there — past any earlier arg, including a greedy variadic
                // that would otherwise swallow the rest. This mirrors clap's `Arg::last(true)`,
                // which is what `double_dash="required"` is generated from. Specs without such
                // an arg find nothing and keep the cursor where it was.
                let target = out.cmd.args.iter().position(|arg| {
                    arg.double_dash == SpecDoubleDashChoices::Required
                        && !out.args.contains_key(arg)
                });
                if let Some(target) = target {
                    // Forward only. An unfilled required arg declared *before* the cursor is
                    // left where it is rather than rewound to — words already assigned to
                    // later args would have to be taken back for that to mean anything, and
                    // the arg keeps its `MissingArg`. `double_dash="required"` mirrors clap's
                    // `Arg::last(true)`, which is the final positional, so a spec that puts
                    // one ahead of others is already outside what this models.
                    if target > next_arg_idx {
                        next_arg_idx = target;
                    }
                }
                continue;
            }
        }

        // long flags
        if enable_flags && w.starts_with("--") {
            grouped_flag = false;
            // `Some` only when an `=` was actually written, so `--jobs=` can supply
            // an empty value while `--jobs` supplies none. Collapsing the two lost
            // the flag entirely.
            let split = w.split_once('=');
            let word = split.map(|(word, _)| word).unwrap_or(&w);
            if let Some(f) = binding.as_ref().or_else(|| out.available_flags.get(word)) {
                parsed_flag_spellings
                    .entry(Arc::as_ptr(f) as usize)
                    .or_default()
                    .insert(word.to_string());
                apply_flag_overrides(
                    f,
                    &out.available_flags,
                    &mut out.flags,
                    &mut out.flag_awaiting_value,
                    &mut overridden_flags,
                );
                // An attached value only means something to a flag that takes one:
                // `--jobs=` is an empty string, while `--force=yes` has nothing to
                // give a flag that holds no value. Handing that leftover to the
                // positionals would re-split one token into two, so `ex --force=yes`
                // would fill an argument the caller never typed a word for.
                if f.arg.is_some() {
                    let f = Arc::clone(f);
                    out.flag_awaiting_value.push(Arc::clone(&f));
                    // The `=` has already settled that this text is the value, so it
                    // binds here rather than going back on the queue to be read as a
                    // token again — where `--jobs=--force` looked like a flag of its
                    // own and bound `force`, leaving `jobs` unset.
                    if let Some((_, val)) = split {
                        // The `=` settles where the *first* value came from and nothing
                        // more, so a variadic argument goes on collecting from the words
                        // after it exactly as the detached form does.
                        let mut val = val.to_string();
                        let should_return = bind_pending_flag_value(
                            spec,
                            &out.cmd,
                            &mut out.errors,
                            &mut out.flags,
                            &mut out.flag_awaiting_value,
                            &mut val,
                            &mut input,
                            &mut prefix_bindings,
                            custom_env,
                        )?;
                        if should_return {
                            record_cursor(&mut out, next_arg_idx, seen_double_dash);
                            return Ok((out, overridden_flags));
                        }
                    }
                } else if f.count {
                    let arr = out
                        .flags
                        .entry(Arc::clone(f))
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    let negate = f.negate.clone().unwrap_or_default();
                    // Which form was typed is a question about the name, so it is
                    // asked of `word` rather than the whole token: the attached value
                    // is dropped just above, and comparing `--no-color=yes` against
                    // `--no-color` would take the negation down with it.
                    out.flags
                        .insert(Arc::clone(f), ParseValue::Bool(word != negate));
                }
                continue;
            }
            if is_help_arg(spec, &w) {
                out.errors
                    .push(render_help_err(spec, &out.cmd, w.len() > 2));
                record_cursor(&mut out, next_arg_idx, seen_double_dash);
                return Ok((out, overridden_flags));
            }
            reject_unknown_flag_if_asked(spec, &out.cmds, &w)?;
        }

        // short flags
        //
        // A fresh token is checked whole before any of it is applied: `-az` with only
        // `-a` declared is not a bundle at all, so it must not set `a` on the way to
        // discovering that `z` names nothing. A grouped continuation is exempt — its
        // token was already checked when it arrived.
        if enable_flags
            && !grouped_flag
            // A word phase 1 already resolved to a flag needs no re-checking, and
            // the flags in scope have changed since, so re-checking would be wrong.
            && binding.is_none()
            && w.starts_with('-')
            && w.len() > 1
            && is_flag_like(&w)
            && !short_bundle_is_known(&out.available_flags, &w)
        {
            // Refused if this command asked for that; otherwise it carries on below
            // as one word, with none of its letters applied.
            reject_unknown_flag_if_asked(spec, &out.cmds, &w)?;
        } else if enable_flags && w.starts_with('-') && w.len() > 1 {
            let short = w.chars().nth(1).unwrap();
            if let Some(f) = binding
                .as_ref()
                .or_else(|| out.available_flags.get(&format!("-{short}")))
            {
                parsed_flag_spellings
                    .entry(Arc::as_ptr(f) as usize)
                    .or_default()
                    .insert(format!("-{short}"));
                apply_flag_overrides(
                    f,
                    &out.available_flags,
                    &mut out.flags,
                    &mut out.flag_awaiting_value,
                    &mut overridden_flags,
                );
                let rest = &w[1 + short.len_utf8()..];
                if !rest.is_empty() {
                    input.push_front(format!("-{rest}"));
                    prefix_bindings.push_front(None);
                    grouped_flag = true;
                }
                if f.arg.is_some() {
                    out.flag_awaiting_value.push(Arc::clone(f));
                } else if f.count {
                    let arr = out
                        .flags
                        .entry(Arc::clone(f))
                        .or_insert_with(|| ParseValue::MultiBool(vec![]))
                        .try_as_multi_bool_mut()
                        .unwrap();
                    arr.push(true);
                } else {
                    let negate = f.negate.clone().unwrap_or_default();
                    out.flags
                        .insert(Arc::clone(f), ParseValue::Bool(w != negate));
                }
                continue;
            }
            if is_help_arg(spec, &w) {
                out.errors
                    .push(render_help_err(spec, &out.cmd, w.len() > 2));
                record_cursor(&mut out, next_arg_idx, seen_double_dash);
                return Ok((out, overridden_flags));
            }
            reject_unknown_flag_if_asked(spec, &out.cmds, &w)?;
            if grouped_flag {
                grouped_flag = false;
                w.remove(0);
                // What is left is a short flag's attached value, and one `=` between
                // the letter and the value is a separator: `-j=8` means 8. Only one,
                // so `-j==8` still means `=8`.
                if !out.flag_awaiting_value.is_empty() && w.starts_with('=') {
                    w.remove(0);
                }
            }
        }

        // Only while flags are still being read. A flag still waiting when the separator
        // was consumed is starved: its value would have to come from after the `--`,
        // where every token is data. Draining there gave `ex --jobs -- x` the word after
        // the separator, so the command line quietly meant `ex --jobs=x` and the `--`
        // was gone. Left waiting, it is reported as the missing value it is.
        if enable_flags && !out.flag_awaiting_value.is_empty() {
            // Held before the drain pops it: a flag whose argument is variadic keeps
            // taking values after this first one.
            let should_return = bind_pending_flag_value(
                spec,
                &out.cmd,
                &mut out.errors,
                &mut out.flags,
                &mut out.flag_awaiting_value,
                &mut w,
                &mut input,
                &mut prefix_bindings,
                custom_env,
            )?;
            if should_return {
                record_cursor(&mut out, next_arg_idx, seen_double_dash);
                return Ok((out, overridden_flags));
            }
            continue;
        }

        if let Some(arg) = out.cmd.args.get(next_arg_idx) {
            // Before anything else: an arg that requires `--` accepts nothing until one has been
            // seen. Checking ahead of `validate_choices` keeps a discarded word from also being
            // reported as an invalid choice, and from reaching that function's help escape.
            if arg.double_dash == SpecDoubleDashChoices::Required && !seen_double_dash {
                report_double_dash_violation(arg, &mut out.errors, &mut double_dash_violations);
                // Drop the word without filling the arg or advancing the cursor: every later
                // word hits the same arg and is rejected the same way, so the parse still ends
                // in an error rather than in `unexpected word`.
                continue;
            }
            if validate_choices(
                spec,
                &out.cmd,
                &mut out.errors,
                ChoiceTarget::arg(arg),
                &w,
                arg.choices.as_ref(),
                custom_env,
            )? {
                record_cursor(&mut out, next_arg_idx, seen_double_dash);
                return Ok((out, overridden_flags));
            }
            // `double_dash="automatic"` means the first value this arg takes is the last
            // token read as anything but data: a wrapper declaring it can forward flags
            // without its caller typing a `--`. Set before the value is stored, so the
            // rest of the command line is already past flag parsing.
            if arg.double_dash == SpecDoubleDashChoices::Automatic {
                enable_flags = false;
            }
            if arg.var {
                let arr = out
                    .args
                    .entry(Arc::new(arg.clone()))
                    .or_insert_with(|| ParseValue::MultiString(vec![]))
                    .try_as_multi_string_mut()
                    .unwrap();
                arr.push(w);
                if arr.len() >= arg.var_max.unwrap_or(usize::MAX) {
                    next_arg_idx += 1;
                }
            } else {
                out.args
                    .insert(Arc::new(arg.clone()), ParseValue::String(w));
                next_arg_idx += 1;
            }
            continue;
        }
        if is_help_arg(spec, &w) {
            out.errors
                .push(render_help_err(spec, &out.cmd, w.len() > 2));
            record_cursor(&mut out, next_arg_idx, seen_double_dash);
            return Ok((out, overridden_flags));
        }
        bail!("unexpected word: {w}");
    }

    record_cursor(&mut out, next_arg_idx, seen_double_dash);

    // `out.flags` is keyed by `SpecFlag`, whose equality is intentionally name-only. Two
    // declarations with the same canonical name therefore share one public value entry even
    // when both were typed. The spelling ledger is keyed by declaration identity and retains
    // both, which is what exclusivity needs.
    let flag_was_parsed =
        |flag: &Arc<SpecFlag>| parsed_flag_spellings.contains_key(&(Arc::as_ptr(flag) as usize));

    // The spellings the selected command's own declaration speaks for, on this object.
    //
    // Empty unless that declaration really is this object's: a parent and child may each
    // declare `--clean` without merging, leaving two flags that share a name, and the
    // ancestor's must not be read as the child's. The test is whether every spelling the child
    // declared resolves back here — true of a merged flag, and of a plain local one, but not of
    // an ancestor whose long form the child took over.
    let child_spellings = |flag: &Arc<SpecFlag>| -> HashSet<String> {
        let declared: HashSet<String> = out
            .cmd
            .flags
            .iter()
            .filter(|declared| declared.name == flag.name)
            .flat_map(flag_keys)
            .collect();
        // *Any* of them, not all. All was too strong: a child may declare a spelling that
        // some other inherited global already owns — `-c --clean` beside an inherited
        // `-c --config` — and that collision is resolved in the other global's favor, so the
        // child's `-c` resolves elsewhere. Requiring every spelling to land here let one
        // unrelated collision disown the child from the `--clean` it plainly does own.
        //
        // Still enough to tell the two-object case apart, which is what this guards: when a
        // child re-declares a global as global, the child's own spellings resolve to the
        // child's separate flag, so none of them lands on the ancestor's.
        let speaks_for_this_flag = declared.iter().any(|spelling| {
            out.available_flags
                .get(spelling)
                .is_some_and(|available| Arc::ptr_eq(available, flag))
        });
        if speaks_for_this_flag {
            declared
        } else {
            HashSet::new()
        }
    };

    // Whose `exclusive` an occurrence activates, as `(the child's, an ancestor's)`.
    //
    // A child that re-declares an inherited global merges into one object answering to two
    // alias sets whose declarations may disagree, so there is no single owner to name: the
    // child owns the spellings it declared and the ancestor keeps the ones only it declared.
    // Both sides can be in play at once — `run -c --clean` is the ancestor's alias and the
    // child's in one invocation — and each carries its own declaration's answer.
    let exclusivity_in_play = |flag: &Arc<SpecFlag>| -> (bool, bool) {
        let child = child_spellings(flag);
        let child_exclusive = !child.is_empty()
            && out
                .cmd
                .flags
                .iter()
                .any(|declared| declared.name == flag.name && declared.exclusive);
        match parsed_flag_spellings.get(&(Arc::as_ptr(flag) as usize)) {
            Some(spellings) => (
                child_exclusive && spellings.iter().any(|s| child.contains(s)),
                flag.exclusive && spellings.iter().any(|s| !child.contains(s)),
            ),
            // An environment value has no spelling to attribute it by. The declaration the
            // selected command has in scope is the one that answers — which is the child's
            // when it re-declared the flag, and the ancestor's when it did not.
            None => (child_exclusive, flag.exclusive && child.is_empty()),
        }
    };

    let exclusive_occurrence = |flag: &Arc<SpecFlag>| {
        let (child, ancestor) = exclusivity_in_play(flag);
        child || ancestor
    };

    // clap's `exclusive` is also an escape from requiredness: `--version` has to work on a
    // command that otherwise needs an input. Companions are still diagnosed below, but an
    // exclusive occurrence suppresses the missing-value checks that would make it unusable
    // whether it was alone or not.
    let exclusive_present =
        unique_flags(out.available_flags.values().chain(out.flags.keys())).any(|flag| {
            exclusive_occurrence(flag)
                && !overridden_flags.contains(&flag.name)
                && (flag_was_parsed(flag) || flag_has_env(flag, custom_env))
        });

    // A command that says it needs a subcommand, given none. Checked on `out.cmd` and nowhere
    // else, because `out.cmd` *is* the command the words reached: had a subcommand been taken,
    // the child would be here instead. The spec has carried `subcommand_required` since it was
    // added for the derive, and this parser never read it — so `mise generate` parsed as a
    // complete invocation while usage-argv and clap both refused it.
    if out.cmd.subcommand_required && !out.cmd.subcommands.is_empty() {
        let mut names: Vec<&str> = out
            .cmd
            .subcommands
            .iter()
            // Aliases share a map entry with the name they point at; listing both would offer
            // the same command twice under two spellings.
            .filter(|(name, sub)| sub.name == **name && !sub.hide)
            .map(|(name, _)| name.as_str())
            .collect();
        names.sort_unstable();
        out.errors.push(UsageErr::MissingSubcommand(
            out.cmd.name.clone(),
            names.join(", "),
        ));
    }

    // Not `skip(out.args.len())`: a `--` may have jumped the cursor past an arg that stayed
    // empty, so position and fill count can disagree. Ask `out.args` which args it holds.
    if !exclusive_present {
        for arg in out.cmd.args.iter() {
            if out.args.contains_key(arg) {
                continue;
            }
            // Already reported as needing a `--`; one mistake should not yield two messages.
            if double_dash_violations.contains(&arg.name) {
                continue;
            }
            if arg.required && arg.default.is_empty() {
                // Check if there's an env var available (custom env map takes precedence)
                let has_env = arg
                    .env
                    .as_ref()
                    .is_some_and(|env_var| env_contains(custom_env, env_var));
                if !has_env {
                    out.errors.push(UsageErr::MissingArg(arg.name.clone()));
                }
            }
        }
    }

    // Conflicts are a question about the invocation as a whole rather than about any one
    // token, so they are checked here beside the requirement checks rather than at the
    // point a flag is matched — the flag it conflicts with may still be ahead of it.
    // Its own loop: the requirement loop below skips the flags that *were* given, which
    // is exactly the set this needs.
    //
    // A value from the environment counts on both sides, matching what
    // `selector_is_explicit` says about the other flag: the question is whether a flag
    // has a value, not how it got one. That is what clap does, and an asymmetric rule
    // would make the same pair of flags a conflict or not depending on which one
    // happened to be typed.
    for flag in unique_flags(out.available_flags.values()) {
        let given = out.flags.contains_key(flag) || flag_has_env(flag, custom_env);
        if !given || overridden_flags.contains(&flag.name) {
            continue;
        }
        for other in &flag.conflicts {
            if selector_is_explicit(other, &out, &overridden_flags, custom_env) {
                out.errors.push(UsageErr::InvalidFlag {
                    token: format!("--{}", flag.name),
                    reason: format!("conflicts with {other}"),
                    span: (0, 0).into(),
                    input: format!("--{} {other}", flag.name),
                });
            }
        }
        // The positive form, checked in the same pass and under the same rule: a value
        // from the environment satisfies a requirement, because the question is whether
        // the other flag has a value rather than how it got one. A flag that was
        // overridden away has not been given, so it cannot satisfy anything either —
        // which is what `selector_is_explicit` already accounts for.
        //
        // Reported as the missing flag rather than as something wrong with the flag that
        // named it, which is what clap says too: an unmet `requires` is a required
        // argument that was not provided. Named by its own name, resolved through the
        // same matcher, so a `requires="-f"` reports `--force` rather than the selector.
        if !exclusive_present {
            for other in &flag.requires {
                if !selector_is_satisfied(other, &out, &overridden_flags, custom_env) {
                    let name = selector_flag_name(other, &out).unwrap_or_else(|| other.clone());
                    out.errors.push(UsageErr::MissingFlag(name));
                }
            }
        }
    }

    // An exclusive flag is the whole-command form of a conflict: `--version` means the
    // rest of the line has nothing to act on. Everything the invocation supplied counts,
    // positionals included, which is what distinguishes it from being in a group with
    // every other flag.
    //
    // Only what was *given*, as `conflicts` reads it: a defaulted flag standing beside an
    // exclusive one is nobody saying anything, and counting it would make the exclusive
    // flag unusable on any command that has a default. Environment values do count, also as
    // `conflicts` reads them, so the spec parser and the derive agree.
    for flag in unique_flags(out.available_flags.values().chain(out.flags.keys())) {
        // `SpecFlag` equality is intentionally name-only for the public parsed-value map,
        // but re-declared aliases can leave distinct declarations with that same name in
        // scope. Exclusivity is about the declaration the typed spelling resolved to, so
        // compare the parser's `Arc`s by identity here.
        let given = flag_was_parsed(flag) || flag_has_env(flag, custom_env);
        if !exclusive_occurrence(flag) || !given || overridden_flags.contains(&flag.name) {
            continue;
        }
        let other_flag = unique_flags(out.available_flags.values().chain(out.flags.keys()))
            .find(|other| {
                !Arc::ptr_eq(other, flag)
                    && !overridden_flags.contains(&other.name)
                    && (flag_was_parsed(other) || flag_has_env(other, custom_env))
            })
            .map(|other| format!("--{}", other.name));
        let other_arg = out.cmd.args.iter().find(|arg| {
            out.args.keys().any(|given| given.name == arg.name)
                || arg
                    .env
                    .as_ref()
                    .is_some_and(|env| env_contains(custom_env, env))
        });
        // Selecting a child is company for an exclusive flag declared by an ancestor. An
        // exclusive flag belonging to the child itself does not conflict with the command word
        // needed to reach that child — so the question is not who owns the flag but whose
        // exclusivity is the one being enforced, which is what `exclusivity_in_play` already
        // separated.
        let (_, ancestor_exclusivity) = exclusivity_in_play(flag);
        let selected_subcommand =
            (out.cmds.len() > 1 && ancestor_exclusivity).then(|| out.cmd.name.clone());
        let other = other_flag
            .or_else(|| other_arg.map(|arg| format!("<{}>", arg.name)))
            .or(selected_subcommand);
        if let Some(other) = other {
            out.errors.push(UsageErr::InvalidFlag {
                token: format!("--{}", flag.name),
                reason: format!("must be given on its own, and {other} was given too"),
                span: (0, 0).into(),
                input: format!("--{} {other}", flag.name),
            });
        }
    }

    // Groups, checked once per group rather than per flag: both questions a group asks —
    // how many members were given, and whether that is enough — are about the set, which
    // is the whole reason a group exists rather than a pile of pairwise conflicts.
    //
    // The same "given" rule as everything else here, so a member filled from the
    // environment or a default counts.
    // Every command in the chain, not only the selected one: a group may name global
    // flags, which belong to an ancestor and are declared there.
    let mut group_errors: Vec<UsageErr> = Vec::new();
    for group in out.cmds.iter().flat_map(|cmd| &cmd.groups) {
        // Counted by the *flag* a selector resolves to, not by the selector. `-f` and
        // `--file` are two spellings of one flag, and a group naming both — or naming one
        // flag twice — would otherwise report that flag as conflicting with itself the
        // moment it was given. Deduplicated rather than refused where the group is
        // written, because listing both spellings is redundant, not wrong.
        let mut given: Vec<&str> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for selector in &group.members {
            if !selector_is_explicit(selector, &out, &overridden_flags, custom_env) {
                continue;
            }
            let name = selector_flag_name(selector, &out).unwrap_or_else(|| selector.clone());
            if seen.contains(&name) {
                continue;
            }
            seen.push(name);
            given.push(selector.as_str());
        }
        if !group.multiple && given.len() > 1 {
            group_errors.push(UsageErr::InvalidFlag {
                token: given[1].to_string(),
                reason: format!("cannot be used with {} in group {}", given[0], group.name),
                span: (0, 0).into(),
                input: format!("{} {}", given[0], given[1]),
            });
        }
        // Requiredness is a *positive* rule, so it reads a default as filling a member —
        // the rule `requires` follows. That is also why it cannot reuse `given` above:
        // exclusivity must count only what was supplied, or a defaulted member would
        // collide with the sibling the user actually typed.
        let satisfied = group
            .members
            .iter()
            .any(|selector| selector_is_satisfied(selector, &out, &overridden_flags, custom_env));
        if group.required && !satisfied && !exclusive_present {
            // The members are what a user has to type, so they are in the message; the
            // group's name is there too, since a command with several groups would
            // otherwise report the same sentence twice with nothing to tell them apart.
            group_errors.push(UsageErr::MissingGroup {
                group: group.name.clone(),
                members: group.members.join(", "),
            });
        }
    }
    out.errors.extend(group_errors);

    if !exclusive_present {
        for flag in unique_flags(out.available_flags.values()) {
            if out.flags.contains_key(flag) || overridden_flags.contains(&flag.name) {
                continue;
            }
            let has_default =
                !flag.default.is_empty() || flag.arg.iter().any(|a| !a.default.is_empty());
            let has_env = flag_has_env(flag, custom_env);
            let required_if = flag.required_if.iter().any(|selector| {
                selector_is_explicit(selector, &out, &overridden_flags, custom_env)
            });
            let required_unless = !flag.required_unless.is_empty()
                && !flag.required_unless.iter().any(|selector| {
                    selector_is_explicit(selector, &out, &overridden_flags, custom_env)
                });
            if (flag.required || required_if || required_unless) && !has_default && !has_env {
                out.errors.push(UsageErr::MissingFlag(flag.name.clone()));
            }
        }
    }

    // Validate var_min/var_max constraints for variadic args
    for (arg, value) in &out.args {
        if arg.var {
            if let ParseValue::MultiString(values) = value {
                if let Some(min) = arg.var_min {
                    if values.len() < min {
                        out.errors.push(UsageErr::VarArgTooFew {
                            name: arg.name.clone(),
                            min,
                            got: values.len(),
                        });
                    }
                }
                if let Some(max) = arg.var_max {
                    if values.len() > max {
                        out.errors.push(UsageErr::VarArgTooMany {
                            name: arg.name.clone(),
                            max,
                            got: values.len(),
                        });
                    }
                }
            }
        }
    }

    // Validate var_min/var_max constraints for variadic flags
    for (flag, value) in &out.flags {
        if flag.var {
            let count = match value {
                ParseValue::MultiString(values) => values.len(),
                ParseValue::MultiBool(values) => values.len(),
                _ => continue,
            };
            if let Some(min) = flag.var_min {
                if count < min {
                    out.errors.push(UsageErr::VarFlagTooFew {
                        name: flag.name.clone(),
                        min,
                        got: count,
                    });
                }
            }
            if let Some(max) = flag.var_max {
                if count > max {
                    out.errors.push(UsageErr::VarFlagTooMany {
                        name: flag.name.clone(),
                        max,
                        got: count,
                    });
                }
            }
        }
    }

    Ok((out, overridden_flags))
}

fn flag_matches_selector(flag: &SpecFlag, selector: &str) -> bool {
    flag.name == selector || flag_keys(flag).iter().any(|key| key == selector)
}

fn flags_override(overrider: &SpecFlag, overridden: &SpecFlag) -> bool {
    overrider
        .overrides
        .iter()
        .any(|selector| flag_matches_selector(overridden, selector))
}

fn apply_prefix_flag_overrides(
    prefix_flags: &mut Vec<(Arc<SpecFlag>, Vec<String>)>,
    flag: Arc<SpecFlag>,
) {
    prefix_flags
        .retain(|(other, _)| !(flags_override(&flag, other) || flags_override(other, &flag)));
}

fn mount_prefix_words(prefix_flags: &[(Arc<SpecFlag>, Vec<String>)]) -> Vec<String> {
    prefix_flags
        .iter()
        .flat_map(|(_, words)| words.iter().cloned())
        .collect()
}

fn env_contains(custom_env: Option<&HashMap<String, String>>, env_var: &str) -> bool {
    match custom_env {
        Some(env) => env.contains_key(env_var),
        None => std::env::var(env_var).is_ok(),
    }
}

fn flag_has_env(flag: &SpecFlag, custom_env: Option<&HashMap<String, String>>) -> bool {
    flag.env
        .as_ref()
        .is_some_and(|env_var| env_contains(custom_env, env_var))
}

fn selector_is_explicit(
    selector: &str,
    out: &ParseOutput,
    overridden_flags: &HashSet<String>,
    custom_env: Option<&HashMap<String, String>>,
) -> bool {
    out.available_flags
        .values()
        .chain(out.flags.keys())
        .any(|flag| {
            flag_matches_selector(flag, selector)
                && !overridden_flags.contains(&flag.name)
                && (out.flags.contains_key(flag) || flag_has_env(flag, custom_env))
        })
}

/// The name of the flag a selector points at, for an error that has to name it.
///
/// `selector_is_explicit` only answers yes or no, which is all a check needs; a message
/// about a flag that is *missing* has to say which one, and the selector may be a short
/// form or an alias rather than the name.
fn selector_flag_name(selector: &str, out: &ParseOutput) -> Option<String> {
    out.available_flags
        .values()
        .chain(out.flags.keys())
        .find(|flag| flag_matches_selector(flag, selector))
        .map(|flag| flag.name.clone())
}

/// Whether a selector's flag ended up with a value, however it got one.
///
/// The rule for a *positive* relationship, and the difference from
/// [`selector_is_explicit`] is deliberate. A negative rule — `conflicts`, or a group's
/// exclusivity — has to count only what was given, or a flag with a default would
/// conflict with everything and no command line would parse. A positive one asks whether
/// the flag it names has a value, and a default is a value: that is already how plain
/// `required`, `required_if` and `required_unless` read a default, and `requires` saying
/// otherwise would have made the same flag missing here and present ten lines below.
fn selector_is_satisfied(
    selector: &str,
    out: &ParseOutput,
    overridden_flags: &HashSet<String>,
    custom_env: Option<&HashMap<String, String>>,
) -> bool {
    if selector_is_explicit(selector, out, overridden_flags, custom_env) {
        return true;
    }
    out.available_flags
        .values()
        .chain(out.flags.keys())
        .filter(|flag| flag_matches_selector(flag, selector))
        .any(|flag| {
            !overridden_flags.contains(&flag.name)
                && (!flag.default.is_empty() || flag.arg.iter().any(|a| !a.default.is_empty()))
        })
}

fn apply_flag_overrides(
    flag: &Arc<SpecFlag>,
    available_flags: &BTreeMap<String, Arc<SpecFlag>>,
    parsed_flags: &mut IndexMap<Arc<SpecFlag>, ParseValue>,
    pending_flags: &mut Vec<Arc<SpecFlag>>,
    overridden_flags: &mut HashSet<String>,
) {
    let overridden_names: HashSet<String> = available_flags
        .values()
        .chain(parsed_flags.keys())
        .filter(|other| flags_override(flag, other) || flags_override(other, flag))
        .map(|other| other.name.clone())
        .collect();

    parsed_flags.retain(|parsed, _| !overridden_names.contains(&parsed.name));
    pending_flags.retain(|pending| !overridden_names.contains(&pending.name));
    overridden_flags.extend(overridden_names);
    // An explicit occurrence always restores this flag, including self-overrides.
    overridden_flags.remove(&flag.name);
}

#[cfg(feature = "docs")]
fn render_help_err(spec: &Spec, cmd: &SpecCommand, long: bool) -> UsageErr {
    UsageErr::Help(docs::cli::render_help(spec, cmd, long))
}

#[cfg(not(feature = "docs"))]
fn render_help_err(_spec: &Spec, _cmd: &SpecCommand, _long: bool) -> UsageErr {
    UsageErr::Help("help".to_string())
}

#[derive(Copy, Clone)]
struct ChoiceTarget<'a> {
    kind: &'a str,
    name: &'a str,
}

impl<'a> ChoiceTarget<'a> {
    fn arg(arg: &'a SpecArg) -> Self {
        Self {
            kind: "arg",
            name: &arg.name,
        }
    }

    fn option(flag: &'a SpecFlag) -> Self {
        Self {
            kind: "option",
            name: &flag.name,
        }
    }
}

/// Whether every letter of a short token names a flag in scope.
///
/// Scanning stops at the first letter whose flag takes a value, because everything
/// after it is that value rather than more letters.
fn short_bundle_is_known(available: &BTreeMap<String, Arc<SpecFlag>>, token: &str) -> bool {
    for c in token.chars().skip(1) {
        match available.get(&format!("-{c}")) {
            None => return false,
            Some(f) if f.arg.is_some() => return true,
            Some(_) => {}
        }
    }
    true
}

/// Refuse a flag-like token that named nothing, if this command asked for that.
///
/// Called from the flag branches, where the lookup has just failed and nothing from
/// the token has been applied yet — so a bundle like `-az` is refused whole rather
/// than after setting `-a`.
fn reject_unknown_flag_if_asked(
    spec: &Spec,
    path: &[SpecCommand],
    token: &str,
) -> Result<(), UsageErr> {
    // A lone `-` is a value by convention and a negative number is a value because
    // no CLI could accept `--offset -1` otherwise. Both reach the short-flag branch
    // and neither is a misspelled flag, so neither is refused. oclif made exactly
    // this mistake when it switched to refusing unknown flags, and had to add the
    // number case back.
    if !is_flag_like(token) {
        return Ok(());
    }
    if effective_unknown_flags(spec, path) != UnknownFlags::Error {
        return Ok(());
    }
    Err(UsageErr::InvalidFlag {
        token: token.to_string(),
        reason: "no such flag".to_string(),
        span: (0, 0).into(),
        input: token.to_string(),
    })
}

/// Whether a flag-like token that matches nothing is a value or an error, here.
///
/// The nearest enclosing command that stated a preference wins, then the spec,
/// then the default. Inherited, unlike `effect`: it describes how a command line
/// is read, and a CLI that forwards options tends to forward them at every level.
fn effective_unknown_flags(spec: &Spec, path: &[SpecCommand]) -> UnknownFlags {
    path.iter()
        .rev()
        .find_map(|cmd| cmd.unknown_flags)
        .or(spec.unknown_flags)
        .unwrap_or_default()
}

/// Whether a token would be read as a flag, for the purpose of rejecting unknown
/// ones.
///
/// A lone `-` is a value by convention, and a negative number is a value because no
/// CLI could accept `--offset -1` otherwise. The number has to actually be one:
/// treating anything after a digit as numeric would let `-1x` slip past a CLI that
/// asked for unknown flags to be refused.
fn is_flag_like(token: &str) -> bool {
    match token.strip_prefix('-') {
        None | Some("") => false,
        Some(rest) => !is_number(rest),
    }
}

/// Digits, at most one `.`, and an optional exponent.
///
/// Spelled out rather than deferred to `f64::from_str`, which also accepts `inf` and
/// `NaN`: `-inf` is far likelier to be a misspelled flag than a number somebody meant
/// to pass. usage-argv implements the same rule, and the corpus pins the edges — the
/// two disagreed about `-1e5` when one used a float parse and the other did not.
fn is_number(rest: &str) -> bool {
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(at) => (&rest[..at], Some(&rest[at + 1..])),
        None => (rest, None),
    };

    let mut seen_digit = false;
    let mut seen_dot = false;
    for c in mantissa.chars() {
        match c {
            '0'..='9' => seen_digit = true,
            '.' if !seen_dot => seen_dot = true,
            _ => return false,
        }
    }
    if !seen_digit {
        return false;
    }

    match exponent {
        None => true,
        Some(exp) => {
            let digits = exp
                .strip_prefix('+')
                .or_else(|| exp.strip_prefix('-'))
                .unwrap_or(exp);
            !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())
        }
    }
}

/// Bind one value to the flag waiting for it, and let a variadic argument go on
/// collecting from the words that follow.
///
/// Every route to a flag's value comes through here — the following word, the text
/// after an `=`, and the token a `allow_hyphen_values` flag takes whatever it looks
/// like — so that all three agree on how many values the flag ends up with.
#[allow(clippy::too_many_arguments)]
fn bind_pending_flag_value(
    spec: &Spec,
    cmd: &SpecCommand,
    errors: &mut Vec<UsageErr>,
    flags: &mut IndexMap<Arc<SpecFlag>, ParseValue>,
    flag_awaiting_value: &mut Vec<Arc<SpecFlag>>,
    word: &mut String,
    input: &mut VecDeque<String>,
    prefix_bindings: &mut VecDeque<Option<Arc<SpecFlag>>>,
    custom_env: Option<&HashMap<String, String>>,
) -> miette::Result<bool> {
    // Held before the drain pops it, along with what the flag is already carrying: a
    // `var_max` bounds the values this occurrence takes, not the list they are appended
    // to, so a second `--include` starts counting again.
    let collecting = flag_awaiting_value
        .last()
        .filter(|flag| flag.arg.as_ref().is_some_and(|arg| arg.var))
        .cloned()
        .map(|flag| {
            let carried = flags.get(&flag).map(value_count).unwrap_or(0);
            (flag, carried)
        });
    if drain_pending_flag_values(
        spec,
        cmd,
        errors,
        flags,
        flag_awaiting_value,
        word,
        custom_env,
    )? {
        return Ok(true);
    }
    let Some((flag, carried)) = collecting else {
        return Ok(false);
    };
    collect_variadic_flag_values(
        spec,
        cmd,
        errors,
        flags,
        flag_awaiting_value,
        &flag,
        carried,
        input,
        prefix_bindings,
        custom_env,
    )
}

/// Keep feeding a flag whose argument is variadic from the words that follow it.
///
/// `--include <pattern>...` collects from a single occurrence, so it takes tokens until
/// one is flag-like, a `--` arrives, its `var_max` is reached, or the command line ends.
/// This is greedy by design — a command declaring both such a flag and positionals will
/// find the flag eating them, and `--` or a `var_max` is how the run is stopped.
///
/// `carried` is what the flag already held when this occurrence began, so the bound
/// counts this run rather than everything the flag has collected across the command
/// line. Each value goes through the same drain as the first, so choices are checked
/// and the value lands in the same list rather than by a second route that could
/// disagree.
#[allow(clippy::too_many_arguments)]
fn collect_variadic_flag_values(
    spec: &Spec,
    cmd: &SpecCommand,
    errors: &mut Vec<UsageErr>,
    flags: &mut IndexMap<Arc<SpecFlag>, ParseValue>,
    flag_awaiting_value: &mut Vec<Arc<SpecFlag>>,
    flag: &Arc<SpecFlag>,
    carried: usize,
    input: &mut VecDeque<String>,
    prefix_bindings: &mut VecDeque<Option<Arc<SpecFlag>>>,
    custom_env: Option<&HashMap<String, String>>,
) -> miette::Result<bool> {
    let max = flag
        .arg
        .as_ref()
        .and_then(|arg| arg.var_max)
        .unwrap_or(usize::MAX);
    while flags
        .get(flag)
        .map(value_count)
        .unwrap_or(0)
        .saturating_sub(carried)
        < max
    {
        let Some(next) = input.front() else { break };
        // The separator is left where it is: stopping here hands it to the arm that
        // knows what it means, rather than reading it as one more value.
        if next == "--" || is_flag_like(next) {
            break;
        }
        let mut word = input.pop_front().unwrap();
        // The two queues are read in step, so a word taken here takes its binding with it.
        prefix_bindings.pop_front();
        flag_awaiting_value.push(Arc::clone(flag));
        if drain_pending_flag_values(
            spec,
            cmd,
            errors,
            flags,
            flag_awaiting_value,
            &mut word,
            custom_env,
        )? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// How many values a flag is holding, for a bound that counts them.
fn value_count(value: &ParseValue) -> usize {
    match value {
        ParseValue::MultiString(values) => values.len(),
        ParseValue::MultiBool(values) => values.len(),
        _ => 1,
    }
}

fn drain_pending_flag_values(
    spec: &Spec,
    cmd: &SpecCommand,
    errors: &mut Vec<UsageErr>,
    flags: &mut IndexMap<Arc<SpecFlag>, ParseValue>,
    flag_awaiting_value: &mut Vec<Arc<SpecFlag>>,
    word: &mut String,
    custom_env: Option<&HashMap<String, String>>,
) -> miette::Result<bool> {
    while let Some(flag) = flag_awaiting_value.pop() {
        let arg = flag.arg.as_ref().unwrap();
        if validate_choices(
            spec,
            cmd,
            errors,
            ChoiceTarget::option(&flag),
            word,
            arg.choices.as_ref(),
            custom_env,
        )? {
            return Ok(true);
        }
        let value = std::mem::take(word);
        // Two ways to hold several values, and both record a list: a `var` flag
        // collects one per occurrence, a variadic argument collects several from one.
        if flag.var || arg.var {
            let arr = flags
                .entry(flag)
                .or_insert_with(|| ParseValue::MultiString(vec![]))
                .try_as_multi_string_mut()
                .unwrap();
            arr.push(value);
        } else {
            flags.insert(flag, ParseValue::String(value));
        }
    }
    Ok(false)
}

fn choice_error(
    target: ChoiceTarget<'_>,
    value: &str,
    choices: Option<&SpecChoices>,
    custom_env: Option<&HashMap<String, String>>,
) -> Option<String> {
    let choices = choices?;
    let values = choices.values_with_env(custom_env);
    if values.iter().any(|choice| choice == value) {
        return None;
    }
    if let Some(env) = choices.env() {
        if values.is_empty() {
            return Some(format!(
                "Invalid choice for {} {}: {value}, no choices resolved from env {env}",
                target.kind, target.name,
            ));
        }
    }
    Some(format!(
        "Invalid choice for {} {}: {value}, expected one of {}",
        target.kind,
        target.name,
        values.join(", ")
    ))
}

fn validate_choices(
    spec: &Spec,
    cmd: &SpecCommand,
    errors: &mut Vec<UsageErr>,
    target: ChoiceTarget<'_>,
    value: &str,
    choices: Option<&SpecChoices>,
    custom_env: Option<&HashMap<String, String>>,
) -> miette::Result<bool> {
    if is_help_arg(spec, value)
        && choices.is_some_and(|choices| {
            !choices
                .values_with_env(custom_env)
                .iter()
                .any(|choice| choice == value)
        })
    {
        errors.push(render_help_err(spec, cmd, value.len() > 2));
        return Ok(true);
    }

    if let Some(err) = choice_error(target, value, choices, custom_env) {
        bail!("{err}");
    }
    Ok(false)
}

fn validate_choice_value(
    target: ChoiceTarget<'_>,
    value: &str,
    choices: Option<&SpecChoices>,
    custom_env: Option<&HashMap<String, String>>,
) -> miette::Result<()> {
    if let Some(err) = choice_error(target, value, choices, custom_env) {
        bail!("{err}");
    }
    Ok(())
}

fn validate_choice_values(
    target: ChoiceTarget<'_>,
    values: &[String],
    choices: Option<&SpecChoices>,
    custom_env: Option<&HashMap<String, String>>,
) -> miette::Result<()> {
    for value in values {
        validate_choice_value(target, value, choices, custom_env)?;
    }
    Ok(())
}

/// Publish where Phase 2 left its positional cursor, so callers that do not re-run the parse —
/// completions, above all — agree with it. Called on every exit from the loop, including the
/// early ones that render help, where the cursor is still the useful answer.
fn record_cursor(out: &mut ParseOutput, next_arg_idx: usize, seen_double_dash: bool) {
    out.next_arg = out.cmd.args.get(next_arg_idx).cloned().map(Arc::new);
    out.double_dash_seen = seen_double_dash;
}

/// Record that `arg` was handed a word before the `--` it requires.
///
/// A variadic arg would otherwise report the same mistake once per word it was offered, so the
/// message is emitted only the first time each arg is seen. The set is also what suppresses the
/// `MissingArg` that a `required` + `double_dash="required"` arg would otherwise collect at the
/// end of the parse.
fn report_double_dash_violation(
    arg: &SpecArg,
    errors: &mut Vec<UsageErr>,
    violations: &mut HashSet<String>,
) {
    if violations.insert(arg.name.clone()) {
        errors.push(UsageErr::ArgRequiresDoubleDash(arg.name.clone()));
    }
}

fn is_help_arg(spec: &Spec, w: &str) -> bool {
    spec.disable_help != Some(true)
        && (w == "--help"
            || w == "-h"
            || w == "-?"
            || (spec.cmd.subcommands.is_empty() && w == "help"))
}

impl ParseOutput {
    pub fn as_env(&self) -> BTreeMap<String, String> {
        let mut env = BTreeMap::new();
        for (flag, val) in &self.flags {
            let key = format!("usage_{}", flag.name.to_snake_case());
            let val = match val {
                ParseValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
                ParseValue::String(s) => s.clone(),
                ParseValue::MultiBool(b) => b.iter().filter(|b| **b).count().to_string(),
                ParseValue::MultiString(s) => shell_words::join(s),
            };
            env.insert(key, val);
        }
        for (arg, val) in &self.args {
            let key = format!("usage_{}", arg.name.to_snake_case());
            env.insert(key, val.to_string());
        }
        env
    }
}

impl Display for ParseValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseValue::Bool(b) => write!(f, "{b}"),
            ParseValue::String(s) => write!(f, "{s}"),
            ParseValue::MultiBool(b) => write!(f, "{}", b.iter().join(" ")),
            ParseValue::MultiString(s) => write!(f, "{}", shell_words::join(s)),
        }
    }
}

impl Debug for ParseOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParseOutput")
            .field("cmds", &self.cmds.iter().map(|c| &c.name).join(" ").trim())
            .field(
                "args",
                &self
                    .args
                    .iter()
                    .map(|(a, w)| format!("{}: {w}", a.name))
                    .collect_vec(),
            )
            .field(
                "available_flags",
                &self
                    .available_flags
                    .iter()
                    .map(|(f, w)| format!("{f}: {w}"))
                    .collect_vec(),
            )
            .field(
                "flags",
                &self
                    .flags
                    .iter()
                    .map(|(f, w)| format!("{}: {w}", f.name))
                    .collect_vec(),
            )
            .field("flag_awaiting_value", &self.flag_awaiting_value)
            .field("errors", &self.errors)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    fn spec_with_arg(arg: SpecArg) -> Spec {
        let cmd = SpecCommand::builder().name("test").arg(arg).build();
        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    fn spec_with_flag(flag: SpecFlag) -> Spec {
        let cmd = SpecCommand::builder().name("test").flag(flag).build();
        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    fn parse_with_env(
        spec: &Spec,
        words: &[&str],
        env: &[(&str, &str)],
    ) -> Result<ParseOutput, miette::Error> {
        let env = env
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        Parser::new(spec).with_env(env).parse(&input(words))
    }

    fn first_string_value(parsed: &ParseOutput) -> &str {
        if let Some(ParseValue::String(value)) = parsed.args.values().next() {
            return value;
        }
        if let Some(ParseValue::String(value)) = parsed.flags.values().next() {
            return value;
        }
        panic!("expected first parsed value to be ParseValue::String");
    }

    fn flag_string_value<'a>(parsed: &'a ParseOutput, name: &str) -> &'a str {
        let flag = parsed
            .flags
            .keys()
            .find(|flag| flag.name == name)
            .unwrap_or_else(|| panic!("expected flag {name}"));
        let value = parsed
            .flags
            .get(flag)
            .unwrap_or_else(|| panic!("expected value for flag {name}"));
        match value {
            ParseValue::String(value) => value,
            _ => panic!("expected flag {name} to be ParseValue::String"),
        }
    }

    fn assert_parse_err(result: Result<ParseOutput, miette::Error>, expected: &str) {
        let err = result.expect_err("expected parser error");
        assert_eq!(format!("{err}"), expected);
    }

    #[cfg(feature = "unstable_choices_env")]
    fn spec_arg_choices_env(key: &str) -> Spec {
        spec_with_arg(
            SpecArg::builder()
                .name("env")
                .choices_env(key)
                .required(false)
                .build(),
        )
    }

    #[cfg(feature = "unstable_choices_env")]
    fn spec_flag_choices_env(key: &str) -> Spec {
        spec_with_flag(
            SpecFlag::builder()
                .long("env")
                .arg(SpecArg::builder().name("env").choices_env(key).build())
                .build(),
        )
    }

    #[test]
    fn test_parse() {
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(SpecArg::builder().name("arg").build())
            .flag(SpecFlag::builder().long("flag").build())
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };
        let input = vec!["test".to_string(), "arg1".to_string(), "--flag".to_string()];
        let parsed = parse(&spec, &input).unwrap();
        assert_eq!(parsed.cmds.len(), 1);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.args.len(), 1);
        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(parsed.available_flags.len(), 1);
    }

    #[test]
    fn test_flag_overrides_last_occurrence_wins() {
        let spec: Spec = r#"
flag "--stdin" default=#true
flag "--file <file>" overrides="--stdin"
        "#
        .parse()
        .unwrap();

        let file_wins = parse(&spec, &input(&["test", "--stdin", "--file", "input.txt"])).unwrap();
        assert_eq!(file_wins.flags.len(), 1);
        assert_eq!(flag_string_value(&file_wins, "file"), "input.txt");
        assert!(!file_wins.flags.keys().any(|flag| flag.name == "stdin"));

        let stdin_wins = parse(&spec, &input(&["test", "--file", "input.txt", "--stdin"])).unwrap();
        assert_eq!(stdin_wins.flags.len(), 1);
        assert!(stdin_wins.flags.keys().any(|flag| flag.name == "stdin"));
        assert!(!stdin_wins.flags.keys().any(|flag| flag.name == "file"));
    }

    #[test]
    fn test_flag_override_clears_pending_value() {
        let spec: Spec = r#"
flag "--file <file>" overrides="--stdin"
flag "--stdin"
arg "[input]"
        "#
        .parse()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "--file", "--stdin", "input.txt"])).unwrap();
        assert_eq!(parsed.flags.len(), 1);
        assert!(parsed.flags.keys().any(|flag| flag.name == "stdin"));
        assert_eq!(first_string_value(&parsed), "input.txt");
    }

    #[cfg(unix)]
    #[test]
    fn a_mount_on_the_root_discovers_subcommands() {
        // The root is a command like any other, so it can find its own subcommands
        // by running something. Uses `echo` rather than a fixture because resolving
        // a mount is what is being tested.
        let spec: Spec = r#"
name "ex"
bin "ex"
cmd "declared"
mount run="echo 'cmd \"discovered\"'"
"#
        .parse()
        .unwrap();

        let out = parse(&spec, &["ex".to_string(), "discovered".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "discovered");
    }

    #[cfg(unix)]
    #[test]
    fn completion_sees_root_mounted_commands_with_nothing_typed() {
        // The case a root mount exists for. `mycli <tab>` has no word to trigger
        // discovery with, so a completion has to resolve up front or the mounted
        // commands are never offered.
        let spec: Spec = r#"
name "ex"
bin "ex"
cmd "declared"
mount run="echo 'cmd \"discovered\"'"
"#
        .parse()
        .unwrap();

        let out = parse_partial(&spec, &["ex".to_string()]).unwrap();
        assert!(
            out.cmd.subcommands.contains_key("discovered"),
            "a completion should see mounted commands; got {:?}",
            out.cmd.subcommands.keys().collect::<Vec<_>>()
        );
    }

    #[cfg(unix)]
    #[test]
    fn completion_and_execution_agree_about_discovery() {
        // Offering a command that a real parse would hand to the default instead is
        // worse than not offering it, so the gate applies to both paths. The mount
        // fails if it runs, which is how both halves are checked at once.
        let spec: Spec = r#"
name "ex"
bin "ex"
default_subcommand "run"
cmd "run" {
  arg "<task>"
}
mount run="exit 1"
"#
        .parse()
        .unwrap();

        let out = parse_partial(&spec, &["ex".to_string()]).unwrap();
        assert!(
            !out.cmd.subcommands.contains_key("discovered"),
            "a completion must not offer what execution will not route"
        );

        let out = parse(&spec, &["ex".to_string(), "mytask".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "run");
    }

    #[cfg(unix)]
    #[test]
    fn a_default_subcommand_outranks_discovery() {
        // The default already says what an unmatched word means, and says it for
        // free. The mount fails if it runs, so parsing proves discovery was skipped.
        let spec: Spec = r#"
name "ex"
bin "ex"
default_subcommand "run"
cmd "run" {
  arg "<task>"
}
mount run="exit 1"
"#
        .parse()
        .unwrap();

        let out = parse(&spec, &["ex".to_string(), "mytask".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "run");
    }

    #[cfg(unix)]
    #[test]
    fn a_mount_may_ask_to_outrank_the_default() {
        // Opting in, and paying for it: discovery runs first, so a discovered
        // command wins over the fallback.
        let spec: Spec = r#"
name "ex"
bin "ex"
default_subcommand "run"
cmd "run" {
  arg "<task>"
}
mount run="echo 'cmd \"discovered\"'" overrides_default=#true
"#
        .parse()
        .unwrap();

        let out = parse(&spec, &["ex".to_string(), "discovered".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "discovered");

        // A word it does not know still reaches the default.
        let out = parse(&spec, &["ex".to_string(), "mytask".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "run");
    }

    #[cfg(unix)]
    #[test]
    fn a_flag_does_not_run_the_mount() {
        // A flag matches no subcommand, which would have been enough to trigger
        // discovery — so `ex --help` spawned a process. The mount fails if it runs,
        // so parsing at all is the proof that it did not.
        let spec: Spec = r#"
name "ex"
bin "ex"
flag "--verbose"
cmd "declared"
mount run="exit 1"
"#
        .parse()
        .unwrap();

        let out = parse(&spec, &["ex".to_string(), "--verbose".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "ex");
    }

    #[cfg(unix)]
    #[test]
    fn a_declared_subcommand_does_not_run_the_mount() {
        // The mount would fail if it ran, so this parsing at all is the proof that
        // discovery is skipped when the word is already known. Worth pinning: a root
        // mount that resolved eagerly would spawn a process on every invocation.
        let spec: Spec = r#"
name "ex"
bin "ex"
cmd "declared"
mount run="exit 1"
"#
        .parse()
        .unwrap();

        let out = parse(&spec, &["ex".to_string(), "declared".to_string()]).unwrap();
        assert_eq!(out.cmd.name, "declared");
    }

    #[test]
    fn a_root_mount_survives_being_written_out() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nmount run=\"ex plugins --usage\"\n"
            .parse()
            .unwrap();
        assert_eq!(spec.cmd.mounts.len(), 1);

        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.cmd.mounts.len(), 1, "written:\n{spec}");
        assert_eq!(reparsed.cmd.mounts[0].run, "ex plugins --usage");
    }

    #[test]
    fn test_mount_prefix_applies_flag_overrides() {
        let stdin = Arc::new(
            SpecFlag::builder()
                .name("stdin")
                .long("stdin")
                .global(true)
                .build(),
        );
        let file = Arc::new(
            SpecFlag::builder()
                .name("file")
                .long("file")
                .arg(SpecArg::builder().name("file").build())
                .global(true)
                .overrides_with(vec!["--stdin".to_string()])
                .build(),
        );
        let mut prefix_flags = vec![(stdin, vec!["--stdin".to_string()])];

        apply_prefix_flag_overrides(&mut prefix_flags, Arc::clone(&file));
        prefix_flags.push((file, vec!["--file".to_string(), "input.txt".to_string()]));

        assert_eq!(mount_prefix_words(&prefix_flags), ["--file", "input.txt"]);
    }

    #[test]
    fn test_flag_override_suppresses_env_value() {
        let spec: Spec = r#"
flag "--stdin" env="USE_STDIN"
flag "--file <file>" overrides="--stdin"
        "#
        .parse()
        .unwrap();

        let parsed = parse_with_env(
            &spec,
            &["test", "--file", "input.txt"],
            &[("USE_STDIN", "true")],
        )
        .unwrap();
        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(flag_string_value(&parsed, "file"), "input.txt");
    }

    #[test]
    fn test_flag_override_suppresses_required_check() {
        let spec: Spec = r#"
flag "--stdin" required=#true
flag "--file <file>" overrides="--stdin"
        "#
        .parse()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "--file", "input.txt"])).unwrap();
        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(flag_string_value(&parsed, "file"), "input.txt");
    }

    #[test]
    fn test_flag_required_if() {
        let spec: Spec = r#"
flag "--dir <dir>"
flag "--file <file>" required_if="--dir"
        "#
        .parse()
        .unwrap();

        parse(&spec, &input(&["test"])).unwrap();
        assert_parse_err(
            parse(&spec, &input(&["test", "--dir", "src"])),
            "Missing required flag: --file <file>",
        );
        parse(
            &spec,
            &input(&["test", "--dir", "src", "--file", "input.txt"]),
        )
        .unwrap();
    }

    #[test]
    fn test_flag_required_unless() {
        let spec: Spec = r#"
flag "--stdin"
flag "--file <file>" required_unless="--stdin"
        "#
        .parse()
        .unwrap();

        assert_parse_err(
            parse(&spec, &input(&["test"])),
            "Missing required flag: --file <file>",
        );
        parse(&spec, &input(&["test", "--stdin"])).unwrap();
        parse(&spec, &input(&["test", "--file", "input.txt"])).unwrap();
    }

    #[test]
    fn test_conditional_requirements_treat_env_as_explicit() {
        let spec: Spec = r#"
flag "--dir <dir>" env="INPUT_DIR"
flag "--stdin" env="USE_STDIN"
flag "--file <file>" required_if="--dir" required_unless="--stdin"
        "#
        .parse()
        .unwrap();

        assert_parse_err(
            parse_with_env(&spec, &["test"], &[("INPUT_DIR", "src")]),
            "Missing required flag: --file <file>",
        );
        parse_with_env(&spec, &["test"], &[("USE_STDIN", "true")]).unwrap();
    }

    #[test]
    fn test_custom_env_does_not_fall_back_to_process_env() {
        assert!(std::env::var("PATH").is_ok());
        let spec: Spec = r#"flag "--file <file>" env="PATH" required=#true"#.parse().unwrap();

        assert_parse_err(
            parse_with_env(&spec, &["test"], &[]),
            "Missing required flag: --file <file>",
        );
    }

    #[test]
    fn test_conditional_requirements_ignore_defaults_on_condition_flags() {
        let spec: Spec = r#"
flag "--dir <dir>" default="src"
flag "--file <file>" required_if="--dir"
        "#
        .parse()
        .unwrap();

        parse(&spec, &input(&["test"])).unwrap();
    }

    #[test]
    fn test_conditional_requirements_see_overridden_flags_as_absent() {
        let spec: Spec = r#"
flag "--stdin"
flag "--dir <dir>" overrides="--stdin"
flag "--file <file>" required_unless="--stdin"
        "#
        .parse()
        .unwrap();

        assert_parse_err(
            parse(&spec, &input(&["test", "--stdin", "--dir", "src"])),
            "Missing required flag: --file <file>",
        );
    }

    #[test]
    fn short_flag_is_one_character_not_one_byte() {
        // A short is declared and read by character. Counting bytes instead either
        // refuses the declaration or slices the token inside the character, and clap
        // — which many specs are generated from — accepts shorts like this one.
        let spec = spec_with_flag(
            SpecFlag::builder()
                .short('磨')
                .long("polish")
                .arg(SpecArg::builder().name("opt").build())
                .build(),
        );
        let attached = Parser::new(&spec)
            .parse(&input(&["test", "-磨VALUE"]))
            .unwrap();
        assert_eq!(flag_string_value(&attached, "polish"), "VALUE");
        let detached = Parser::new(&spec)
            .parse(&input(&["test", "-磨", "V"]))
            .unwrap();
        assert_eq!(flag_string_value(&detached, "polish"), "V");
    }

    #[test]
    fn test_as_env() {
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(SpecArg::builder().name("arg").build())
            .flag(SpecFlag::builder().long("flag").build())
            .flag(
                SpecFlag::builder()
                    .long("force")
                    .negate("--no-force")
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };
        let input = vec![
            "test".to_string(),
            "--flag".to_string(),
            "--no-force".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();
        let env = parsed.as_env();
        assert_eq!(env.len(), 2);
        assert_eq!(env.get("usage_flag"), Some(&"true".to_string()));
        assert_eq!(env.get("usage_force"), Some(&"false".to_string()));
    }

    #[test]
    fn test_arg_env_var() {
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("input")
                    .env("TEST_ARG_INPUT")
                    .required(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var
        std::env::set_var("TEST_ARG_INPUT", "test_file.txt");

        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let arg = parsed.args.keys().next().unwrap();
        assert_eq!(arg.name, "input");
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "test_file.txt");

        // Clean up
        std::env::remove_var("TEST_ARG_INPUT");
    }

    #[test]
    fn test_flag_env_var_with_arg() {
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("output")
                    .env("TEST_FLAG_OUTPUT")
                    .arg(SpecArg::builder().name("file").build())
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var
        std::env::set_var("TEST_FLAG_OUTPUT", "output.txt");

        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let flag = parsed.flags.keys().next().unwrap();
        assert_eq!(flag.name, "output");
        let value = parsed.flags.values().next().unwrap();
        assert_eq!(value.to_string(), "output.txt");

        // Clean up
        std::env::remove_var("TEST_FLAG_OUTPUT");
    }

    #[test]
    fn test_flag_env_var_boolean() {
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("verbose")
                    .env("TEST_FLAG_VERBOSE")
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var to true
        std::env::set_var("TEST_FLAG_VERBOSE", "true");

        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let flag = parsed.flags.keys().next().unwrap();
        assert_eq!(flag.name, "verbose");
        let value = parsed.flags.values().next().unwrap();
        assert_eq!(value.to_string(), "true");

        // Clean up
        std::env::remove_var("TEST_FLAG_VERBOSE");
    }

    #[test]
    fn test_env_var_precedence() {
        // CLI args should take precedence over env vars
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("input")
                    .env("TEST_PRECEDENCE_INPUT")
                    .required(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Set env var
        std::env::set_var("TEST_PRECEDENCE_INPUT", "env_file.txt");

        let input = vec!["test".to_string(), "cli_file.txt".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // CLI arg should take precedence
        assert_eq!(value.to_string(), "cli_file.txt");

        // Clean up
        std::env::remove_var("TEST_PRECEDENCE_INPUT");
    }

    #[test]
    fn test_flag_var_true_with_single_default() {
        // When var=true and default="bar", the default should be MultiString(["bar"])
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("foo")
                    .var(true)
                    .arg(SpecArg::builder().name("foo").build())
                    .default_value("bar")
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the flag
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let flag = parsed.flags.keys().next().unwrap();
        assert_eq!(flag.name, "foo");
        let value = parsed.flags.values().next().unwrap();
        // Should be MultiString, not String
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0], "bar");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_flag_var_true_with_multiple_defaults() {
        // When var=true and multiple defaults, should return MultiString(["xyz", "bar"])
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("foo")
                    .var(true)
                    .arg(SpecArg::builder().name("foo").build())
                    .default_values(["xyz", "bar"])
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the flag
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let value = parsed.flags.values().next().unwrap();
        // Should be MultiString with both values
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "xyz");
                assert_eq!(v[1], "bar");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_flag_var_false_with_default_remains_string() {
        // When var=false (default), the default should still be String("bar")
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .long("foo")
                    .var(false) // Default behavior
                    .arg(SpecArg::builder().name("foo").build())
                    .default_value("bar")
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the flag
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        let value = parsed.flags.values().next().unwrap();
        // Should be String, not MultiString
        match value {
            ParseValue::String(s) => {
                assert_eq!(s, "bar");
            }
            _ => panic!("Expected String, got {:?}", value),
        }
    }

    #[test]
    fn test_arg_var_true_with_single_default() {
        // When arg has var=true and default="bar", the default should be MultiString(["bar"])
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("files")
                    .var(true)
                    .default_value("default.txt")
                    .required(false)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the arg
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // Should be MultiString, not String
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 1);
                assert_eq!(v[0], "default.txt");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_arg_var_true_with_multiple_defaults() {
        // When arg has var=true and multiple defaults
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("files")
                    .var(true)
                    .default_values(["file1.txt", "file2.txt"])
                    .required(false)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the arg
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // Should be MultiString with both values
        match value {
            ParseValue::MultiString(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "file1.txt");
                assert_eq!(v[1], "file2.txt");
            }
            _ => panic!("Expected MultiString, got {:?}", value),
        }
    }

    #[test]
    fn test_arg_var_false_with_default_remains_string() {
        // When arg has var=false (default), the default should still be String
        let cmd = SpecCommand::builder()
            .name("test")
            .arg(
                SpecArg::builder()
                    .name("file")
                    .var(false)
                    .default_value("default.txt")
                    .required(false)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // User doesn't provide the arg
        let input = vec!["test".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        // Should be String, not MultiString
        match value {
            ParseValue::String(s) => {
                assert_eq!(s, "default.txt");
            }
            _ => panic!("Expected String, got {:?}", value),
        }
    }

    #[test]
    fn test_scalar_defaults_validate_only_first_default_choice() {
        let specs = [
            spec_with_arg(
                SpecArg::builder()
                    .name("env")
                    .var(false)
                    .default_values(["dev", "prod"])
                    .choices(["dev"])
                    .required(false)
                    .build(),
            ),
            spec_with_flag(
                SpecFlag::builder()
                    .long("env")
                    .arg(
                        SpecArg::builder()
                            .name("env")
                            .default_values(["dev", "prod"])
                            .choices(["dev"])
                            .build(),
                    )
                    .build(),
            ),
        ];

        for spec in specs {
            let parsed = parse(&spec, &input(&["test"])).unwrap();
            assert_eq!(first_string_value(&parsed), "dev");
        }
    }

    #[test]
    fn an_exclusive_flag_has_to_be_alone() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--dump\" exclusive=#true\nflag \"--verbose\"\narg \"[target]\"\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--dump"])).expect("alone is the point");

        // Any other flag.
        let err = parse(&spec, &input(&["ex", "--dump", "--verbose"])).unwrap_err();
        assert!(err.to_string().contains("on its own"), "{err}");

        // And a positional, which is what makes this more than a conflict with every
        // other flag.
        let err = parse(&spec, &input(&["ex", "--dump", "t"])).unwrap_err();
        assert!(err.to_string().contains("on its own"), "{err}");

        // Not given, so it imposes nothing.
        parse(&spec, &input(&["ex", "--verbose", "t"])).expect("without it, nothing changes");
    }

    #[test]
    fn an_exclusive_flag_is_not_disturbed_by_a_default() {
        // Only what was supplied counts, as `conflicts` reads it. A default counting as
        // company would make an exclusive flag unusable on any command that has one.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--dump\" exclusive=#true\nflag \"--jobs <n>\" default=\"4\"\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--dump"])).expect("a default is nobody saying anything");
        assert!(parse(&spec, &input(&["ex", "--dump", "--jobs", "8"])).is_err());
    }

    #[test]
    fn an_exclusive_flag_bypasses_required_siblings() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--dump\" exclusive=#true\nflag \"--out <path>\" required=#true\narg \"<target>\"\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--dump"]))
            .expect("exclusive is the command's requiredness escape");
        assert!(parse(
            &spec,
            &input(&["ex", "--dump", "--out", "somewhere", "target"])
        )
        .is_err());
    }

    #[test]
    fn an_environment_value_counts_for_an_exclusive_flag() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--dump\" exclusive=#true\nflag \"--out <path>\" env=\"EX_OUT\"\n"
            .parse()
            .unwrap();

        assert!(parse_with_env(&spec, &["ex", "--dump"], &[("EX_OUT", "somewhere")]).is_err());
        parse_with_env(&spec, &["ex", "--dump"], &[]).expect("without the value it is alone");
    }

    #[test]
    fn a_selected_subcommand_counts_for_an_ancestor_exclusive_flag() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--version\" global=#true exclusive=#true\ncmd \"run\"\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--version"])).expect("alone is allowed");
        assert!(parse(&spec, &input(&["ex", "--version", "run"])).is_err());
    }

    #[test]
    fn a_child_exclusive_flag_is_not_mistaken_for_a_same_named_parent_flag() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" exclusive=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"]))
            .expect("the child flag is alone within the child command");
        assert!(
            parse(&spec, &input(&["ex", "--clean", "run"])).is_err(),
            "the parent flag still conflicts with selecting the child"
        );
    }

    #[test]
    fn a_child_local_exclusive_redeclaration_belongs_to_the_child() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true exclusive=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"]))
            .expect("the child-local exclusive flag is alone inside the child command");
        assert!(
            parse(&spec, &input(&["ex", "--clean", "run"])).is_err(),
            "the ancestor spelling still conflicts with selecting the child"
        );
    }

    #[test]
    fn a_same_named_parent_flag_is_company_for_a_child_exclusive_flag() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true\ncmd \"run\" {\n  flag \"--clean\" global=#true exclusive=#true\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"])).expect("the child exclusive flag is alone");
        assert!(
            parse(&spec, &input(&["ex", "--clean", "run", "--clean"])).is_err(),
            "the distinct parent declaration is still company despite sharing a name"
        );
    }

    #[test]
    fn a_local_child_redeclaration_keeps_its_exclusivity_when_merged() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"]))
            .expect("the child exclusive flag is valid alone");
        assert!(
            parse(&spec, &input(&["ex", "run", "--clean", "--verbose"])).is_err(),
            "merging with the inherited global must not discard child exclusivity"
        );
    }

    #[test]
    fn an_orphan_parent_alias_does_not_disown_a_child_local_exclusive_flag() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" global=#true exclusive=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"]))
            .expect("the typed long form belongs to the child declaration");
        assert!(
            parse(&spec, &input(&["ex", "run", "-c"])).is_err(),
            "the inherited short form still belongs to the ancestor"
        );
        assert!(
            parse(&spec, &input(&["ex", "run", "-c", "--clean"])).is_err(),
            "a child spelling cannot mask the ancestor-exclusive occurrence on the same merged flag"
        );
    }

    #[test]
    fn an_inherited_alias_keeps_its_ancestor_exclusivity() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" global=#true exclusive=#true\ncmd \"run\" {\n  flag \"--clean\" global=#true\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"]))
            .expect("the child's spelling does not activate the orphan ancestor alias");
        assert!(
            parse(&spec, &input(&["ex", "run", "-c"])).is_err(),
            "the inherited short alias still belongs to the ancestor exclusive flag"
        );
    }

    #[test]
    fn an_inherited_negated_alias_keeps_its_ancestor_exclusivity() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" negate=\"--no-clean\" global=#true exclusive=#true\ncmd \"run\" {\n  flag \"-c --clean\" global=#true\n}\n"
            .parse()
            .unwrap();

        assert!(
            parse(&spec, &input(&["ex", "run", "--no-clean"])).is_err(),
            "the inherited negated alias still belongs to the ancestor exclusive flag"
        );
    }

    #[test]
    fn a_colliding_alias_does_not_disown_the_child_from_the_rest() {
        // The child re-declares the inherited `--clean` as exclusive and gives it a `-c` that
        // an unrelated inherited global already owns. That collision is resolved in the other
        // global's favor, so the child's `-c` resolves elsewhere — but the child plainly owns
        // the `--clean` it declared, and its exclusivity holds.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true\nflag \"-c --config <f>\" global=#true\ncmd \"run\" {\n  flag \"-c --clean\" exclusive=#true\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"])).expect("alone is allowed");
        assert!(
            parse(&spec, &input(&["ex", "run", "--clean", "--verbose"])).is_err(),
            "one unrelated alias collision cannot disown the child from its own flag"
        );
    }

    #[test]
    fn a_local_child_declaration_is_not_in_scope_before_the_subcommand() {
        // A child's *local* re-declaration describes the flag at the child. Typed ahead of the
        // subcommand word the flag can only be the ancestor's, because that is the only one in
        // scope there — so the ancestor's exclusivity is the one that answers, whichever way it
        // is set. The pair below differ in nothing else, which is what makes this one rule
        // rather than two behaviors.
        let quiet: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();
        parse(&quiet, &input(&["ex", "--clean", "run", "--verbose"]))
            .expect("the ancestor owns this occurrence, and it is not exclusive");
        assert!(
            parse(&quiet, &input(&["ex", "run", "--clean", "--verbose"])).is_err(),
            "after the subcommand word the child's declaration is in scope, and it is exclusive"
        );

        let loud: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true exclusive=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n}\n"
            .parse()
            .unwrap();
        assert!(
            parse(&loud, &input(&["ex", "--clean", "run"])).is_err(),
            "the same rule, with an exclusive ancestor: selecting the child is company for it"
        );
    }

    #[test]
    fn an_orphan_ancestor_alias_keeps_its_exclusivity_past_a_plain_child_redeclaration() {
        // The mirror of `a_local_child_redeclaration_keeps_its_exclusivity_when_merged`: the
        // child owns `--clean` and says nothing about exclusivity, but `-c` is a spelling only
        // the ancestor ever declared, so the ancestor's answer still governs it.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" global=#true exclusive=#true\ncmd \"run\" {\n  flag \"--clean\"\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();

        assert!(
            parse(&spec, &input(&["ex", "run", "-c"])).is_err(),
            "the orphan ancestor alias is still the ancestor's exclusive flag"
        );
        parse(&spec, &input(&["ex", "run", "--clean", "--verbose"]))
            .expect("the child's own spelling drops the exclusivity the child did not restate");
    }

    #[test]
    fn a_child_spelling_carries_its_exclusivity_even_beside_an_ancestor_spelling() {
        // Both spellings of one merged flag, typed together. The child's `--clean` is exclusive
        // whatever else was typed alongside it, so `--verbose` is company; attributing the whole
        // occurrence to the ancestor because `-c` appeared in it lost that.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" global=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();

        assert!(
            parse(&spec, &input(&["ex", "run", "-c", "--clean", "--verbose"])).is_err(),
            "the child spelling is exclusive whatever it was typed beside"
        );
        parse(&spec, &input(&["ex", "run", "-c", "--verbose"]))
            .expect("the ancestor's own spelling was never exclusive");
    }

    #[test]
    fn an_environment_value_takes_the_exclusivity_of_the_declaration_in_scope() {
        // An environment value has no spelling to attribute, so the declaration the selected
        // command has in scope answers — in both directions. Comparing whole alias sets asked
        // the ancestor instead, because the merged flag also carries its orphan `-c`.
        let added: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" global=#true env=\"EX_CLEAN\"\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();

        assert!(
            parse_with_env(&added, &["ex", "run", "--verbose"], &[("EX_CLEAN", "1")]).is_err(),
            "the child added exclusivity the environment value has to honor"
        );

        let dropped: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-c --clean\" global=#true exclusive=#true env=\"EX_CLEAN\"\ncmd \"run\" {\n  flag \"--clean\"\n  flag \"--verbose\"\n}\n"
            .parse()
            .unwrap();

        parse_with_env(&dropped, &["ex", "run", "--verbose"], &[("EX_CLEAN", "1")])
            .expect("the child dropped the exclusivity, and the environment value follows it");
    }

    #[test]
    fn a_merged_child_exclusive_flag_still_escapes_requiredness() {
        // Exclusivity suppresses missing-value checks, and that has to survive the merge for
        // the same reason the companion check does.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--clean\" global=#true\ncmd \"run\" {\n  flag \"--clean\" exclusive=#true\n  flag \"--out <path>\" required=#true\n}\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "run", "--clean"]))
            .expect("a merged child exclusive flag is still the command's requiredness escape");
    }

    #[test]
    fn a_group_allows_one_member_and_refuses_two() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--file <f>\"\nflag \"--url <u>\"\nflag \"--stdin\"\ngroup \"input\" \"--file\" \"--url\" \"--stdin\"\n"
            .parse()
            .unwrap();

        // One is fine, and so is none: a plain group says "at most one".
        parse(&spec, &input(&["ex", "--file", "a.txt"])).expect("one member is fine");
        parse(&spec, &input(&["ex"])).expect("a group that is not required asks for nothing");

        let err = parse(&spec, &input(&["ex", "--file", "a.txt", "--stdin"])).unwrap_err();
        assert!(err.to_string().contains("group input"), "{err}");
    }

    #[test]
    fn a_required_group_needs_one_of_its_members() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--file <f>\"\nflag \"--url <u>\"\ngroup \"input\" \"--file\" \"--url\" required=#true\n"
            .parse()
            .unwrap();

        let err = parse(&spec, &input(&["ex"])).unwrap_err();
        // The members, because that is what a user has to type; the name, because a
        // command with several groups would otherwise report the same sentence twice.
        assert!(err.to_string().contains("--file, --url"), "{err}");
        assert!(err.to_string().contains("input"), "{err}");

        parse(&spec, &input(&["ex", "--url", "u"])).expect("one member satisfies it");
    }

    #[test]
    fn a_multiple_group_only_polices_requiredness() {
        // `multiple` with `required` is "at least one of these", so two is fine and
        // none is not.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--a\"\nflag \"--b\"\ngroup \"any\" \"--a\" \"--b\" required=#true multiple=#true\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--a", "--b"])).expect("multiple allows both");
        assert!(parse(&spec, &input(&["ex"])).is_err());
    }

    #[test]
    fn a_group_reads_a_default_for_requiredness_and_not_for_exclusivity() {
        // The two halves of a group are two kinds of rule, and they read a default
        // differently on purpose. Requiredness asks whether a member has a value, and a
        // default is a value — the rule `requires` follows. Exclusivity asks what the
        // user supplied, because a defaulted member counted as supplied would collide
        // with the sibling they actually typed and refuse a correct command line.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--file <f>\" default=\"a.txt\"\nflag \"--url <u>\"\ngroup \"input\" \"--file\" \"--url\" required=#true\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex"])).expect("the default fills the group");
        parse(&spec, &input(&["ex", "--url", "u"]))
            .expect("the default must not conflict with the flag the user typed");
    }

    #[test]
    fn a_group_naming_two_spellings_of_one_flag_is_not_a_conflict() {
        // `-f` and `--file` are one flag. Counted by selector, giving it once would
        // report it as conflicting with itself.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"-f --file <f>\"\nflag \"--url <u>\"\ngroup \"input\" \"-f\" \"--file\" \"--url\"\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--file", "a.txt"])).expect("one flag is one member");
        parse(&spec, &input(&["ex", "-f", "a.txt"])).expect("either spelling, still one member");

        // A genuine collision is still one.
        let err = parse(&spec, &input(&["ex", "--file", "a.txt", "--url", "u"])).unwrap_err();
        assert!(err.to_string().contains("group input"), "{err}");
    }

    #[test]
    fn a_group_reads_the_environment_as_given() {
        // The environment does count, which is the same asymmetry `conflicts` has: an
        // env var is somebody saying something, a default is nobody saying anything.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--file <f>\" env=\"EX_FILE\"\nflag \"--url <u>\"\ngroup \"input\" \"--file\" \"--url\" required=#true\n"
            .parse()
            .unwrap();

        parse_with_env(&spec, &["ex"], &[("EX_FILE", "a.txt")]).expect("the environment fills it");
    }

    #[test]
    fn a_requirement_names_the_flag_that_is_missing() {
        // Reported as the missing flag rather than as something wrong with `--out`,
        // which is what clap says for an unmet `requires` and what a user can act on.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--out <p>\" requires=\"--format\"\nflag \"--format <f>\"\n"
            .parse()
            .unwrap();

        let err = parse(&spec, &input(&["ex", "--out", "a.txt"])).unwrap_err();
        assert!(
            err.to_string().contains("format"),
            "the missing flag should be named: {err}"
        );

        // Satisfied, in either order.
        for words in [
            &["ex", "--out", "a.txt", "--format", "json"][..],
            &["ex", "--format", "json", "--out", "a.txt"][..],
        ] {
            parse(&spec, &input(words)).unwrap_or_else(|e| panic!("{words:?}: {e}"));
        }

        // Nothing happens when the flag that imposes the rule is absent: a requirement
        // is a consequence of using the flag, not a rule about the command line.
        parse(&spec, &input(&["ex"])).expect("a bare invocation requires nothing");
    }

    #[test]
    fn a_default_satisfies_a_requirement() {
        // The flag it names has a value, which is the question a requirement asks. Read
        // any other way, `--format` would be missing here and present ten lines further
        // down, where plain required-ness reads the same default as filling it.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--out <p>\" requires=\"--format\"\nflag \"--format <f>\" default=\"json\"\n"
            .parse()
            .unwrap();

        parse(&spec, &input(&["ex", "--out", "a.txt"]))
            .expect("a defaulted flag is not a missing one");
    }

    #[test]
    fn an_environment_value_satisfies_a_requirement() {
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--out <p>\" requires=\"--format\"\nflag \"--format <f>\" env=\"EX_FORMAT\"\n"
            .parse()
            .unwrap();

        assert!(parse(&spec, &input(&["ex", "--out", "a.txt"])).is_err());
        parse_with_env(&spec, &["ex", "--out", "a.txt"], &[("EX_FORMAT", "json")])
            .expect("the environment supplies it");
    }

    #[test]
    fn a_requirement_is_satisfied_by_a_short_form() {
        // The selector may spell the other flag any way it answers to, so the check
        // resolves it the way every other selector is resolved rather than matching
        // text. The error names the flag, not the selector.
        let spec: Spec =
            "name \"ex\"\nbin \"ex\"\nflag \"--sign\" requires=\"-k\"\nflag \"-k --key <k>\"\n"
                .parse()
                .unwrap();

        parse(&spec, &input(&["ex", "--sign", "--key", "x"])).expect("--key satisfies -k");

        let err = parse(&spec, &input(&["ex", "--sign"])).unwrap_err();
        assert!(err.to_string().contains("key"), "{err}");
    }

    #[test]
    fn conflicting_flags_are_rejected_in_either_order() {
        // Declared once, on `--file`, which is all clap exposes — so the check has to
        // be order-independent by looking at every flag that was given rather than at
        // the one that declared the conflict.
        let spec: Spec =
            "name \"ex\"\nbin \"ex\"\nflag \"--file <f>\" conflicts=\"--stdin\"\nflag \"--stdin\"\n"
                .parse()
                .unwrap();

        for words in [
            &["ex", "--file", "a.txt", "--stdin"][..],
            &["ex", "--stdin", "--file", "a.txt"][..],
        ] {
            let err = parse(&spec, &input(words)).unwrap_err();
            assert!(
                err.to_string().contains("conflicts with --stdin"),
                "{words:?} should be refused: {err}"
            );
        }

        // Either one alone is fine.
        parse(&spec, &input(&["ex", "--stdin"])).unwrap();
        parse(&spec, &input(&["ex", "--file", "a.txt"])).unwrap();
    }

    #[test]
    fn unknown_flags_are_values_by_default() {
        // The default, and the reason it is the default: a spec often parses a
        // command line whose flags belong to something else.
        let spec: Spec = "name \"ex\"\nbin \"ex\"\nflag \"--force\"\narg \"[rest]...\"\n"
            .parse()
            .unwrap();
        let out = parse(
            &spec,
            &["ex".to_string(), "--wat".to_string(), "x".to_string()],
        )
        .unwrap();
        let rest = out.args.keys().find(|a| a.name == "rest").unwrap();
        assert_eq!(out.args[rest].to_string(), "--wat x");
    }

    #[test]
    fn unknown_flags_can_be_rejected_for_the_whole_cli() {
        let spec: Spec =
            "name \"ex\"\nbin \"ex\"\nunknown_flags \"error\"\nflag \"--force\"\narg \"[rest]...\"\n"
                .parse()
                .unwrap();
        let err = parse(&spec, &["ex".to_string(), "--wat".to_string()]).unwrap_err();
        assert!(
            err.to_string().contains("--wat"),
            "the message should name the token: {err}"
        );

        // A negative number is still a value, which is the carve-out oclif had to
        // add back after making this its default.
        let out = parse(&spec, &["ex".to_string(), "-1".to_string()]).unwrap();
        let rest = out.args.keys().find(|a| a.name == "rest").unwrap();
        assert_eq!(out.args[rest].to_string(), "-1");
    }

    #[test]
    fn a_command_may_override_the_cli_wide_setting() {
        // Strict overall, lenient for the one command that forwards options.
        let spec: Spec = r#"
name "ex"
bin "ex"
unknown_flags "error"
cmd "exec" unknown_flags="value" {
  arg "[rest]..."
}
cmd "build" {
  arg "[rest]..."
}
"#
        .parse()
        .unwrap();

        let out = parse(
            &spec,
            &["ex".to_string(), "exec".to_string(), "--wat".to_string()],
        )
        .unwrap();
        let rest = out.args.keys().find(|a| a.name == "rest").unwrap();
        assert_eq!(out.args[rest].to_string(), "--wat");

        assert!(
            parse(
                &spec,
                &["ex".to_string(), "build".to_string(), "--wat".to_string()]
            )
            .is_err(),
            "a command that says nothing inherits the CLI's choice"
        );
    }

    #[test]
    fn the_setting_survives_a_round_trip() {
        let spec: Spec =
            "name \"ex\"\nbin \"ex\"\nunknown_flags \"error\"\ncmd \"x\" unknown_flags=\"value\"\n"
                .parse()
                .unwrap();
        let reparsed: Spec = spec.to_string().parse().unwrap();
        assert_eq!(reparsed.unknown_flags, Some(UnknownFlags::Error));
        assert_eq!(
            reparsed.cmd.subcommands["x"].unknown_flags,
            Some(UnknownFlags::Value)
        );
    }

    #[test]
    fn test_default_subcommand() {
        // Test that default_subcommand routes to the specified subcommand
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test mytask" should be parsed as if it were "test run mytask"
        let input = vec!["test".to_string(), "mytask".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have two commands: root and "run"
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[1].name, "run");

        // Should have parsed the task argument
        assert_eq!(parsed.args.len(), 1);
        let arg = parsed.args.keys().next().unwrap();
        assert_eq!(arg.name, "task");
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "mytask");
    }

    #[test]
    fn test_default_subcommand_explicit_still_works() {
        // Test that explicit subcommand takes precedence
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .build();
        let other_cmd = SpecCommand::builder()
            .name("other")
            .arg(SpecArg::builder().name("other_arg").build())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        cmd.subcommands.insert("other".to_string(), other_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test other foo" should use "other" subcommand, not default
        let input = vec!["test".to_string(), "other".to_string(), "foo".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have used "other" subcommand
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[1].name, "other");
    }

    #[test]
    fn test_default_subcommand_applies_only_at_the_root() {
        // `default_subcommand` is declared once, for the whole spec, and only at the top. It
        // was being looked up wherever the parser happened to be standing, so a command with
        // an unrelated subcommand of the same name acquired a default of its own: with
        // `default_subcommand "ls"`, `ex config zzz` descended into `config ls` and bound
        // `zzz` there. Nothing declared that, and nothing could have.
        let mut config_ls = SpecCommand::builder().name("ls").build();
        config_ls.args.push(SpecArg::builder().name("what").build());
        let mut config_cmd = SpecCommand::builder().name("config").build();
        config_cmd.subcommands.insert("ls".to_string(), config_ls);

        // The root's own `ls`, which is what its default points at. It takes an argument so
        // that a routed word has somewhere to land.
        let mut root_ls = SpecCommand::builder().name("ls").build();
        root_ls.args.push(SpecArg::builder().name("what").build());
        let mut cmd = SpecCommand::builder().name("ex").build();
        cmd.subcommands.insert("ls".to_string(), root_ls);
        cmd.subcommands.insert("config".to_string(), config_cmd);

        let spec = Spec {
            name: "ex".to_string(),
            bin: "ex".to_string(),
            cmd,
            default_subcommand: Some("ls".to_string()),
            ..Default::default()
        };

        // `config` has an `ls`, but `config` did not declare a default, so `zzz` is `config`'s
        // own business — and `config` takes no argument, so this is an error rather than a
        // silent descent.
        let input = vec!["ex".to_string(), "config".to_string(), "zzz".to_string()];
        assert!(
            parse(&spec, &input).is_err(),
            "`config` has no default subcommand and no argument, so `zzz` cannot bind"
        );

        // At the root, where it is declared, it still applies.
        let input = vec!["ex".to_string(), "zzz".to_string()];
        let parsed = parse(&spec, &input).expect("the root's default applies");
        assert_eq!(
            parsed
                .cmds
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["ex", "ls"]
        );
        assert_eq!(
            parsed.args.values().next().map(|v| v.to_string()),
            Some("zzz".to_string()),
            "and the word binds inside the command it reached"
        );
    }

    #[test]
    fn test_default_subcommand_with_nested_subcommands() {
        // Test that default_subcommand works when the default subcommand has nested subcommands.
        // This is the mise use case: "mise say" should be parsed as "mise run say"
        // where "say" is a subcommand of "run" (a task).
        let say_cmd = SpecCommand::builder()
            .name("say")
            .arg(SpecArg::builder().name("name").build())
            .build();
        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("say".to_string(), say_cmd);

        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test say hello" should be parsed as "test run say hello"
        let input = vec!["test".to_string(), "say".to_string(), "hello".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have three commands: root, "run", and "say"
        assert_eq!(parsed.cmds.len(), 3);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");
        assert_eq!(parsed.cmds[2].name, "say");

        // Should have parsed the "name" argument
        assert_eq!(parsed.args.len(), 1);
        let arg = parsed.args.keys().next().unwrap();
        assert_eq!(arg.name, "name");
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "hello");
    }

    /// Build a spec equivalent to the post-mount structure produced by mise's
    /// `mise usage` output: a root with a value-taking global flag (`-C/--cd`), a `run`
    /// subcommand that re-declares the same flag as NON-global, and a mounted task
    /// (`sample:run`) carrying a positional arg with `choices`.
    ///
    /// We construct the merged structure directly instead of executing a real mount so the
    /// test stays hermetic and cross-platform while still exercising the parser defect.
    fn mounted_global_flag_spec() -> Spec {
        let task_cmd = SpecCommand::builder()
            .name("sample:run")
            .arg(
                SpecArg::builder()
                    .name("profile")
                    .choices(["alpha", "beta", "gamma"])
                    .build(),
            )
            .build();
        // `run` re-declares `-C/--cd` but as a NON-global flag, mirroring the mise spec.
        let mut run_cmd = SpecCommand::builder()
            .name("run")
            .flag(
                SpecFlag::builder()
                    .name("cd")
                    .short('C')
                    .long("cd")
                    .arg(SpecArg::builder().name("dir").build())
                    .global(false)
                    .build(),
            )
            .build();
        run_cmd
            .subcommands
            .insert("sample:run".to_string(), task_cmd);

        let mut cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .name("cd")
                    .short('C')
                    .long("cd")
                    .arg(SpecArg::builder().name("dir").build())
                    .global(true)
                    .build(),
            )
            .build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    #[test]
    fn test_prefix_global_flag_does_not_pollute_choices() {
        // Regression for the parser-side root cause referenced by jdx/mise#10069.
        //
        // When `run` re-declares the global `-C/--cd` as non-global, descending into it (and
        // then into the mounted `sample:run`) used to drop the inherited global flag from
        // `available_flags`. Phase 2 then no longer recognized the prefix `-C`, so it was
        // mis-validated against the task's `choices` positional arg.
        let spec = mounted_global_flag_spec();

        // The prefix global flag must stay recognized so it is consumed as a flag (not as the
        // positional). Before the fix this bailed with "Invalid choice for arg profile: -C".
        for words in [
            &["test", "-C", "/tmp", "run", "sample:run"][..],
            // Embedded-value form must behave identically.
            &["test", "--cd=/tmp", "run", "sample:run"][..],
        ] {
            let parsed = parse_partial(&spec, &input(words)).unwrap();
            assert_eq!(
                parsed
                    .cmds
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["test", "run", "sample:run"],
            );
            // No positional arg should have been consumed by the leftover global-flag tokens.
            assert!(
                parsed.args.is_empty(),
                "args should be empty, got {:?}",
                parsed.args
            );

            // Fix (B): the inherited global flag survives the descent even though `run`
            // re-declares `-C/--cd` as non-global.
            let cd = parsed
                .available_flags
                .get("--cd")
                .expect("--cd should remain available after descending into the subcommand");
            assert!(cd.global, "--cd must stay global after descent");
            assert!(
                parsed.available_flags.get("-C").is_some_and(|f| f.global),
                "-C must stay global after descent",
            );

            // The global flag must still be recorded in `out.flags` so it reaches `as_env()`
            // for normal execution and for the env passed to mount scripts. (Removing the
            // token in Phase 1 instead of re-parsing it would silently drop `usage_cd`.)
            assert_eq!(
                parsed.as_env().get("usage_cd").map(String::as_str),
                Some("/tmp"),
                "global flag value must survive in as_env(), got {:?}",
                parsed.as_env(),
            );
        }

        // A real, valid choice still parses through the global flag prefix.
        let parsed = parse_partial(
            &spec,
            &input(&["test", "-C", "/tmp", "run", "sample:run", "alpha"]),
        )
        .unwrap();
        assert_eq!(parsed.args.len(), 1);
        assert_eq!(parsed.args.values().next().unwrap().to_string(), "alpha");

        // And genuinely invalid choices are still rejected (we didn't disable validation).
        assert_parse_err(
            parse_partial(&spec, &input(&["test", "run", "sample:run", "wrong"])),
            "Invalid choice for arg profile: wrong, expected one of alpha, beta, gamma",
        );
    }

    /// Build a spec mirroring mise's orphan-short re-declarations: a root with a LONG-ONLY
    /// global boolean flag (`--raw`, no short), a `run` subcommand that re-declares it as a
    /// NON-global flag while ADDING a short (`-r --raw`) plus a purely-local `-f/--force`
    /// flag, and a mounted task (`sample:run`) with a `choices` positional arg.
    fn mounted_orphan_short_spec() -> Spec {
        let task_cmd = SpecCommand::builder()
            .name("sample:run")
            .arg(
                SpecArg::builder()
                    .name("profile")
                    .choices(["alpha", "beta", "gamma"])
                    .build(),
            )
            .build();
        // `run` re-declares `--raw` as NON-global but adds a `-r` short that exists only here,
        // and also carries a purely-local `-f/--force` flag (shares nothing with a global).
        let mut run_cmd = SpecCommand::builder()
            .name("run")
            .flag(
                SpecFlag::builder()
                    .name("raw")
                    .short('r')
                    .long("raw")
                    .global(false)
                    .build(),
            )
            .flag(
                SpecFlag::builder()
                    .name("force")
                    .short('f')
                    .long("force")
                    .global(false)
                    .build(),
            )
            .build();
        run_cmd
            .subcommands
            .insert("sample:run".to_string(), task_cmd);

        // Root global is LONG-ONLY: `--raw` with no short.
        let mut cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .name("raw")
                    .long("raw")
                    .global(true)
                    .build(),
            )
            .build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    #[test]
    fn test_orphan_short_alias_survives_merge() {
        // Follow-up to test_prefix_global_flag_does_not_pollute_choices (jdx/mise#10069):
        // when `run` re-declares the long-only global `--raw` as a non-global `-r --raw`, the
        // added short `-r` must be unioned onto the surviving inherited global flag instead of
        // being discarded with the wholesale re-declaration. Otherwise `mycli run -r <task>`
        // would not recognize `-r` and would mis-validate it against the task's `choices` arg.
        let spec = mounted_orphan_short_spec();

        let parsed = parse_partial(&spec, &input(&["test", "run", "-r", "sample:run"])).unwrap();
        assert_eq!(
            parsed
                .cmds
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["test", "run", "sample:run"],
        );

        // (a) The orphan short `-r` survives the descent, merged onto the inherited global flag,
        // and the original long `--raw` is still global too.
        assert!(
            parsed.available_flags.get("-r").is_some_and(|f| f.global),
            "-r must be merged onto the inherited global flag and stay global after descent",
        );
        assert!(
            parsed
                .available_flags
                .get("--raw")
                .is_some_and(|f| f.global),
            "--raw must stay global after descent",
        );

        // (b) The token is consumed as a flag, not mistaken for the `choices` positional.
        assert!(
            parsed.args.is_empty(),
            "args should be empty, got {:?}",
            parsed.args
        );

        // (c) The value still reaches as_env() so `usage_raw` is produced for execution/mounts.
        assert_eq!(
            parsed.as_env().get("usage_raw").map(String::as_str),
            Some("true"),
            "merged short's value must survive in as_env(), got {:?}",
            parsed.as_env(),
        );

        // (d) Negative case: a purely-local flag that shares nothing with a global is NOT
        // promoted/merged — it is correctly dropped when descending into the mount.
        assert!(
            !parsed.available_flags.contains_key("-f"),
            "purely-local -f must not be promoted onto a global",
        );
        assert!(
            !parsed.available_flags.contains_key("--force"),
            "purely-local --force must not be promoted onto a global",
        );

        // A real, valid choice still parses through the merged short prefix.
        let parsed =
            parse_partial(&spec, &input(&["test", "run", "-r", "sample:run", "alpha"])).unwrap();
        assert_eq!(parsed.args.len(), 1);
        assert_eq!(parsed.args.values().next().unwrap().to_string(), "alpha");

        // And genuinely invalid choices are still rejected.
        assert_parse_err(
            parse_partial(&spec, &input(&["test", "run", "-r", "sample:run", "wrong"])),
            "Invalid choice for arg profile: wrong, expected one of alpha, beta, gamma",
        );
    }

    #[test]
    fn test_orphan_short_does_not_clobber_unrelated_global() {
        // When a re-declaration's orphan short collides with a DIFFERENT inherited global's
        // short, the merge must not steal it. Here the root has both a long-only `--raw` global
        // and a `-r --restrict` global; `run` re-declares `-r --raw` as non-global. `-r` is a
        // genuine collision with `--restrict`, so global precedence must keep `-r -> restrict`.
        let run_cmd = SpecCommand::builder()
            .name("run")
            .flag(
                SpecFlag::builder()
                    .name("raw")
                    .short('r')
                    .long("raw")
                    .global(false)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .name("raw")
                    .long("raw")
                    .global(true)
                    .build(),
            )
            .flag(
                SpecFlag::builder()
                    .name("restrict")
                    .short('r')
                    .long("restrict")
                    .global(true)
                    .build(),
            )
            .build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        let parsed = parse_partial(&spec, &input(&["test", "run"])).unwrap();
        // `-r` stays owned by the unrelated `--restrict` global, not stolen by the merged raw.
        assert_eq!(
            parsed.available_flags.get("-r").map(|f| f.name.as_str()),
            Some("restrict"),
            "-r must remain owned by the unrelated global it already belonged to",
        );
        // Both globals are still recognized and global after the descent.
        assert!(parsed
            .available_flags
            .get("--raw")
            .is_some_and(|f| f.global));
        assert!(parsed
            .available_flags
            .get("--restrict")
            .is_some_and(|f| f.global));
    }

    #[test]
    fn test_redeclared_global_aliases_share_one_flag() {
        // A global declared with BOTH a short and a long, re-declared non-globally by a
        // subcommand that adds a third alias. Every alias key must resolve to the SAME merged
        // flag: the child's keys iterate in BTreeMap order (`--assume-yes`, `--yes`, `-y`), so by
        // the time `-y` is reached the long already points at the merged flag. That merged flag is
        // not a *different* inherited global, so the collision guard must not skip `-y` and leave
        // it pointing at the pre-merge global (which lacks the added `assume-yes` alias).
        let spec = r#"
flag "-y --yes" global=#true effect="write"
cmd "run" {
    flag "-y --yes --assume-yes"
}
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse_partial(&spec, &input(&["test", "run"])).unwrap();

        for key in ["-y", "--yes", "--assume-yes"] {
            let flag = parsed
                .available_flags
                .get(key)
                .unwrap_or_else(|| panic!("{key} must be recognized after the descent"));
            assert!(flag.global, "{key} must stay global after the descent");
            assert_eq!(
                flag.long,
                vec!["yes".to_string(), "assume-yes".to_string()],
                "{key} must resolve to the flag carrying every alias",
            );
            assert_eq!(flag.short, vec!['y'], "{key} must keep the global's short");
        }

        // One logical flag means one object: all three keys share a single `Arc`.
        assert_eq!(
            unique_flags(parsed.available_flags.values()).count(),
            1,
            "all aliases must point at one flag object, got {:?}",
            parsed.available_flags,
        );

        // The global's effect survives the merge, so `-y` still marks the command as writing.
        assert_eq!(
            parsed.available_flags["-y"].effect,
            Some(crate::SpecCommandEffect::Write),
        );
    }

    #[test]
    fn test_partially_redeclared_global_keeps_all_aliases_on_one_flag() {
        // Same one-flag-one-object requirement as above, but the child re-declares only ONE of
        // the global's three aliases (`--yes`, not `-y`/`--confirm`) while adding a new one. The
        // aliases the child omits are never visited by the merge loop, so they must be rebound to
        // the merged flag explicitly — otherwise `-y` and `--confirm` keep pointing at the
        // pre-merge global and miss the added `assume-yes`.
        let spec = r#"
flag "-y --yes --confirm" global=#true
cmd "run" {
    flag "--yes --assume-yes"
}
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse_partial(&spec, &input(&["test", "run"])).unwrap();

        for key in ["-y", "--yes", "--confirm", "--assume-yes"] {
            let flag = parsed
                .available_flags
                .get(key)
                .unwrap_or_else(|| panic!("{key} must be recognized after the descent"));
            assert!(flag.global, "{key} must stay global after the descent");
            assert_eq!(
                flag.long,
                vec![
                    "yes".to_string(),
                    "confirm".to_string(),
                    "assume-yes".to_string()
                ],
                "{key} must resolve to the flag carrying every alias",
            );
        }

        assert_eq!(
            unique_flags(parsed.available_flags.values()).count(),
            1,
            "all aliases must point at one flag object, got {:?}",
            parsed.available_flags,
        );
    }

    /// Build a spec shaped like mise's post-mount structure for jdx/mise#11282: a root with
    /// globals (`-E/--env <ENV>`, `--silent`), a `run` subcommand with a non-global flag, and a
    /// MOUNTED task command that declares its own `--env` (with choices) plus `--bump`.
    ///
    /// The task command is marked `mounted` the same way `SpecCommand::mount()` marks the
    /// commands it merges in, so the test stays hermetic (no mount subprocess).
    fn mounted_task_flag_spec() -> Spec {
        let mut task_cmd = SpecCommand::builder()
            .name("mytask")
            .flag(
                SpecFlag::builder()
                    .name("env")
                    .long("env")
                    .arg(
                        SpecArg::builder()
                            .name("name")
                            .choices(["dev", "stage", "prod"])
                            .build(),
                    )
                    .global(false)
                    .build(),
            )
            .flag(
                SpecFlag::builder()
                    .name("bump")
                    .long("bump")
                    .arg(
                        SpecArg::builder()
                            .name("type")
                            .choices(["auto", "major"])
                            .build(),
                    )
                    .global(false)
                    .build(),
            )
            .build();
        task_cmd.mounted = true;

        let mut run_cmd = SpecCommand::builder()
            .name("run")
            .flag(
                SpecFlag::builder()
                    .name("force")
                    .short('f')
                    .long("force")
                    .global(false)
                    .build(),
            )
            .build();
        run_cmd.subcommands.insert("mytask".to_string(), task_cmd);

        let mut cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .name("env")
                    .short('E')
                    .long("env")
                    .arg(SpecArg::builder().name("ENV").build())
                    .global(true)
                    .build(),
            )
            .flag(
                SpecFlag::builder()
                    .name("silent")
                    .long("silent")
                    .global(true)
                    .build(),
            )
            .build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    #[test]
    fn test_mount_boundary_does_not_apply_inside_the_mounted_tree() {
        // The mounted program's own commands are ordinary commands relative to each other, so
        // descending *within* the mounted tree must follow the normal rules — including keeping
        // an inherited global that a nested command re-declares as non-global (jdx/usage#649).
        // Treating every level of the tree as a mount boundary let the re-declaration shadow the
        // global, which the next descent's `retain(global)` then dropped entirely.
        let deep = SpecCommand::builder().name("deep").build();
        let mut sub = SpecCommand::builder()
            .name("sub")
            // Re-declares the mounted program's own global as non-global.
            .flag(
                SpecFlag::builder()
                    .name("cd")
                    .short('C')
                    .long("cd")
                    .arg(SpecArg::builder().name("dir").build())
                    .global(false)
                    .build(),
            )
            .build();
        sub.subcommands.insert("deep".to_string(), deep);
        let mut task = SpecCommand::builder()
            .name("task")
            .flag(
                SpecFlag::builder()
                    .name("cd")
                    .short('C')
                    .long("cd")
                    .arg(SpecArg::builder().name("dir").build())
                    .global(true)
                    .build(),
            )
            .build();
        task.subcommands.insert("sub".to_string(), sub);
        task.mark_mounted();

        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("task".to_string(), task);
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        let parsed = parse_partial(&spec, &input(&["test", "run", "task", "sub", "deep"])).unwrap();
        assert!(
            parsed.available_flags.get("--cd").is_some_and(|f| f.global),
            "the mounted program's own global must survive descents inside the mounted tree",
        );
        assert!(
            parsed.completion_flags().contains_key("--cd"),
            "and must still be offered there: it belongs to the mounted program",
        );
        assert!(
            parsed.completion_flags().contains_key("-C"),
            "including the short the nested command re-declared",
        );
    }

    #[test]
    fn test_mount_flags_merged_into_the_mounting_cmd_are_offered() {
        // A mounted spec may declare flags on its own root, which `SpecCommand::merge` folds
        // into the command the mount sits on. They belong to the mounted program, so they must
        // be offered inside the mounted commands rather than filtered out with the mounting
        // CLI's own flags.
        let mut task = SpecCommand::builder()
            .name("task")
            .flag(
                SpecFlag::builder()
                    .name("bump")
                    .long("bump")
                    .global(false)
                    .build(),
            )
            .build();
        task.mark_mounted();

        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("task".to_string(), task);
        // What `mount()` leaves behind when the mounted spec's root declares flags.
        run_cmd.flags = vec![
            SpecFlag::builder()
                .name("tglobal")
                .long("tglobal")
                .global(true)
                .build(),
            SpecFlag::builder()
                .name("tlocal")
                .long("tlocal")
                .global(false)
                .build(),
        ];
        run_cmd.flags_from_mount = true;

        let mut cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .name("silent")
                    .long("silent")
                    .global(true)
                    .build(),
            )
            .build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        let parsed = parse_partial(&spec, &input(&["test", "run", "task"])).unwrap();
        assert_eq!(
            parsed.completion_flags().keys().collect::<Vec<_>>(),
            vec!["--bump", "--tglobal"],
            "the mounted spec's root global belongs to the mounted program; the mounting CLI's \
             `--silent` does not, and the mount's non-global root flag is not inherited",
        );
    }

    #[test]
    fn test_mounted_cmd_does_not_offer_mounting_cli_globals() {
        // Regression for jdx/mise#11282. A mounted command describes another program, which
        // does not accept the mounting CLI's globals (mise forwards everything after a task
        // name to the task). They must stay recognized — they may appear before the mounted
        // command — but must not be offered in completions there.
        let spec = mounted_task_flag_spec();
        let parsed = parse_partial(&spec, &input(&["test", "run", "mytask"])).unwrap();

        // Still recognized for parsing...
        assert!(parsed.available_flags.contains_key("--silent"));
        assert!(parsed.available_flags.contains_key("-E"));
        // ...but belonging to a command above the mount, so not offered.
        assert_eq!(
            parsed.completion_flags().keys().collect::<Vec<_>>(),
            vec!["--bump", "--env"],
            "only the mounted command's own flags may be offered",
        );

        // `run`'s own non-global flag is dropped on descent, as it always was.
        assert!(!parsed.available_flags.contains_key("--force"));
    }

    #[test]
    fn test_mounted_cmd_flag_wins_over_inherited_global() {
        // Second half of jdx/mise#11282: the mounted `--env` (with choices) used to be shadowed
        // by the root's `--env` global, so completing its value fell back to file completion.
        let spec = mounted_task_flag_spec();
        let parsed = parse_partial(&spec, &input(&["test", "run", "mytask", "--env"])).unwrap();

        let awaiting = parsed
            .flag_awaiting_value
            .first()
            .expect("--env should await a value");
        assert_eq!(
            awaiting
                .arg
                .as_ref()
                .and_then(|a| a.choices.as_ref())
                .map(|c| c.choices.clone()),
            Some(vec![
                "dev".to_string(),
                "stage".to_string(),
                "prod".to_string()
            ]),
            "the mounted command's own --env must win over the inherited global",
        );

        // The global's short is not declared by the mounted command, so it keeps pointing at
        // the global and a value passed before the mounted command still parses.
        let parsed =
            parse_partial(&spec, &input(&["test", "-E", "anything", "run", "mytask"])).unwrap();
        assert!(
            parsed.args.is_empty(),
            "prefix global tokens must not be consumed as positionals, got {:?}",
            parsed.args
        );
        assert_eq!(
            parsed.as_env().get("usage_env").map(String::as_str),
            Some("anything"),
        );
    }

    #[test]
    fn test_prefix_flag_keeps_the_flag_it_was_read_as() {
        // A word before the mounted command is re-parsed by Phase 2, when the mounted command
        // already owns the name. It has to stay bound to the flag Phase 1 read it as, or the
        // global's value would be validated against the mounted flag's choices and a legitimate
        // value would be rejected.
        let spec = mounted_task_flag_spec();
        let parsed = parse_partial(
            &spec,
            &input(&["test", "--env", "not-a-task-choice", "run", "mytask"]),
        )
        .unwrap();
        assert!(
            parsed.errors.is_empty(),
            "prefix global value must not be validated against the mounted flag: {:?}",
            parsed
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            parsed.as_env().get("usage_env").map(String::as_str),
            Some("not-a-task-choice"),
        );

        // The embedded-value form binds the same way.
        let parsed = parse_partial(
            &spec,
            &input(&["test", "--env=not-a-task-choice", "run", "mytask"]),
        )
        .unwrap();
        assert!(parsed.errors.is_empty());
        assert_eq!(
            parsed.as_env().get("usage_env").map(String::as_str),
            Some("not-a-task-choice"),
        );

        // Meanwhile a word *after* the mounted command belongs to the mounted flag, even when
        // the same name was already used before it.
        let parsed = parse_partial(
            &spec,
            &input(&["test", "--env", "prod", "run", "mytask", "--env"]),
        )
        .unwrap();
        let awaiting = parsed
            .flag_awaiting_value
            .first()
            .expect("--env should await a value");
        assert_eq!(
            awaiting
                .arg
                .as_ref()
                .and_then(|a| a.choices.as_ref())
                .map(|c| c.choices.clone()),
            Some(vec![
                "dev".to_string(),
                "stage".to_string(),
                "prod".to_string()
            ]),
            "the mounted command's --env must own the name after the mounted command",
        );
    }

    #[test]
    fn test_non_global_flag_does_not_hide_subcommand() {
        // A non-global flag may precede a subcommand (`mycli run --force task`). Phase 1 used to
        // stop scanning at one, so the subcommand — and any mount on it — was never reached and
        // its name was left to Phase 2 to mis-read as a positional: `unexpected word: mytask`.
        let spec = mounted_task_flag_spec();

        for words in [
            // `run` declares `-f/--force` as non-global.
            &["test", "run", "--force", "mytask"][..],
            &["test", "run", "-f", "mytask"][..],
            // Mixed with a global before the subcommand.
            &["test", "-E", "prod", "run", "--force", "mytask"][..],
        ] {
            let parsed = parse_partial(&spec, &input(words)).unwrap();
            assert_eq!(
                parsed
                    .cmds
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>(),
                vec!["test", "run", "mytask"],
                "{words:?} should descend into the mounted command",
            );
            assert!(
                parsed.args.is_empty(),
                "{words:?} should not consume a positional, got {:?}",
                parsed.args,
            );
            assert_eq!(
                parsed.as_env().get("usage_force").map(String::as_str),
                Some("true"),
                "the non-global flag must still be recorded for {words:?}",
            );
        }

        // A non-global flag that takes a value consumes it, rather than reading the value as the
        // subcommand.
        let mut run_cmd = SpecCommand::builder()
            .name("run")
            .flag(
                SpecFlag::builder()
                    .name("output")
                    .short('o')
                    .long("output")
                    .arg(SpecArg::builder().name("mode").build())
                    .global(false)
                    .build(),
            )
            .build();
        run_cmd.subcommands.insert(
            "task".to_string(),
            SpecCommand::builder().name("task").build(),
        );
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        let parsed =
            parse_partial(&spec, &input(&["test", "run", "--output", "quiet", "task"])).unwrap();
        assert_eq!(
            parsed
                .cmds
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            vec!["test", "run", "task"],
        );
        assert_eq!(
            parsed.as_env().get("usage_output").map(String::as_str),
            Some("quiet"),
        );

        // An unknown flag still stops the scan: it may take a value, so the next word cannot be
        // assumed to be a subcommand. `run` takes no positional, so this stays an error.
        assert_parse_err(
            parse_partial(&spec, &input(&["test", "run", "--nope", "task"])),
            "unexpected word: --nope",
        );
    }

    #[test]
    fn test_non_mounted_subcommand_offers_inherited_globals() {
        // Nothing changes for ordinary (non-mounted) subcommands: a global declared above is
        // still both recognized and offered.
        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert(
            "nested".to_string(),
            SpecCommand::builder().name("nested").build(),
        );
        let mut cmd = SpecCommand::builder()
            .name("test")
            .flag(
                SpecFlag::builder()
                    .name("silent")
                    .long("silent")
                    .global(true)
                    .build(),
            )
            .build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        let parsed = parse_partial(&spec, &input(&["test", "run", "nested"])).unwrap();
        assert_eq!(
            parsed.completion_flags().keys().collect::<Vec<_>>(),
            parsed.available_flags.keys().collect::<Vec<_>>(),
        );
        assert!(parsed.completion_flags().contains_key("--silent"));
    }

    #[test]
    fn test_subcommand_alias_collision_keeps_last_owner() {
        // The orphan-alias merge must not disturb how two flags in the SAME subcommand that
        // share an alias are resolved. Historically the flattened flag map gave the shared
        // alias to the LAST-declared flag (last-writer-wins); that must be preserved.
        let run_cmd = SpecCommand::builder()
            .name("run")
            .flag(
                SpecFlag::builder()
                    .name("alpha")
                    .short('x')
                    .long("alpha")
                    .global(false)
                    .build(),
            )
            .flag(
                SpecFlag::builder()
                    .name("beta")
                    .short('x')
                    .long("beta")
                    .global(false)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        let parsed = parse_partial(&spec, &input(&["test", "run"])).unwrap();
        // `-x` is declared by both flags; the last one (`beta`) keeps it, as before the fix.
        assert_eq!(
            parsed.available_flags.get("-x").map(|f| f.name.as_str()),
            Some("beta"),
            "the last-declared flag must keep a shared short alias",
        );
        // Both distinct long aliases remain recognized and point to their own flag.
        assert_eq!(
            parsed
                .available_flags
                .get("--alpha")
                .map(|f| f.name.as_str()),
            Some("alpha"),
        );
        assert_eq!(
            parsed
                .available_flags
                .get("--beta")
                .map(|f| f.name.as_str()),
            Some("beta"),
        );
    }

    #[test]
    fn test_default_subcommand_same_name_child() {
        // Test that default_subcommand doesn't cause issues when the default subcommand
        // has a child with the same name (e.g., "run" has a task named "run").
        // This verifies we don't switch multiple times or get stuck in a loop.
        let run_task = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("args").build())
            .build();
        let mut run_cmd = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("run".to_string(), run_task);

        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        // "test run" explicitly matches the "run" subcommand (not via default_subcommand)
        let input = vec!["test".to_string(), "run".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have two commands: root and "run"
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");

        // "test run run" should descend into the "run" task (child of "run" subcommand)
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "run".to_string(),
            "hello".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        assert_eq!(parsed.cmds.len(), 3);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");
        assert_eq!(parsed.cmds[2].name, "run");
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "hello");

        // Key test case: "test other" should switch to default subcommand "run"
        // and treat "other" as a positional arg (not try to switch again because
        // "run" also has a "run" child).
        let mut run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .build();
        let run_task = SpecCommand::builder().name("run").build();
        run_cmd.subcommands.insert("run".to_string(), run_task);

        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            default_subcommand: Some("run".to_string()),
            ..Default::default()
        };

        let input = vec!["test".to_string(), "other".to_string()];
        let parsed = parse(&spec, &input).unwrap();

        // Should have two commands: root and "run" (the default)
        // We should NOT have switched again to the "run" task child
        assert_eq!(parsed.cmds.len(), 2);
        assert_eq!(parsed.cmds[0].name, "test");
        assert_eq!(parsed.cmds[1].name, "run");

        // "other" should be parsed as a positional arg
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "other");
    }

    #[test]
    fn test_restart_token() {
        // Test that restart_token resets argument parsing
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 ::: task2" - should end up with task2 as the arg
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            ":::".to_string(),
            "task2".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // After restart, args were cleared and task2 was parsed
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "task2");
    }

    #[test]
    fn test_restart_token_multiple() {
        // Test multiple restart tokens
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 ::: task2 ::: task3" - should end up with task3 as the arg
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            ":::".to_string(),
            "task2".to_string(),
            ":::".to_string(),
            "task3".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // After multiple restarts, args were cleared and task3 was parsed
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "task3");
    }

    #[test]
    fn test_restart_token_clears_flag_awaiting_value() {
        // Test that restart_token clears pending flag values
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .flag(
                SpecFlag::builder()
                    .name("jobs")
                    .long("jobs")
                    .arg(SpecArg::builder().name("count").build())
                    .build(),
            )
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 --jobs ::: task2" - task2 should be an arg, not a flag value
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            "--jobs".to_string(),
            ":::".to_string(),
            "task2".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // task2 should be parsed as the task arg, not as --jobs value
        assert_eq!(parsed.args.len(), 1);
        let value = parsed.args.values().next().unwrap();
        assert_eq!(value.to_string(), "task2");
        // --jobs should not have a value
        assert!(parsed.flag_awaiting_value.is_empty());
    }

    #[test]
    fn test_restart_token_resets_double_dash() {
        // Test that restart_token resets the -- separator effect
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .arg(SpecArg::builder().name("extra_args").var(true).build())
            .flag(SpecFlag::builder().name("verbose").long("verbose").build())
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run task1 -- extra ::: --verbose task2" - --verbose should be a flag after :::
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            "--".to_string(),
            "extra".to_string(),
            ":::".to_string(),
            "--verbose".to_string(),
            "task2".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        // --verbose should be parsed as a flag (not an arg) after the restart
        assert!(parsed.flags.keys().any(|f| f.name == "verbose"));
        // task2 should be the arg after restart
        let task_arg = parsed.args.keys().find(|a| a.name == "task").unwrap();
        let value = parsed.args.get(task_arg).unwrap();
        assert_eq!(value.to_string(), "task2");
    }

    #[test]
    fn test_double_dashes_without_preserve() {
        // Only the first `--` is a separator; a later one is a value, because flag
        // parsing has already stopped and there is nothing left for it to do.
        // `preserve` is about the *first* one — see the test below, where none is
        // consumed at all.
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("args").var(true).build())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run arg1 -- arg2 -- arg3": the first separates, the second is a value
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "arg1".to_string(),
            "--".to_string(),
            "arg2".to_string(),
            "--".to_string(),
            "arg3".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let args_arg = parsed.args.keys().find(|a| a.name == "args").unwrap();
        let value = parsed.args.get(args_arg).unwrap();
        assert_eq!(value.to_string(), "arg1 arg2 -- arg3");
    }

    #[test]
    fn test_double_dashes_with_preserve() {
        // Test that variadic args WITH `preserve` keep all double dashes
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(
                SpecArg::builder()
                    .name("args")
                    .var(true)
                    .double_dash(SpecDoubleDashChoices::Preserve)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run arg1 -- arg2 -- arg3" - all double dashes should be preserved
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "arg1".to_string(),
            "--".to_string(),
            "arg2".to_string(),
            "--".to_string(),
            "arg3".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let args_arg = parsed.args.keys().find(|a| a.name == "args").unwrap();
        let value = parsed.args.get(args_arg).unwrap();
        assert_eq!(value.to_string(), "arg1 -- arg2 -- arg3");
    }

    #[test]
    fn test_double_dashes_with_preserve_only_dashes() {
        // Test that variadic args WITH `preserve` keep all double dashes even
        // if the values are just double dashes
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(
                SpecArg::builder()
                    .name("args")
                    .var(true)
                    .double_dash(SpecDoubleDashChoices::Preserve)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // "test run -- --" - all double dashes should be preserved
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "--".to_string(),
            "--".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let args_arg = parsed.args.keys().find(|a| a.name == "args").unwrap();
        let value = parsed.args.get(args_arg).unwrap();
        assert_eq!(value.to_string(), "-- --");
    }

    #[test]
    fn test_double_dashes_with_preserve_multiple_args() {
        // Test with multiple args where only the second has has `preserve`
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .arg(
                SpecArg::builder()
                    .name("extra_args")
                    .var(true)
                    .double_dash(SpecDoubleDashChoices::Preserve)
                    .build(),
            )
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);

        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // The first arg "task1" is captured normally
        // Then extra_args with `preserve` captures everything, including the "--" tokens
        let input = vec![
            "test".to_string(),
            "run".to_string(),
            "task1".to_string(),
            "--".to_string(),
            "arg1".to_string(),
            "--".to_string(),
            "--foo".to_string(),
        ];
        let parsed = parse(&spec, &input).unwrap();

        let task_arg = parsed.args.keys().find(|a| a.name == "task").unwrap();
        let task_value = parsed.args.get(task_arg).unwrap();
        assert_eq!(task_value.to_string(), "task1");

        let extra_arg = parsed.args.keys().find(|a| a.name == "extra_args").unwrap();
        let extra_value = parsed.args.get(extra_arg).unwrap();
        assert_eq!(extra_value.to_string(), "-- arg1 -- --foo");
    }

    fn spec_with_args(args: impl IntoIterator<Item = SpecArg>) -> Spec {
        let cmd = SpecCommand::builder().name("test").args(args).build();
        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    fn arg_value(parsed: &ParseOutput, name: &str) -> String {
        let arg = parsed
            .args
            .keys()
            .find(|a| a.name == name)
            .unwrap_or_else(|| panic!("expected arg {name} to be parsed"));
        parsed.args.get(arg).unwrap().to_string()
    }

    fn required_arg(name: &str) -> SpecArg {
        SpecArg::builder()
            .name(name)
            .var(true)
            .required(false)
            .double_dash(SpecDoubleDashChoices::Required)
            .build()
    }

    #[test]
    fn test_double_dash_required_reports_error_once_for_variadic() {
        // A variadic arg is offered every remaining word, but the mistake is one mistake.
        let spec = spec_with_args([required_arg("files")]);

        let parsed = parse_partial(&spec, &input(&["test", "a", "b", "c"])).unwrap();

        assert!(parsed.args.is_empty());
        assert_eq!(parsed.errors.len(), 1);
        assert!(
            matches!(&parsed.errors[0], UsageErr::ArgRequiresDoubleDash(name) if name == "files")
        );
    }

    #[test]
    fn test_double_dash_required_suppresses_missing_arg() {
        // The arg is never filled, so the end-of-parse check would also call it missing.
        let spec = spec_with_args([SpecArg::builder()
            .name("file")
            .required(true)
            .double_dash(SpecDoubleDashChoices::Required)
            .build()]);

        let parsed = parse_partial(&spec, &input(&["test", "x"])).unwrap();

        assert_eq!(parsed.errors.len(), 1);
        assert!(matches!(
            &parsed.errors[0],
            UsageErr::ArgRequiresDoubleDash(_)
        ));
        // The cursor stays put, so a completion keeps offering the same arg.
        assert_eq!(
            parsed.next_arg.as_ref().map(|a| a.name.as_str()),
            Some("file")
        );
        assert!(!parsed.double_dash_seen);
    }

    #[test]
    fn test_double_dash_routes_to_required_arg() {
        // Everything after `--` belongs to the arg that requires it, even though the greedy
        // variadic before it would otherwise swallow the rest (clap's `Arg::last(true)`).
        let spec = spec_with_args([
            SpecArg::builder()
                .name("tool")
                .var(true)
                .required(false)
                .build(),
            required_arg("command"),
        ]);

        let parsed = parse(&spec, &input(&["test", "node@20", "--", "node", "app.js"])).unwrap();

        assert_eq!(arg_value(&parsed, "tool"), "node@20");
        assert_eq!(arg_value(&parsed, "command"), "node app.js");
        assert!(parsed.double_dash_seen);
    }

    #[test]
    fn test_double_dash_routes_with_gap_reports_missing_arg() {
        // Jumping the cursor leaves `tool` empty even though `command` is filled, so the
        // "is it filled?" check cannot be a count of how many args were filled.
        let spec = spec_with_args([
            SpecArg::builder()
                .name("tool")
                .var(true)
                .required(true)
                .build(),
            required_arg("command"),
        ]);

        let parsed = parse_partial(&spec, &input(&["test", "--", "ls"])).unwrap();

        assert_eq!(arg_value(&parsed, "command"), "ls");
        assert!(parsed.args.keys().all(|a| a.name != "tool"));
        assert!(parsed
            .errors
            .iter()
            .any(|e| matches!(e, UsageErr::MissingArg(name) if name == "tool")));
    }

    #[test]
    fn test_double_dash_gap_applies_defaults() {
        // Same gap, seen from `Parser::parse`: the skipped arg still gets its default.
        let spec = spec_with_args([
            SpecArg::builder()
                .name("tool")
                .var(true)
                .required(false)
                .default_value("node@20")
                .build(),
            required_arg("command"),
        ]);

        let parsed = parse(&spec, &input(&["test", "--", "ls"])).unwrap();

        assert_eq!(arg_value(&parsed, "command"), "ls");
        assert_eq!(arg_value(&parsed, "tool"), "node@20");
    }

    fn spec_with_restart_token_and_required_arg() -> Spec {
        let run_cmd = SpecCommand::builder()
            .name("run")
            .arg(SpecArg::builder().name("task").build())
            .arg(required_arg("run_args"))
            .restart_token(":::".to_string())
            .build();
        let mut cmd = SpecCommand::builder().name("test").build();
        cmd.subcommands.insert("run".to_string(), run_cmd);
        Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        }
    }

    #[test]
    fn test_double_dash_required_restart_token_resets_separator() {
        // The `--` before `:::` belongs to the previous invocation only.
        let spec = spec_with_restart_token_and_required_arg();

        let parsed = parse_partial(
            &spec,
            &input(&["test", "run", "task1", "--", "a", ":::", "task2", "b"]),
        )
        .unwrap();

        assert_eq!(arg_value(&parsed, "task"), "task2");
        assert!(parsed.args.keys().all(|a| a.name != "run_args"));
        // Reported once even though the arg was violated after already succeeding once.
        assert_eq!(
            parsed
                .errors
                .iter()
                .filter(|e| matches!(e, UsageErr::ArgRequiresDoubleDash(_)))
                .count(),
            1
        );
    }

    #[test]
    fn test_double_dash_required_restart_token_accepts_new_separator() {
        let spec = spec_with_restart_token_and_required_arg();

        let parsed = parse(
            &spec,
            &input(&["test", "run", "task1", "--", "a", ":::", "task2", "--", "c"]),
        )
        .unwrap();

        assert_eq!(arg_value(&parsed, "task"), "task2");
        assert_eq!(arg_value(&parsed, "run_args"), "c");
    }

    #[test]
    fn test_double_dash_preserve_is_not_a_separator() {
        // A `--` that `preserve` keeps is a *value* of that arg, so it must not unlock the
        // arg that requires a separator. Deliberate: one token cannot be both.
        let spec = spec_with_args([
            SpecArg::builder()
                .name("kept")
                .var(true)
                .var_max(1)
                .required(false)
                .double_dash(SpecDoubleDashChoices::Preserve)
                .build(),
            required_arg("rest"),
        ]);

        let parsed = parse_partial(&spec, &input(&["test", "--", "x"])).unwrap();

        assert_eq!(arg_value(&parsed, "kept"), "--");
        assert!(parsed.args.keys().all(|a| a.name != "rest"));
        assert!(!parsed.double_dash_seen);
        assert_eq!(parsed.errors.len(), 1);
    }

    #[test]
    fn test_double_dash_required_does_not_bail_in_parse_partial() {
        // Completions parse half-typed command lines; they must still get a result.
        let spec = spec_with_args([required_arg("file")]);

        assert!(parse_partial(&spec, &input(&["test", "x"])).is_ok());
        assert!(parse(&spec, &input(&["test", "x"])).is_err());
    }

    #[test]
    fn test_double_dash_without_required_arg_does_not_move_cursor() {
        // Specs with no `double_dash="required"` arg are untouched by the jump.
        let spec = spec_with_args([
            SpecArg::builder().name("first").required(false).build(),
            SpecArg::builder().name("second").required(false).build(),
        ]);

        let parsed = parse(&spec, &input(&["test", "--", "a", "b"])).unwrap();

        assert_eq!(arg_value(&parsed, "first"), "a");
        assert_eq!(arg_value(&parsed, "second"), "b");
        assert!(parsed.next_arg.is_none());
    }

    #[test]
    fn test_parser_with_custom_env_for_required_arg() {
        let spec = spec_with_arg(
            SpecArg::builder()
                .name("name")
                .env("NAME")
                .required(true)
                .build(),
        );
        std::env::remove_var("NAME");

        let parsed = parse_with_env(&spec, &["test"], &[("NAME", "john")])
            .expect("parse should succeed with custom env");
        assert_eq!(parsed.args.len(), 1);
        assert_eq!(first_string_value(&parsed), "john");
    }

    #[test]
    fn test_parser_with_custom_env_for_required_flag() {
        let spec = spec_with_flag(
            SpecFlag::builder()
                .long("name")
                .env("NAME")
                .required(true)
                .arg(SpecArg::builder().name("name").build())
                .build(),
        );
        std::env::remove_var("NAME");

        let parsed = parse_with_env(&spec, &["test"], &[("NAME", "jane")])
            .expect("parse should succeed with custom env");
        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(first_string_value(&parsed), "jane");
    }

    #[test]
    fn test_parser_with_custom_env_still_fails_when_missing() {
        let spec = spec_with_arg(
            SpecArg::builder()
                .name("name")
                .env("NAME")
                .required(true)
                .build(),
        );
        std::env::remove_var("NAME");
        assert!(parse_with_env(&spec, &["test"], &[]).is_err());
    }

    #[test]
    fn test_parser_does_not_treat_env_choice_value_as_help() {
        let spec = spec_with_arg(
            SpecArg::builder()
                .name("env")
                .env("CURRENT_ENV")
                .choices(["dev", "staging"])
                .required(false)
                .build(),
        );

        assert_parse_err(
            parse_with_env(&spec, &["test"], &[("CURRENT_ENV", "--help")]),
            "Invalid choice for arg env: --help, expected one of dev, staging",
        );
    }

    #[test]
    fn test_parser_does_not_treat_default_choice_value_as_help() {
        let spec = spec_with_flag(
            SpecFlag::builder()
                .long("env")
                .arg(
                    SpecArg::builder()
                        .name("env")
                        .choices(["dev", "staging"])
                        .build(),
                )
                .default_value("--help")
                .build(),
        );

        assert_parse_err(
            parse_with_env(&spec, &["test"], &[]),
            "Invalid choice for option env: --help, expected one of dev, staging",
        );
    }

    #[cfg(feature = "unstable_choices_env")]
    /// argv as `parse` wants it, program name included.
    fn words(of: &[&str]) -> Vec<String> {
        of.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_command_that_needs_a_subcommand_says_so() {
        // The spec has carried `subcommand_required` since the derive needed it, and this parser
        // never read it — so `mise generate`, which declares it, parsed as a complete
        // invocation while usage-argv and clap both refused. Found by the differential fuzzer.
        let spec: Spec = r#"
name "ex"
bin "ex"
cmd "gen" subcommand_required=#true {
    cmd "two" {}
    cmd "one" {}
    cmd "secret" hide=#true {}
    alias "g"
}
cmd "open" {
    cmd "sub" {}
}
"#
        .parse()
        .unwrap();

        let err = parse(&spec, &words(&["ex", "gen"])).unwrap_err();
        // Sorted, so the message does not depend on map order; hidden commands left out,
        // because a message telling someone to type a hidden name is worse than a vague one;
        // and the alias not listed beside the name it points at.
        assert_eq!(err.to_string(), "`gen` needs a subcommand: one of one, two");

        // Reached through its alias, and still about the command rather than the spelling.
        let err = parse(&spec, &words(&["ex", "g"])).unwrap_err();
        assert!(err.to_string().starts_with("`gen` needs a subcommand"));

        // Given one: fine.
        parse(&spec, &words(&["ex", "gen", "one"])).unwrap();

        // And a command that has subcommands without declaring them required is untouched —
        // this is the half that keeps the check from being "any command with children".
        parse(&spec, &words(&["ex", "open"])).unwrap();
        parse(&spec, &words(&["ex", "open", "sub"])).unwrap();
    }

    #[test]
    fn test_parser_arg_choices_from_custom_env() {
        let spec = spec_arg_choices_env("DEPLOY_ENVS");

        let parsed =
            parse_with_env(&spec, &["test", "bar"], &[("DEPLOY_ENVS", "foo,bar baz")]).unwrap();
        assert_eq!(first_string_value(&parsed), "bar");

        assert_parse_err(
            parse_with_env(&spec, &["test", "prod"], &[("DEPLOY_ENVS", "foo,bar baz")]),
            "Invalid choice for arg env: prod, expected one of foo, bar, baz",
        );
        assert_parse_err(
            parse_with_env(&spec, &["test", "prod"], &[]),
            "Invalid choice for arg env: prod, no choices resolved from env DEPLOY_ENVS",
        );
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn test_parser_validates_flag_choices_from_custom_env() {
        let spec = spec_flag_choices_env("DEPLOY_ENVS");
        let parsed = parse_with_env(
            &spec,
            &["test", "--env", "baz"],
            &[("DEPLOY_ENVS", "foo,bar baz")],
        )
        .unwrap();
        assert_eq!(first_string_value(&parsed), "baz");
    }

    #[cfg(feature = "unstable_choices_env")]
    #[test]
    fn test_parser_revalidates_env_and_default_values_against_choices_env() {
        let arg_env_spec = spec_with_arg(
            SpecArg::builder()
                .name("env")
                .env("CURRENT_ENV")
                .choices_env("DEPLOY_ENVS")
                .build(),
        );
        assert_parse_err(
            parse_with_env(
                &arg_env_spec,
                &["test"],
                &[("CURRENT_ENV", "prod"), ("DEPLOY_ENVS", "dev,staging")],
            ),
            "Invalid choice for arg env: prod, expected one of dev, staging",
        );

        let flag_default_spec = spec_with_flag(
            SpecFlag::builder()
                .long("env")
                .arg(
                    SpecArg::builder()
                        .name("env")
                        .choices_env("DEPLOY_ENVS")
                        .build(),
                )
                .default_value("prod")
                .build(),
        );
        assert_parse_err(
            parse_with_env(
                &flag_default_spec,
                &["test"],
                &[("DEPLOY_ENVS", "dev,staging")],
            ),
            "Invalid choice for option env: prod, expected one of dev, staging",
        );
    }

    #[test]
    fn test_variadic_arg_captures_unknown_flags_from_spec_string() {
        let spec: Spec = r#"
            flag "-v --verbose" var=#true
            arg "[database]" default="myapp_dev"
            arg "[args...]"
        "#
        .parse()
        .unwrap();
        let input: Vec<String> = vec!["test", "mydb", "--host", "localhost"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        let env = parsed.as_env();
        assert_eq!(env.get("usage_database").unwrap(), "mydb");
        assert_eq!(env.get("usage_args").unwrap(), "--host localhost");
    }

    #[test]
    fn test_variadic_arg_captures_unknown_flags() {
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(SpecFlag::builder().short('v').long("verbose").build())
            .arg(SpecArg::builder().name("database").required(false).build())
            .arg(
                SpecArg::builder()
                    .name("args")
                    .required(false)
                    .var(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // Unknown --host flag and its value should be captured by [args...]
        let input: Vec<String> = vec!["test", "mydb", "--host", "localhost"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        assert_eq!(parsed.args.len(), 2);
        let args_val = parsed
            .args
            .iter()
            .find(|(a, _)| a.name == "args")
            .unwrap()
            .1;
        match args_val {
            ParseValue::MultiString(v) => {
                assert_eq!(v, &vec!["--host".to_string(), "localhost".to_string()]);
            }
            _ => panic!("Expected MultiString, got {:?}", args_val),
        }
    }

    #[test]
    fn test_variadic_arg_captures_unknown_flags_with_double_dash() {
        let cmd = SpecCommand::builder()
            .name("test")
            .flag(SpecFlag::builder().short('v').long("verbose").build())
            .arg(SpecArg::builder().name("database").required(false).build())
            .arg(
                SpecArg::builder()
                    .name("args")
                    .required(false)
                    .var(true)
                    .build(),
            )
            .build();
        let spec = Spec {
            name: "test".to_string(),
            bin: "test".to_string(),
            cmd,
            ..Default::default()
        };

        // With explicit -- separator
        let input: Vec<String> = vec!["test", "--", "mydb", "--host", "localhost"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        assert_eq!(parsed.args.len(), 2);
        let args_val = parsed
            .args
            .iter()
            .find(|(a, _)| a.name == "args")
            .unwrap()
            .1;
        match args_val {
            ParseValue::MultiString(v) => {
                assert_eq!(v, &vec!["--host".to_string(), "localhost".to_string()]);
            }
            _ => panic!("Expected MultiString, got {:?}", args_val),
        }
    }

    #[test]
    fn test_variadic_arg_unknown_flag_equals_value_not_split() {
        // Regression: --flag=value should be treated as a single positional token when
        // --flag is not a known spec flag, not split into "--flag=value" AND "value".
        let spec: Spec = r#"arg "[other_args]" var=#true"#.parse().unwrap();

        // Single unknown --flag=value: must not produce a stray "3" positional.
        // as_env() shell-joins via shell_words::join, so "=" gets quoted.
        let input: Vec<String> = vec!["test", "--option=3"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed = parse(&spec, &input).unwrap();
        let env = parsed.as_env();
        assert_eq!(
            env.get("usage_other_args").map(String::as_str),
            Some("'--option=3'"),
            "expected a single --option=3 token, got {:?}",
            env.get("usage_other_args"),
        );

        // Multiple unknown --flag=value args should each be kept intact
        let input2: Vec<String> = vec!["test", "--foo=bar", "--baz=qux"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed2 = parse(&spec, &input2).unwrap();
        let env2 = parsed2.as_env();
        assert_eq!(
            env2.get("usage_other_args").map(String::as_str),
            Some("'--foo=bar' '--baz=qux'"),
            "expected two intact tokens, got {:?}",
            env2.get("usage_other_args"),
        );

        // Mix of plain positional args and unknown --flag=value tokens
        let input3: Vec<String> = vec!["test", "positional1", "--option=3", "positional2"]
            .into_iter()
            .map(String::from)
            .collect();
        let parsed3 = parse(&spec, &input3).unwrap();
        let env3 = parsed3.as_env();
        assert_eq!(
            env3.get("usage_other_args").map(String::as_str),
            Some("positional1 '--option=3' positional2"),
            "expected positional args and intact flag token, got {:?}",
            env3.get("usage_other_args"),
        );
    }

    #[test]
    fn test_allow_hyphen_values_consumes_short_flag_collision() {
        let spec = r#"
flag "-d --working-dir <DIR>"
flag "-a --args <ARGS>" allow_hyphen_values=#true
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "-a", "-destroy"])).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(flag_string_value(&parsed, "args"), "-destroy");
    }

    #[test]
    fn test_allow_hyphen_values_consumes_embedded_long_value() {
        let spec = r#"
flag "-d --working-dir <DIR>"
flag "-a --args <ARGS>" allow_hyphen_values=#true
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "--args=-destroy"])).unwrap();

        assert_eq!(parsed.flags.len(), 1);
        assert_eq!(flag_string_value(&parsed, "args"), "-destroy");
    }

    #[test]
    fn test_allow_hyphen_values_takes_the_separator_as_its_value() {
        // The flag is declared to accept a token that looks like a flag, and `--` looks
        // like one, so it binds — which is what clap does with the same declaration.
        // Letting the separator arm run first consumed it and left the flag hungry, and
        // the flag then ate the word past it: `-a -- -x` bound `-x` with the `--` gone.
        let spec = r#"
flag "-a --args <ARGS>" allow_hyphen_values=#true
arg "[rest]..."
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "-a", "--", "-x"])).unwrap();

        assert_eq!(flag_string_value(&parsed, "args"), "--");
        let rest = parsed
            .args
            .values()
            .next()
            .expect("expected the word after the separator to reach the argument");
        assert_eq!(rest.to_string(), "-x");
    }

    #[test]
    fn test_variadic_allow_hyphen_values_collects_after_a_hyphenated_first_value() {
        // Which token supplied the first value says nothing about how many the argument
        // takes, so collection carries on from a hyphenated one exactly as from a plain
        // one. It still stops at the next flag-like token, which is what keeps a second
        // occurrence of the flag from being eaten as a value.
        let spec = r#"
flag "-a --args <ARGS>..." allow_hyphen_values=#true
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "-a", "-x", "b", "c"])).unwrap();

        let flag = parsed
            .flags
            .keys()
            .find(|flag| flag.name == "args")
            .expect("expected args flag");
        match parsed.flags.get(flag).expect("expected args value") {
            ParseValue::MultiString(values) => assert_eq!(values, &["-x", "b", "c"]),
            other => panic!("expected a list of values, got {other:?}"),
        }
    }

    #[test]
    fn test_variadic_allow_hyphen_values_consumes_repeated_flag_values() {
        let spec = r#"
flag "-a --args <ARGS>" var=#true allow_hyphen_values=#true
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "-a", "-val1", "-a", "-val2"])).unwrap();

        let flag = parsed
            .flags
            .keys()
            .find(|flag| flag.name == "args")
            .expect("expected args flag");
        let value = parsed.flags.get(flag).expect("expected args value");
        match value {
            ParseValue::MultiString(values) => {
                assert_eq!(values, &vec!["-val1".to_string(), "-val2".to_string()]);
            }
            _ => panic!("expected MultiString, got {value:?}"),
        }
    }

    #[test]
    fn test_hyphen_values_still_default_to_short_flag_parsing() {
        let spec = r#"
flag "-d --working-dir <DIR>"
flag "-a --args <ARGS>"
"#
        .parse::<Spec>()
        .unwrap();

        let parsed = parse(&spec, &input(&["test", "-a", "-destroy"])).unwrap();

        assert_eq!(flag_string_value(&parsed, "working-dir"), "estroy");
    }

    /// `available_flags` has to agree with what an actual parse accepts, since
    /// its whole reason to exist is answering that question without one.
    mod available_flags {
        use super::*;

        fn spec() -> Spec {
            r#"
bin "test"
flag "-v --verbose" global=#true
flag "--raw" global=#true effect="write"
flag "--local-only"
cmd "run" {
    flag "-r --raw"
    flag "-w --watch"
    cmd "once"
}
"#
            .parse::<Spec>()
            .unwrap()
        }

        fn chain<'a>(spec: &'a Spec, path: &[&str]) -> Vec<&'a SpecCommand> {
            let mut chain = vec![&spec.cmd];
            for segment in path {
                chain.push(chain.last().unwrap().find_subcommand(segment).unwrap());
            }
            chain
        }

        fn names(spec: &Spec, path: &[&str]) -> Vec<String> {
            let mut names: Vec<_> = available_flags(&chain(spec, path))
                .iter()
                .map(|f| f.name.clone())
                .collect();
            names.sort();
            names
        }

        #[test]
        fn an_empty_chain_yields_nothing() {
            assert!(available_flags(&[]).is_empty());
        }

        #[test]
        fn the_root_gets_its_own_flags() {
            let spec = spec();
            assert_eq!(names(&spec, &[]), ["local-only", "raw", "verbose"]);
        }

        #[test]
        fn a_subcommand_keeps_globals_and_drops_local_only_ancestors() {
            let spec = spec();
            assert_eq!(names(&spec, &["run"]), ["raw", "verbose", "watch"]);
        }

        #[test]
        fn a_re_declared_global_is_listed_once() {
            // The merge can leave the long key on the merged flag and the short
            // key on the pre-merge one. Same flag; it must not be listed twice.
            let spec = r#"
bin "test"
flag "-y --yes" global=#true effect="write"
cmd "rm" {
    flag "-y --yes"
}
"#
            .parse::<Spec>()
            .unwrap();
            let flags = available_flags(&chain(&spec, &["rm"]));
            assert_eq!(flags.len(), 1, "{flags:?}");
            assert_eq!(flags[0].effect.map(|e| e.as_str()), Some("write"));
        }

        #[test]
        fn a_re_declared_global_keeps_the_globals_declaration() {
            // `run` re-declares the long-only global `--raw` as `-r --raw`
            // without `global`. That is the same flag: the global's `effect`
            // survives, the orphan short is unioned in, and it stays global.
            let spec = spec();
            let flags = available_flags(&chain(&spec, &["run"]));
            let raw = flags.iter().find(|f| f.name == "raw").unwrap();
            assert!(raw.global);
            assert_eq!(raw.effect.map(|e| e.as_str()), Some("write"));
            assert_eq!(raw.short, ['r']);
        }

        #[test]
        fn it_matches_what_a_parse_accepts() {
            // The invariant. If these ever disagree, one of them is lying to a
            // caller about which flags a command takes.
            let spec = spec();
            for path in [vec![], vec!["run"], vec!["run", "once"]] {
                let argv = std::iter::once("test".to_string())
                    .chain(path.iter().map(|s| s.to_string()))
                    .collect::<Vec<_>>();
                let parsed = parse_partial(&spec, &argv).unwrap();

                let mut from_parse: Vec<_> = unique_flags(parsed.available_flags.values())
                    .map(|f| f.name.clone())
                    .collect();
                from_parse.sort();
                assert_eq!(names(&spec, &path), from_parse, "path {path:?}");
            }
        }
    }
}
