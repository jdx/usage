//! Random command lines, parsed three ways, over mise's real spec.
//!
//! The corpus checks lines someone thought of. This checks lines nobody did: proptest builds
//! argv out of mise's own vocabulary — 210 command paths, 262 longs, 36 shorts — and every
//! generated line goes through all three parsers.
//!
//! # Why three and not two
//!
//! usage-lib is the reference for *rendering*, and the gate beside this holds usage-argv to it
//! byte for byte. It is a weaker reference for *accepting*: clap is what mise ships today, so
//! clap is what a replacement may not regress, and a line all three read differently is worth
//! seeing whichever way it goes. Hence three.
//!
//! The first run of this test produced disagreements that all looked the same way round —
//! usage-argv stricter than usage-lib — and an earlier version of this comment concluded that
//! usage-lib was lax and had three things to fix. **That was wrong, and the corpus says so.**
//! Two of the three are the grammar working as specified:
//!
//! | corpus vector | what it specifies |
//! |---|---|
//! | `long-repeated-keeps-the-last` | a repeat is a correction, and the later occurrence wins |
//! | `long-unknown` | an unknown flag is "data in transit", offered to the positionals |
//!
//! Acting on that reading got as far as three failing conformance vectors before the mistake
//! surfaced. The third was real and is fixed: `subcommand_required` was in the spec and no
//! parser read it (#992).
//!
//! Repeats are permissive in usage by default, including at the derive layer. A command that
//! owns its entire grammar can opt into clap's rule with `args_override_self=false`:
//!
//! | | a flag given twice |
//! |---|---|
//! | usage-argv, the parser | accepts, last wins — conformant |
//! | usage-lib | accepts, last wins — conformant |
//! | usage-derive, by default | accepts, last wins — conformant |
//! | clap | refuses |
//!
//! The deliberate default keeps wrappers and task runners from rejecting corrections. Fleet
//! commands that need clap parity declare strictness explicitly; the gate's generated shadow
//! leaves the default visible here so tightening it cannot happen accidentally.
//!
//! # A verdict is not enough
//!
//! Each class below names the *reason* a parser refused, not only that it did. Three booleans
//! would allow-list every line of a known shape whatever caused it. This previously caught a
//! global flag incorrectly rejected across a command boundary; the regression test remains even
//! though permissive repeats have since made that particular line valid more broadly.
//! usage-lib's failures are `miette::Error`, a message with no class behind it,
//! so it stays a yes-or-no and the narrowing leans on the two parsers that can say why.
//!
//! # Mounts are stripped, and must be
//!
//! mise's spec carries `mount run="mise tasks --usage"` twice. usage-lib resolves a mount by
//! *running* that command mid-parse — so the first draft of this test spawned real `mise`
//! processes, which loaded config, fetched vfox metadata, and shelled out to `apt-cache`. One
//! generated line took minutes. Any fuzzer over a real spec has to neutralise mounts first;
//! that they are "not covered by the corpus" is the smaller half of the problem.

use std::ffi::OsStr;
use std::sync::LazyLock;

use proptest::prelude::*;
use usage::Spec;

/// Parsed once. Reparsing mise's spec per case cost 80 seconds for 400 cases; the vocabulary
/// walk is not free either, and neither changes between cases.
static SPEC: LazyLock<Spec> = LazyLock::new(spec);
static VOCAB: LazyLock<(Vec<String>, Vec<String>, Vec<String>)> =
    LazyLock::new(|| vocabulary(&SPEC));

/// mise's spec with the mounts taken out. See the note above: leaving them in executes them.
fn spec() -> Spec {
    let kdl: String = include_str!("../../mise.usage.kdl")
        .lines()
        .filter(|l| !l.trim_start().starts_with("mount "))
        .collect::<Vec<_>>()
        .join("\n");
    kdl.parse().expect("mise's spec should parse")
}

