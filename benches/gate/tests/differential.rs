//! Random command lines, parsed three ways, over mise's real spec.
//!
//! The corpus checks lines someone thought of. This checks lines nobody did: proptest builds
//! argv out of mise's own vocabulary — 210 command paths, 262 longs, 36 shorts — and every
//! generated line goes through all three parsers.
//!
//! # Why three and not two
//!
//! usage-lib is the reference for *rendering*, and the gate beside this holds usage-argv to it
//! byte for byte. It is a poor reference for *accepting*, and the first run of this test is what
//! showed it. Every disagreement in that run had usage-argv stricter than usage-lib, which reads
//! as a pile of usage-argv bugs until clap is asked the same questions:
//!
//! | line | usage-argv | usage-lib | clap |
//! |---|---|---|---|
//! | `mise generate` | refuse | **accept** | refuse |
//! | `mise bootstrap macos-defaults` | refuse | **accept** | refuse |
//! | `mise -t --interactive` | refuse | **accept** | refuse |
//!
//! usage-argv agrees with clap; usage-lib is the outlier, and being lax is how it manages to be.
//! So the standard here is clap, not usage-lib: clap is what mise ships today, so clap is what a
//! replacement may not regress. A disagreement with usage-lib where clap sides with usage-argv is
//! usage-lib's to fix, and is recorded as such below.
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
            K::InvalidValue => Verdict::InvalidChoice,
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

        // A global flag given at two levels: `mise -y config ls -y`. clap lets a global be
        // repeated deeper down and the inner occurrence win; usage-argv sees the same flag
        // twice and calls it a duplicate; usage-lib accepts. Found by the generator once it
        // learned to put tokens *before* the command path, which is what makes this shape
        // reachable at all.
        //
        // usage-argv is the strict one here, and the only one of the three, so this is
        // usage-argv's to settle rather than something the fuzzer should paper over —
        // recorded, with `a_repeated_global_is_usage_argvs_alone` below as the witness.
        Outcome {
            argv: Conflict,
            lib: true,
            clap: Accept,
        } => Some("usage-argv calls a global given at two levels a duplicate; clap allows it"),

        // usage-argv and clap both refuse; usage-lib accepts. Three of the four classes the
        // first run found are this shape: a missing subcommand, a flag whose value is missing,
        // and a repeated flag that may not repeat. usage-lib accepts all three.
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
        // Recorded rather than fixed: tightening usage-lib changes what every spec-driven
        // consumer accepts, which is its own change with its own blast radius.
        Outcome {
            argv,
            lib: true,
            clap,
        } if named_refusal(argv) && named_refusal(clap) => {
            Some("usage-lib is lax where usage-argv and clap agree")
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

        _ => None,
    }
}

proptest! {
    // Bounded deliberately: this runs in CI beside everything else, and 400 cases take about
    // three seconds. A local sweep at `PROPTEST_CASES=20000` turned up no cause the arms below
    // do not already name, which is what says 400 keeps the allow-list honest rather than merely
    // green. Re-run that sweep after touching the generator: the `head` tokens were added later
    // and found a class the first sweep could not reach, so a widened generator invalidates the
    // number rather than inheriting it.
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
fn a_bare_word_diverges_because_of_the_mount_not_the_parser() {
    // The first reading of this was "usage-argv refuses a lone `-` that clap accepts", filed as
    // a parser bug. It is not: `mise build` behaves identically, and the trace shows why —
    // both descend into `run`, whose positionals mise's spec supplies through a mount.
    //
    // Pinned so the explanation stays attached to the evidence. If usage-argv ever accepts
    // these, it is because the mount got resolved, and this test should say so instead.
    let spec = spec();
    // `x` is accepted by all three because `x` really is a command — mise's alias for `exec`.
    // That is what separates this from the `-` case: every word naming *nothing* fails here.
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
        (true, true, true),
        "`x` names a command, so nothing diverges"
    );
}

#[test]
fn a_repeated_global_is_usage_argvs_alone() {
    // Found by the generator once `head` let it put tokens before the command path. mise's
    // `-y` is `global=#true`, and clap lets a global be given again on the subcommand — the
    // inner occurrence simply wins. usage-argv sees one flag bound twice and refuses.
    //
    // Pinned rather than fixed: usage-argv is the only strict one of the three, and mise
    // ships clap today, so `mise -y config ls -y` is a line that works now. Which way that
    // should go is usage-argv's to settle; this records what is true meanwhile.
    let spec = spec();
    let repeated: Vec<String> = ["-y", "config", "ls", "-y"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let o = run(&spec, &repeated);
    assert_eq!(o.argv, Verdict::Conflict, "{o:?}");
    assert_eq!(o.accepted(), (false, true, true), "{o:?}");
    assert!(explained(o).is_some(), "{o:?}");

    // At one level, though, clap refuses too — so the divergence really is about globals
    // crossing a command boundary, not about repetition.
    let twice: Vec<String> = ["-y", "-y"].iter().map(|s| s.to_string()).collect();
    let o = run(&spec, &twice);
    assert_eq!((o.argv, o.clap), (Verdict::Conflict, Verdict::Conflict));
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
fn the_vocabulary_is_actually_mises() {
    // A generator drawing from an empty vocabulary would pass this file by testing nothing.
    let spec = spec();
    let (cmds, longs, shorts) = vocabulary(&spec);
    assert!(cmds.len() > 200, "command paths: {}", cmds.len());
    assert!(longs.len() > 250, "longs: {}", longs.len());
    assert!(shorts.len() > 30, "shorts: {}", shorts.len());
}