/// Every command path, long, and short the spec knows.
fn vocabulary(spec: &Spec) -> (Vec<String>, Vec<String>, Vec<String>) {
    fn walk(
        path: Vec<String>,
        cmd: &usage::SpecCommand,
        cmds: &mut Vec<String>,
        longs: &mut Vec<String>,
        shorts: &mut Vec<String>,
    ) {
        for f in &cmd.flags {
            longs.extend(f.long.iter().map(|l| format!("--{l}")));
            shorts.extend(f.short.iter().map(|s| format!("-{s}")));
            // Stored with its dashes, unlike usage-argv's.
            longs.extend(f.negate.clone());
        }
        for (name, sub) in &cmd.subcommands {
            // Aliases sit in the map beside the name they point at.
            if sub.name != *name {
                continue;
            }
            let mut p = path.clone();
            p.push(name.clone());
            cmds.push(p.join(" "));
            walk(p, sub, cmds, longs, shorts);
        }
    }
    let (mut cmds, mut longs, mut shorts) = (Vec::new(), Vec::new(), Vec::new());
    walk(Vec::new(), &spec.cmd, &mut cmds, &mut longs, &mut shorts);
    for v in [&mut cmds, &mut longs, &mut shorts] {
        v.sort();
        v.dedup();
    }
    (cmds, longs, shorts)
}

/// What a parser did with one line: accepted it, or refused it *for a named reason*.
///
/// The reason is what keeps the allow-list below honest. A predicate over three booleans
/// admits every disagreement of a known shape whatever caused it, so a new refusal landing on
/// an allow-listed shape stays green — a gate reporting what it was told rather than what it
/// found. Two of the three parsers can say why they refused, so the classes are keyed on that.
///
/// Coarse on purpose: three parsers written by three sets of hands do not share an error
/// taxonomy, and a class fine enough to separate their wordings is one no line could match in
/// all three at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Accept,
    /// A dash-prefixed token matched no flag.
    UnknownFlag,
    /// A flag that needs a value did not get one.
    MissingValue,
    /// clap's `InvalidValue` with an empty value: a value-taking flag was the final token.
    /// Kept separate because usage may already have bound that token as a positional value,
    /// which is a routing disagreement rather than the ordinary missing-value class.
    MissingValueAtEnd,
    /// A word arrived with no argument left to hold it.
    UnexpectedWord,
    /// A subcommand was wanted, and the line named none.
    MissingSubcommand,
    /// Something declared required was never given.
    MissingRequired,
    /// Two things on the line cannot go together — including a flag with itself. usage-argv
    /// separates a repeat from a declared conflict and clap does not, so they are one class
    /// here; splitting them would make a class no line could match in both.
    Conflict,
    /// A value outside the declared choices.
    InvalidChoice,
    /// `--help` or `--version`. Not a failure, and handed back as one by both parsers that
    /// have an error type — a parse that stops to answer a question produced no value — so it
    /// gets a name of its own rather than being counted as a refusal.
    Question,
    /// Refused for a reason none of the above name. Deliberately not allow-listed anywhere
    /// below: a class this file has never seen is a finding, not a known shape.
    Other,
}

/// What all three did with one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Outcome {
    argv: Verdict,
    /// usage-lib refuses with a `miette::Error`: a rendered message, with no class behind it
    /// to read. So this one stays a yes-or-no, and the narrowing below leans on the two that
    /// can say why. Matching its wording would pin usage-lib's prose, which is not the thing
    /// under test.
    lib: bool,
    clap: Verdict,
}

impl Outcome {
    /// The three booleans this used to be, for the agreement check and the failure message.
    fn accepted(&self) -> (bool, bool, bool) {
        (
            self.argv == Verdict::Accept,
            self.lib,
            self.clap == Verdict::Accept,
        )
    }
}

fn argv_verdict(line: &[&OsStr]) -> Verdict {
    use usage_argv::Error as E;
    match shadow_mise::Cli::parse_from(line) {
        Ok(_) => Verdict::Accept,
        // The fallback arm is mandatory — `Error` is `non_exhaustive` — as well as right: a
        // variant added later lands in `Other`, where it stops matching whichever class it
        // used to be waved through as. That is the safe direction for a gate to fail in.
        Err(e) => match e {
            E::UnknownFlag { .. } => Verdict::UnknownFlag,
            E::MissingFlagValue { .. } => Verdict::MissingValue,
            E::UnexpectedArg { .. } => Verdict::UnexpectedWord,
            E::MissingSubcommand => Verdict::MissingSubcommand,
            E::MissingRequired { .. } => Verdict::MissingRequired,
            E::DuplicateFlag { .. } | E::ConflictingFlags { .. } => Verdict::Conflict,
            E::InvalidChoice { .. } => Verdict::InvalidChoice,
            E::Help { .. } | E::Version { .. } => Verdict::Question,
            _ => Verdict::Other,
        },
    }
}

fn clap_verdict(line: &[String]) -> Verdict {
    use clap::error::ErrorKind as K;
    match <shadow_mise_clap::Cli as clap::Parser>::try_parse_from(line.iter()) {
        Ok(_) => Verdict::Accept,
        Err(e) => match e.kind() {
            // clap spends `UnknownArgument` on both an unrecognised flag and a word with
            // nowhere to go, separating them only in the prose. Those two are most of the
            // point of classifying at all, so they are separated here by asking what the
            // offending token looked like — the same question usage-argv's parser asks.
            K::UnknownArgument => match e
                .get(clap::error::ContextKind::InvalidArg)
                .map(|v| v.to_string())
            {
                Some(t) if t.starts_with('-') && t != "-" => Verdict::UnknownFlag,
                _ => Verdict::UnexpectedWord,
            },
            K::InvalidSubcommand | K::TooManyValues => Verdict::UnexpectedWord,
            // `DisplayHelpOnMissingArgumentOrSubcommand` is how clap says "this command wants a
            // subcommand and got none" when it has help to print: `mise oci` reaches it, and
            // usage-argv calls the same line `MissingSubcommand`.
            K::MissingSubcommand | K::DisplayHelpOnMissingArgumentOrSubcommand => {
                Verdict::MissingSubcommand
            }
            K::MissingRequiredArgument => Verdict::MissingRequired,
            // clap also reports a value-taking flag at end-of-line as `InvalidValue`, with an
            // empty invalid value and no valid choices. That is a missing value, not a choice
            // rejection: `mise asdf 3 -j` is the fleet case that distinguishes the two.
            K::InvalidValue => match e
                .get(clap::error::ContextKind::InvalidValue)
                .map(|v| v.to_string())
            {
                Some(value) if value.is_empty() => Verdict::MissingValueAtEnd,
                _ => Verdict::InvalidChoice,
            },
            K::NoEquals | K::TooFewValues | K::WrongNumberOfValues => Verdict::MissingValue,
            // Also what clap raises for a flag given twice — "cannot be used multiple times" —
            // which is why the class covers both.
            K::ArgumentConflict => Verdict::Conflict,
            K::DisplayHelp | K::DisplayVersion => Verdict::Question,
            _ => Verdict::Other,
        },
    }
}

fn run(spec: &Spec, line: &[String]) -> Outcome {
    let owned: Vec<&OsStr> = line.iter().map(|s| OsStr::new(s.as_str())).collect();
    let argv = argv_verdict(&owned);

    // usage-lib and clap both want the program name; usage-argv is given argv without it.
    let mut with_bin: Vec<String> = vec!["mise".to_string()];
    with_bin.extend(line.iter().cloned());
    let lib = usage::parse(spec, &with_bin).is_ok();

    Outcome {
        argv,
        lib,
        clap: clap_verdict(&with_bin),
    }
}

/// A refusal this file has a name for. `Other` and `Question` are not refusals of a named
/// kind: the first is a class never seen here, and the second is not a refusal at all.
fn named_refusal(v: Verdict) -> bool {
    use Verdict::*;
    matches!(
        v,
        UnknownFlag
            | MissingValue
            | MissingValueAtEnd
            | UnexpectedWord
            | MissingSubcommand
            | MissingRequired
            | Conflict
            | InvalidChoice
    )
}

/// The disagreements this test knows about, each with who is right and why.
///
/// A line matching none of these and disagreeing anywhere is a finding. Written as a predicate
/// over the outcome rather than over the text, because the text is generated: what identifies a
/// class is the *shape* of the disagreement together with the reason each parser gives for its
/// half of it.
fn explained(o: Outcome) -> Option<&'static str> {
    use Verdict::*;
    match o {
        // A question — `--help`, `--version` — which both error-carrying parsers hand back as
        // an `Err` and usage-lib answers by parsing normally. Named first, because otherwise it
        // reads as the lax-usage-lib class below and is nothing of the kind: nobody here
        // disagrees about what the line *means*.
        Outcome {
            argv: Question,
            lib: true,
            clap: Question,
        } => Some("a question, which usage-lib answers by accepting rather than by erroring"),

        // usage-argv and clap both refuse; usage-lib accepts.
        //
        // Read once as "usage-lib is lax in three ways". It is not — see the header. Two of the
        // three are specified behaviour (`long-repeated-keeps-the-last`, `long-unknown`), and
        // the refusals come from usage-derive's post-binding checks, which the grammar does not
        // require and clap happens to share. The third, a missing subcommand, was real and is
        // fixed, so lines of that cause no longer reach this arm at all.
        //
        // The narrowing here is on the *set* of reasons rather than on the two matching. A
        // first attempt required `argv == clap` and was wrong: a generated line often carries
        // more than one fault, and each parser reports the first one it reaches. To usage-argv
        // `mise -s --project-origin=v tasks run --wat` is a flag missing its value, and to clap
        // it is an unknown flag — both correct, about different tokens. What is worth pinning
        // is that neither lands in `Other`: a refusal of a kind this file has never seen is a
        // finding even when the verdicts line up.
        //
        // The shape an adopter cannot be hurt by, too — usage-argv and clap agree on the
        // answer — which is why a looser rule is the right one here and not in the arms below.
        //
        // Recorded, and not to be "fixed": tightening usage-lib here would break the corpus.
        // An attempt to do exactly that failed `long-repeated-keeps-the-last`,
        // `long-boolean-repeated` and `a-bound-counts-one-occurrence` — which is the check that
        // caught the misreading, and the reason this arm now carries the reasoning rather than a
        // to-do.
        Outcome {
            argv,
            lib: true,
            clap,
        } if named_refusal(argv) && named_refusal(clap) => {
            Some("the derive and clap add a rule the grammar does not require")
        }

        // A word that names no command: `mise build`, and `mise -` for the same reason. Both
        // usage parsers descend into the root's `default_subcommand` (`run`), and mise's spec
        // gives `run` no positionals at all — they come from `mount run="mise tasks --usage"`,
        // which usage-argv does not execute and which is stripped here. So there is nothing for
        // the word to bind to.
        //
        // clap accepts it because clap has no mount: `gen-shadow` puts the positionals on the
        // clap root instead, since that is the nearest thing clap can say. The disagreement is
        // between the two *fixtures*, not the two parsers — see
        // `parse.rs::a_bare_task_routes_through_run_and_then_needs_the_mount`, which pins the
        // same behaviour deliberately.
        //
        // Pinned to `UnexpectedWord` for that reason: the missing mount shows up as a word with
        // nowhere to bind, and a refusal here for any other cause is a finding rather than this.
        Outcome {
            argv: UnexpectedWord,
            lib: false,
            clap: Accept,
        } => Some("the word needs the mount usage-argv does not run, which clap's shadow lacks"),

        // And one that is usage-argv's alone: a bare `-`. usage-lib and clap both bind it to the
        // root's `[TASK]`; usage-argv lets it *select* the default subcommand, descends into
        // `run`, and finds no positional there — so it too refuses with a word it cannot place.
        // `is_flag_like` already says `-` is a value ("conventionally stdin"), and a value was
        // never a candidate to select anything, which is the reasoning `--` is excluded by two
        // lines away.
        //
        // Fixed in the commit above this one; the arm stays only so this file bisects cleanly.
        Outcome {
            argv: UnexpectedWord,
            lib: true,
            clap: Accept,
        } => Some("usage-argv lets a bare `-` select the default subcommand"),

        // Both usage parsers accept an unknown flag where clap refuses it: mise's spec lets an
        // unrecognised flag fall through to a positional, so `mise dotfiles status --stdin`
        // binds `--stdin` as a value rather than failing. PLAN.md carries this as an open
        // decision — mise parses *task* arguments with this parser at run time, so refusing an
        // undeclared flag would change what a task accepts, not only what a completion offers.
        //
        // The permissive direction, so not a regression for an adopter, but a difference. Held
        // to clap's *reason* as well as its verdict: clap refusing a line usage accepts for any
        // cause other than a token it could not place is a different question entirely.
        Outcome {
            argv: Accept,
            lib: true,
            clap: UnknownFlag | UnexpectedWord,
        } => Some("usage lets an unknown flag fall through where clap refuses it"),

        // Reachable only once a bare `-` binds — so this arm arrived with the commit above that
        // let it. `mise - bootstrap packages use` fills the root's `[TASK]` with `-`, and usage
        // reads everything after it as that task's arguments. clap keeps looking for a
        // subcommand, finds the real `bootstrap packages use`, and reports the required argument
        // *it* declares.
        //
        // Verified rather than assumed: `mise - bootstrap` is accepted by both, and
        // `mise bootstrap packages use` is refused by both — so the difference is what happens
        // to words *after* a positional is bound, not the words themselves.
        // Any of these verdicts, because the cause is the routing and not what the routed command
        // happened to want: `mise - bootstrap packages use` reaches a command missing a required
        // argument, `mise - bootstrap dotfiles` reaches one missing a subcommand,
        // `mise - unuse x -g --global` reaches duplicate spellings of the same flag, and
        // `mise asdf 3 -j` reaches a global flag missing its value. Keying on the first of those
        // alone left later generated examples unexplained, which is the narrowness this arm was
        // written to avoid in the first place.
        Outcome {
            argv: Accept,
            lib: true,
            clap: MissingValueAtEnd | MissingRequired | MissingSubcommand | Conflict | InvalidChoice,
        } => Some("after a positional binds, usage keeps the words; clap still routes them"),

        _ => None,
    }
}

proptest! {
    // Bounded deliberately: this runs in CI beside everything else, and 400 cases take about
    // three seconds. A local sweep at `PROPTEST_CASES=20000` — 127 seconds — turned up no cause
    // the arms below do not already name, which is what says 400 keeps the allow-list honest
    // rather than merely green. Re-run that sweep after touching the generator: `head` and
    // `rooted` were added later and found a class the first sweep could not reach, so a widened
    // generator invalidates the number rather than inheriting it.
    //
    // A failure persists to `differential.proptest-regressions` beside this file and is replayed
    // first on the next run, so a finding does not evaporate with the seed.
    #![proptest_config(ProptestConfig { cases: 400, max_shrink_iters: 200, ..ProptestConfig::default() })]

    #[test]
    fn no_unexplained_disagreement(
        seed in any::<u64>(),
        // Tokens *before* the command path, and whether there is a command path at all. An
        // earlier draft always opened with a full path, which left two shapes outside the
        // generator's reach: a line that selects no command and exercises the root's own
        // flags and positionals, and a global given before command selection. The second is
        // `mise -t --interactive`, one of the disagreements the first run found, so the
        // generator was unable to rediscover a class this file documents.
        //
        // Weighted rather than even: a command path is the common case and where most of the
        // vocabulary lives, and a generator spending half its cases on the root would be
        // shallower everywhere else.
        head in prop::collection::vec(0usize..12, 0..3),
        rooted in prop::bool::weighted(0.15),
        len in 0usize..6,
        junk in prop::collection::vec(0usize..12, 0..6),
    ) {
        let spec = &*SPEC;
        let (cmds, longs, shorts) = &*VOCAB;

        // The seed picks words; proptest shrinks the shape around it.
        let mut at = seed;
        let mut next = move || { at = at.wrapping_mul(6364136223846793005).wrapping_add(1); (at >> 11) as usize };

        let junk_words = ["--wat", "-Z", "", "x", "3", "--", "-", "a=b", "node@20", "a,b", "-vvv", "--="];

        // Drawn first so that `word` below can take the generator: both need it, and one
        // closure holding it is simpler than threading it through.
        let path: Vec<String> = if rooted || cmds.is_empty() {
            Vec::new()
        } else {
            cmds[next() % cmds.len()].split(' ').map(str::to_string).collect()
        };

        let mut word = |kind: usize| match kind % 6 {
            0 | 1 => longs[next() % longs.len()].clone(),
            2 => shorts[next() % shorts.len()].clone(),
            3 => format!("{}=v", longs[next() % longs.len()]),
            4 => junk_words[next() % junk_words.len()].to_string(),
            _ => ["x", "1", "node@20"][next() % 3].to_string(),
        };

        let mut line: Vec<String> = head.iter().map(|k| word(*k)).collect();
        line.extend(path);
        for i in 0..len {
            line.push(word(junk.get(i).copied().unwrap_or(0)));
        }
        prop_assume!(!line.is_empty());

        let o = run(spec, &line);
        let (argv, lib, clap) = o.accepted();
        let agreed = argv == lib && lib == clap;
        prop_assert!(
            agreed || explained(o).is_some(),
            "unexplained disagreement on `mise {}`:\n  usage-argv {:?}\n  usage-lib  {}\n  clap       {:?}",
            line.join(" "),
            o.argv,
            if lib { "Accept" } else { "refuse" },
            o.clap,
        );
    }
}

#[test]
fn positional_routing_can_still_reach_a_genuine_invalid_choice() {
    let outcome = run(&SPEC, &["asdf".into(), "3".into(), "--log-level=v".into()]);
    assert_eq!(outcome.argv, Verdict::Accept);
    assert!(outcome.lib);
    assert_eq!(outcome.clap, Verdict::InvalidChoice);
    assert!(explained(outcome).is_some());
}

#[test]
fn clap_shadow_keeps_flag_requirements_from_the_shared_spec() {
    let outcome = run(&SPEC, &["set".into(), "--stdin".into()]);
    assert_eq!(outcome.argv, Verdict::MissingRequired);
    assert!(!outcome.lib);
    assert_eq!(outcome.clap, Verdict::MissingRequired);
}

#[test]
fn a_bare_word_diverges_because_of_the_mount_not_the_parser() {
    // The first reading of this was "usage-argv refuses a lone `-` that clap accepts", filed as
    // a parser bug. It is not: `mise build` behaves identically, and the trace shows why —
    // both descend into `run`, whose positionals mise's spec supplies through a mount.
    //
    // Pinned so the explanation stays attached to the evidence. If usage-argv ever accepts
    // these, it is because the mount got resolved, and this test should say so instead.
    let spec = spec();
    // `x` really is a command — mise's alias for `exec` — but that command requires either its
    // trailing command positional or `--command`. Both typed fixtures carry that positional
    // relationship.
    for word in ["build", "foo", "node@20"] {
        let o = run(&spec, &[word.to_string()]);
        // The *reason* matters as much as the refusal: what the missing mount costs is a
        // positional, so a word with nowhere to bind is the failure this class is made of.
        assert_eq!(
            o.argv,
            Verdict::UnexpectedWord,
            "{word}: usage-argv has no positional on `run` to bind"
        );
        assert!(
            !o.lib,
            "{word}: usage-lib agrees once the mount is stripped"
        );
        assert_eq!(
            o.clap,
            Verdict::Accept,
            "{word}: clap's shadow carries the positionals on the root"
        );
    }
    let o = run(&spec, &["x".to_string()]);
    assert_eq!(
        o.accepted(),
        (false, false, false),
        "`x` reaches exec, whose positional relationship every typed fixture carries"
    );
}

#[test]
fn a_global_repeat_is_permissive_unless_the_command_opts_into_strictness() {
    // Found by the generator once `head` let it put tokens before the command path. `-y` is
    // `global=#true`; clap lets it be given again on
    // a subcommand — the inner occurrence wins — and refuses a repeat at one level. Usage keeps
    // the last occurrence at either level unless that command opts into strictness.
    let spec = spec();
    let go = |words: &[&str]| {
        run(
            &spec,
            &words.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    };

    // Across a boundary: allowed, at any depth, and more than once.
    for words in [
        &["-y", "config", "ls", "-y"][..],
        &["-y", "config", "-y", "ls"][..],
    ] {
        let o = go(words);
        assert_eq!(o.accepted(), (true, true, true), "{words:?} {o:?}");
    }

    // Twice at one level: usage's permissive default keeps the later occurrence, while clap's
    // strict default refuses it.
    for words in [
        &["-y", "-y"][..],
        &["config", "ls", "-y", "-y"][..],
        &["-y", "config", "ls", "-y", "-y"][..],
    ] {
        let o = go(words);
        assert_eq!(o.argv, Verdict::Accept, "{words:?} {o:?}");
        assert!(o.lib, "{words:?} {o:?}");
        assert_eq!(o.clap, Verdict::Conflict, "{words:?} {o:?}");
    }
}

#[test]
fn a_reason_separates_two_lines_of_the_same_shape() {
    // What the reason buys, stated as a test rather than only in the comment above
    // `Verdict`: an allow-list over three booleans admits every line of a known shape,
    // whatever caused it.
    let spec = spec();

    // The class: an undeclared flag falls through to a positional, and only clap minds.
    let known = run(
        &spec,
        &[
            "dotfiles".to_string(),
            "status".to_string(),
            "--stdin".to_string(),
        ],
    );
    assert_eq!(known.accepted(), (true, true, false), "{known:?}");
    assert!(explained(known).is_some(), "{known:?}");

    // The same three booleans, a different cause. clap wanting a value that usage bound
    // elsewhere is not "an unknown flag fell through", and is not waved through as it.
    let invented = Outcome {
        argv: Verdict::Accept,
        lib: true,
        clap: Verdict::MissingValue,
    };
    assert_eq!(invented.accepted(), known.accepted());
    assert!(
        explained(invented).is_none(),
        "a shape-only allow-list would have called this explained"
    );

    // And the same again for a refusal of a kind this file has never seen: agreeing verdicts
    // are not enough to make a line one of the known classes.
    let unheard_of = Outcome {
        argv: Verdict::Other,
        lib: true,
        clap: Verdict::UnknownFlag,
    };
    assert!(explained(unheard_of).is_none(), "{unheard_of:?}");
}

#[test]
fn words_after_a_bound_positional_stay_values_for_usage() {
    // The class the `-` fix opened, pinned with the three lines that isolate it.
    let spec = spec();

    // `bootstrap packages use` is a real command path with a required argument. With `-` in
    // front of it, usage has already bound the root's `[TASK]` and reads the rest as its
    // arguments; clap routes into the command and enforces what it declares.
    let routed = run(
        &spec,
        &["-", "bootstrap", "packages", "use"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert_eq!(routed.clap, Verdict::MissingRequired, "{routed:?}");
    assert_eq!(routed.accepted(), (true, true, false), "{routed:?}");
    assert!(explained(routed).is_some(), "{routed:?}");

    // The same cause reaching a command that wants a *subcommand* rather than an argument.
    // Found by the sweep after the arm was first written for `MissingRequired` alone.
    let deeper = run(
        &spec,
        &["-", "bootstrap", "dotfiles"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert_eq!(deeper.clap, Verdict::MissingSubcommand, "{deeper:?}");
    assert!(explained(deeper).is_some(), "{deeper:?}");

    // The same routing difference can make clap see two spellings of one flag after usage has
    // already committed to the root positional. A random CI seed found this third verdict.
    let duplicate = run(
        &spec,
        &["-", "unuse", "x", "-g", "--global"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert_eq!(duplicate.clap, Verdict::Conflict, "{duplicate:?}");
    assert_eq!(duplicate.accepted(), (true, true, false), "{duplicate:?}");
    assert!(explained(duplicate).is_some(), "{duplicate:?}");

    // Not about those words: one of them alone is fine for both.
    let short = run(
        &spec,
        &["-", "bootstrap"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert_eq!(short.accepted(), (true, true, true), "{short:?}");

    // And not about `-` either: without it, all three refuse the same path.
    let bare = run(
        &spec,
        &["bootstrap", "packages", "use"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert!(!bare.accepted().2, "{bare:?}");
}

#[test]
fn usage_lib_accepting_a_repeat_is_the_grammar_not_a_defect() {
    // The guard on the misreading in the header. An earlier reading of this file called
    // usage-lib lax for accepting these and set out to tighten it; the corpus specifies both,
    // and the attempt failed `long-repeated-keeps-the-last`, `long-boolean-repeated` and
    // `a-bound-counts-one-occurrence`.
    //
    // Asserted here rather than left to prose, so that tightening usage-lib fails a test whose
    // name says why — the corpus vectors are in `corpus/01-long-flags.json`, and this is the
    // same statement said where someone touching the parser will run it.
    let spec = spec();

    // A repeat is a correction: the later occurrence wins. mise's `--jobs` is the shape.
    let repeated = run(
        &spec,
        &["--jobs", "1", "--jobs", "2"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    assert!(
        repeated.lib,
        "usage-lib must accept a repeated flag: `long-repeated-keeps-the-last` specifies it. {repeated:?}"
    );

    // An unknown flag is data in transit, offered to the positionals.
    let unknown = run(
        &spec,
        &["--wat"].iter().map(|s| s.to_string()).collect::<Vec<_>>(),
    );
    assert!(
        unknown.lib,
        "usage-lib must accept an unknown flag: `long-unknown` specifies it. {unknown:?}"
    );

    // The typed derive follows the grammar's permissive default too. A CLI that owns the grammar
    // opts into strictness explicitly; clap starts strict.
    assert_eq!(repeated.argv, Verdict::Accept, "{repeated:?}");
    assert_eq!(repeated.clap, Verdict::Conflict, "{repeated:?}");
}

#[test]
fn the_vocabulary_is_actually_mises() {
    // A generator drawing from an empty vocabulary would pass this file by testing nothing.
    let spec = spec();
    let (cmds, longs, shorts) = vocabulary(&spec);
    assert!(cmds.len() > 200, "command paths: {}", cmds.len());
    assert!(longs.len() > 250, "longs: {}", longs.len());
    assert!(shorts.len() > 30, "shorts: {}", shorts.len());
}
