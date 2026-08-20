//! A struct per command, and the `Parse` that fills them.
//!
//! The front door. An author calls `Parse(os.Args[1:])` and gets a value with
//! fields rather than a loop over events; everything below — binding, the
//! post-binding rules, the three tables — is unchanged, and this is the shape
//! that makes it usable without knowing any of it.
//!
//! Fields are `string`, `bool` and `[]string`, because that is what a usage spec
//! knows. A spec says what a value is *called* and never what type it is, so
//! turning `"8"` into an `int` stays the caller's business — `argv.Int` and its
//! neighbours exist for exactly that, and inferring a type from an argument's
//! name would be guessing.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use super::{field_name, Emitted};
use crate::{SpecArg, SpecFlag};

/// The field each entry is assigned to, by key.
///
/// Worked out once and shared by the struct declarations and by `Parse`, so the
/// two cannot disagree about where a value goes.
type Fields = HashMap<String, String>;

/// Write every command's struct, then `Parse`.
pub(super) fn emit(out: &mut String, commands: &[Emitted]) {
    let mut assigned: Fields = HashMap::new();
    for e in commands {
        let doc = if e.root {
            format!("// {} is the whole command line.", name(e))
        } else {
            format!("// {} is `{}`.", name(e), e.cmd.full_cmd.join(" "))
        };
        let _ = writeln!(out, "{doc}");
        let _ = writeln!(out, "type {} struct {{", name(e));

        let mut fields: Vec<(String, String, String)> = Vec::new();
        let mut taken: HashSet<String> = HashSet::new();
        // A field name has to be unique *within its struct*, and a command can
        // declare a `--shell` flag beside a `shell` subcommand — mise does, and
        // does the same with `version`, `command`, `env` and `tool`. The kind is
        // what disambiguates, because `ShellCmd` says which one it is where
        // `Shell2` says only that there were two.
        let mut claim = |base: String, suffix: &str| -> String {
            if taken.insert(base.clone()) {
                return base;
            }
            let kinded = format!("{base}{suffix}");
            if taken.insert(kinded.clone()) {
                return kinded;
            }
            for n in 2.. {
                let numbered = format!("{kinded}{n}");
                if taken.insert(numbered.clone()) {
                    return numbered;
                }
            }
            unreachable!("the loop returns")
        };

        for (flag, named) in &e.flags {
            let field = claim(field_name(&flag.name), "Flag");
            fields.push((
                field.clone(),
                flag_type(flag).to_string(),
                named.key.clone(),
            ));
            assigned.insert(named.key.clone(), field);
        }
        for (arg, named) in &e.args {
            let field = claim(field_name(&arg.name), "Arg");
            fields.push((field.clone(), arg_type(arg).to_string(), named.key.clone()));
            assigned.insert(named.key.clone(), field);
        }
        for at in &e.subcommands {
            let sub = &commands[*at];
            // A pointer, and at most one is set: the command line selects one path
            // down the tree.
            let field = claim(field_name(&sub.cmd.name), "Cmd");
            fields.push((
                field.clone(),
                format!("*{}", name(sub)),
                sub.named.key.clone(),
            ));
            assigned.insert(sub.named.key.clone(), field);
        }

        // gofmt aligns a run of field declarations into columns, so this does too.
        let name_col = fields.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
        let type_col = fields.iter().map(|(_, t, _)| t.len()).max().unwrap_or(0);
        for (field, ty, key) in &fields {
            let _ = writeln!(out, "\t{field:<name_col$} {ty:<type_col$} // {key}");
        }
        let _ = writeln!(out, "}}\n");
    }

    parse_fn(out, commands, &assigned);
}

fn parse_fn(out: &mut String, commands: &[Emitted], assigned: &Fields) {
    let root = &commands[0];
    let mut strict_keys = commands
        .iter()
        .flat_map(|command| {
            command
                .flags
                .iter()
                .filter(|(flag, _)| {
                    !command.cmd.args_override_self
                        && !flag.var
                        && !flag.count
                        && !flag.arg.as_ref().is_some_and(|arg| arg.var)
                })
                .map(|(_, named)| named.key.as_str())
        })
        .collect::<Vec<_>>();
    strict_keys.sort_unstable();
    strict_keys.dedup();
    let mut strict_negate_keys = commands
        .iter()
        .flat_map(|command| {
            command
                .flags
                .iter()
                .filter(|(flag, _)| !command.cmd.args_override_self && flag.negate.is_some())
                .map(|(_, named)| named.key.as_str())
        })
        .collect::<Vec<_>>();
    strict_negate_keys.sort_unstable();
    strict_negate_keys.dedup();
    let duplicate_state = if strict_keys.is_empty() {
        String::new()
    } else {
        let polarity = if strict_negate_keys.is_empty() {
            ""
        } else {
            "\tpolaritySeen := map[uint64]uint8{}\n"
        };
        format!(
            "\tlevelSeen := map[uint64]int{{}}\n{polarity}\tstrictSeen := map[uint64]bool{{}}\n\tduplicateSeen := map[uint64]bool{{}}\n"
        )
    };
    let duplicate_event = if strict_keys.is_empty() {
        String::new()
    } else {
        let keys = strict_keys.join(", ");
        let per_kind = if strict_negate_keys.is_empty() {
            "\t\t\t\tlevelSeen[ev.Flag.Key]++\n\t\t\t\tif levelSeen[ev.Flag.Key] > 1 {\n\t\t\t\t\tduplicateSeen[ev.Flag.Key] = true\n\t\t\t\t}\n".to_string()
        } else {
            let negate_keys = strict_negate_keys.join(", ");
            format!(
                "\t\t\t\tswitch ev.Flag.Key {{\n\t\t\t\tcase {negate_keys}:\n\t\t\t\t\tpolarity := uint8(1)\n\t\t\t\t\tif ev.Negated {{\n\t\t\t\t\t\tpolarity = 2\n\t\t\t\t\t}}\n\t\t\t\t\tif polaritySeen[ev.Flag.Key]&polarity != 0 {{\n\t\t\t\t\t\tduplicateSeen[ev.Flag.Key] = true\n\t\t\t\t\t}}\n\t\t\t\t\tpolaritySeen[ev.Flag.Key] |= polarity\n\t\t\t\tdefault:\n\t\t\t\t\tlevelSeen[ev.Flag.Key]++\n\t\t\t\t\tif levelSeen[ev.Flag.Key] > 1 {{\n\t\t\t\t\t\tduplicateSeen[ev.Flag.Key] = true\n\t\t\t\t\t}}\n\t\t\t\t}}\n"
            )
        };
        format!(
            "\t\t\tswitch ev.Flag.Key {{\n\t\t\tcase {keys}:\n\t\t\t\tstrictSeen[ev.Flag.Key] = true\n{per_kind}\t\t\t}}\n"
        )
    };
    let duplicate_occurrences = if strict_keys.is_empty() {
        ""
    } else {
        "\t\toccurrences := seen[key]\n\t\tif strictSeen[key] {\n\t\t\toccurrences = 1\n\t\t\tif duplicateSeen[key] {\n\t\t\t\toccurrences = 2\n\t\t\t}\n\t\t}\n"
    };
    let check_occurrences = if strict_keys.is_empty() {
        "seen[key]"
    } else {
        "occurrences"
    };
    let duplicate_command = if strict_keys.is_empty() {
        String::new()
    } else {
        let polarity = if strict_negate_keys.is_empty() {
            ""
        } else {
            "\t\t\tpolaritySeen = map[uint64]uint8{}\n"
        };
        format!("\t\t\tlevelSeen = map[uint64]int{{}}\n{polarity}")
    };
    let has_relationship_values = commands.iter().any(|command| {
        command.flags.iter().any(|(flag, _)| {
            !flag.requires_if.is_empty()
                || !flag.required_if_eq.is_empty()
                || !flag.required_if_eq_all.is_empty()
        }) || command
            .args
            .iter()
            .any(|(arg, _)| !arg.required_if_eq.is_empty() || !arg.required_if_eq_all.is_empty())
    });
    let has_default_if = commands
        .iter()
        .flat_map(|command| command.flags.iter())
        .any(|(flag, _)| !flag.default_if.is_empty());
    let needs_negated = has_relationship_values || has_default_if;
    let conditional_state = if needs_negated {
        "\tnegated := map[uint64]bool{}\n"
    } else {
        ""
    };
    let conditional_resolved = if has_relationship_values {
        "\tresolved := map[uint64][]string{}\n"
    } else {
        ""
    };
    let conditional_value = if has_relationship_values {
        "\t\tresolved[key] = values\n"
    } else {
        ""
    };
    let _ = writeln!(
        out,
        "// Parse binds a command line and fills the structs above.\n\
         //\n\
         // The rules decided once the last token has been read run here too, so a\n\
         // missing required flag or a value outside its choices comes back rather than\n\
         // reaching your code. A returned error is an *argv.Error; render it with\n\
         // argv.Render.\n\
         //\n\
         // Help and version arrive as errors, because a parse that stops to print a page\n\
         // has produced no value. Check the code before treating one as a failure.\n\
         func Parse(args []string) (*{}, error) {{",
        name(root)
    );
    let _ = writeln!(out, "\tout := &{}{{}}", name(root));

    // One variable per command, assigned on descent. A flag's key can only arrive
    // after its command was selected, so the variable is set by the time anything
    // reads it.
    for e in commands.iter().skip(1) {
        let _ = writeln!(out, "\tvar {}V *{}", e.named.var, name(e));
    }

    let _ = writeln!(
        out,
        "\n\t// Collected by key, so the post-binding rules can judge what arrived\n\
         \t// before any of it is handed back.\n\
         \tgiven := map[uint64][]string{{}}\n\
         \tseen := map[uint64]int{{}}\n\
         {duplicate_state}\
         {conditional_state}\
         \tchain := []*argv.Command{{Root}}\n\
         \n\tp := argv.New(Root, args)\n\
         \tfor p.Next() {{\n\
         \t\tev := p.Event()\n\
         \t\tswitch ev.Kind {{\n\
         \t\tcase argv.KindCommand:\n\
         {duplicate_command}\
         \t\t\tchain = append(chain, ev.Command)\n\
         \t\t\tswitch ev.Command.Key {{"
    );
    for (i, e) in commands.iter().enumerate().skip(1) {
        let owner = match parent_of(commands, i) {
            Some(0) | None => "out".to_string(),
            Some(at) => format!("{}V", commands[at].named.var),
        };
        let _ = writeln!(
            out,
            "\t\t\tcase {}:\n\t\t\t\t{v}V = &{}{{}}\n\t\t\t\t{owner}.{} = {v}V",
            e.named.key,
            name(e),
            assigned[&e.named.key],
            v = e.named.var
        );
    }

    let conditional_event = if needs_negated {
        "\t\t\tnegated[ev.Flag.Key] = ev.Negated\n"
    } else {
        ""
    };
    let _ = writeln!(
        out,
        "\t\t\t}}\n\t\tcase argv.KindFlag:\n\t\t\tseen[ev.Flag.Key]++\n\
         {duplicate_event}\
         {conditional_event}\
         \t\t\tif ev.HasValue {{\n\
         \t\t\t\tgiven[ev.Flag.Key] = append(given[ev.Flag.Key], argv.SplitValue(ev.Value, ev.Flag.Delimiter, true)...)\n\
         \t\t\t}} else if given[ev.Flag.Key] == nil {{\n\
         \t\t\t\t// Given without a value is still given, and nil would read as\n\
         \t\t\t\t// absent when the fallbacks are applied.\n\
         \t\t\t\tgiven[ev.Flag.Key] = []string{{}}\n\t\t\t}}\n\
         \t\t\tswitch ev.Flag.Key {{"
    );
    for e in commands {
        let owner = owner_of(e);
        for (flag, named) in &e.flags {
            let _ = writeln!(
                out,
                "\t\t\tcase {}:\n{}",
                named.key,
                flag_assign(flag, &owner, &assigned[&named.key])
            );
        }
    }

    let _ = writeln!(
        out,
        "\t\t\t}}\n\t\tcase argv.KindArg:\n\t\t\tseen[ev.Arg.Key]++\n\
         \t\t\tvalues := argv.SplitValue(ev.Value, ev.Arg.Delimiter, ev.Delimit)\n\
         \t\t\tgiven[ev.Arg.Key] = append(given[ev.Arg.Key], values...)\n\
         \t\t\tswitch ev.Arg.Key {{"
    );
    for e in commands {
        let owner = owner_of(e);
        for (arg, named) in &e.args {
            let field = &assigned[&named.key];
            let assign = if arg.var {
                format!("\t\t\t\t{owner}.{field} = append({owner}.{field}, values...)")
            } else {
                format!("\t\t\t\t{owner}.{field} = ev.Value")
            };
            let _ = writeln!(out, "\t\t\tcase {}:\n{assign}", named.key);
        }
    }

    let negated_arg = if needs_negated { "negated" } else { "nil" };
    let _ = writeln!(
        out,
        "\t\t\t}}\n\t\t}}\n\t}}\n\
         \tif err := p.Err(); err != nil {{\n\t\treturn nil, err\n\t}}\n\
         \tif p.Command().ArgRequiredElseHelp && p.CommandStart() == len(args) {{\n\
         \t\treturn nil, &argv.Error{{Code: argv.CodeHelp, Cmd: p.Command()}}\n\t}}\n\
         \n\t// Only the commands the words actually selected are judged: a required\n\
         \t// flag on a command nobody ran is not missing.\n\
         \tvar scope []uint64\n\
         \trequirements := map[uint64]bool{{}}\n\
         \tfor i, cmd := range chain {{\n\
         \t\tcheckRequirements := i == len(chain)-1 || !cmd.SubcommandNegatesReqs\n\
         \t\tfor _, f := range cmd.Flags {{\n\t\t\tscope = append(scope, f.Key)\n\t\t\trequirements[f.Key] = checkRequirements\n\t\t}}\n\
         \t\tfor _, a := range cmd.Args {{\n\t\t\tscope = append(scope, a.Key)\n\t\t\trequirements[a.Key] = checkRequirements\n\t\t}}\n\
         \t}}\n\
         \tsources := map[uint64]argv.Source{{}}\n\
         \tfilled := map[uint64][]string{{}}\n\
         {conditional_resolved}\
         \tfor _, key := range scope {{\n\
         \t\tvalues, source := argv.Fill(Meta.Lookup(key), given[key], argv.LookupEnv)\n\
         \t\tfilled[key] = values\n\
         \t\tsources[key] = source\n\
         \t}}\n\
         \targv.ApplyDefaultIf(Meta, scope, filled, sources, {negated_arg})\n\
         \tfor _, key := range scope {{\n\
         \t\tvalues, source := filled[key], sources[key]\n\
         {conditional_value}\
         {duplicate_occurrences}\
         \t\tentryMeta := Meta.Lookup(key)\n\
         \t\tif entryMeta != nil && !requirements[key] {{\n\t\t\tcopy := *entryMeta\n\t\t\tcopy.Required = false\n\t\t\tentryMeta = &copy\n\t\t}}\n\
         \t\tif err := argv.Check(entryMeta, values, {check_occurrences}); err != nil {{\n\
         \t\t\treturn nil, err\n\t\t}}\n\
         \t\t// What the environment or a default supplied has to reach the field\n\
         \t\t// too. A front door that enforces a default and then hands back the\n\
         \t\t// zero value is worse than one that has no defaults at all.\n\
         \t\t//\n\
         \t\t// Written here rather than in a function of its own because a\n\
         \t\t// subcommand's struct is reachable only from inside this one: the\n\
         \t\t// variable holding it is local, and only the keys of commands the\n\
         \t\t// words selected are in scope, so it is never nil when its key is.\n\
         \t\tif source == argv.FromEnv || source == argv.FromDefault {{\n\
         \t\t\tswitch key {{"
    );
    fallback_cases(out, commands, assigned);
    let _ = writeln!(out, "\t\t\t}}\n\t\t}}\n\t}}");
    if has_relationship_values {
        let _ = writeln!(
            out,
            "\tif err := argv.CheckRelationshipsWithValuesAndRequirements(Meta, scope, func(k uint64) argv.Source {{\n\
             \t\treturn sources[k]\n\t}}, func(k uint64) []string {{\n\
             \t\treturn argv.RelationshipValues(Meta.Lookup(k), resolved[k], sources[k], negated[k])\n\
             \t}}, func(k uint64) bool {{ return requirements[k] }}); err != nil {{\n\t\treturn nil, err\n\t}}"
        );
    } else {
        let _ = writeln!(
            out,
            "\tif err := argv.CheckRelationshipsWithValuesAndRequirements(Meta, scope, func(k uint64) argv.Source {{\n\
             \t\treturn sources[k]\n\t}}, nil, func(k uint64) bool {{ return requirements[k] }}); err != nil {{\n\t\treturn nil, err\n\t}}"
        );
    }
    let _ = writeln!(out, "\treturn out, nil\n}}\n");
}

/// The cases that put an `env` or `default` value into its field.
fn fallback_cases(out: &mut String, commands: &[Emitted], assigned: &Fields) {
    for e in commands {
        let owner = owner_of(e);
        for (flag, named) in &e.flags {
            let field = &assigned[&named.key];
            let assign = match flag_type(flag) {
                // A value-less flag has nowhere to put text, so the variable is
                // read as a yes or a no — by usage-lib's allow-list, which is
                // narrower than Go's own spellings on purpose.
                "bool" => format!(
                    "\t\t\t\tif source == argv.FromEnv {{\n\
                     \t\t\t\t\t{owner}.{field} = argv.EnvTruth(values[0])\n\
                     \t\t\t\t}} else {{\n\t\t\t\t\t{owner}.{field} = values[0] == \"true\"\n\t\t\t\t}}"
                ),
                "[]string" => {
                    format!("\t\t\t\t{owner}.{field} = append({owner}.{field}, values...)")
                }
                // A count is occurrences, which nothing but the command line has.
                "int" => continue,
                _ => format!("\t\t\t\t{owner}.{field} = values[len(values)-1]"),
            };
            let _ = writeln!(out, "\t\t\tcase {}:\n{assign}", named.key);
        }
        for (arg, named) in &e.args {
            let field = &assigned[&named.key];
            let assign = if arg.var {
                format!("\t\t\t\t{owner}.{field} = append({owner}.{field}, values...)")
            } else {
                format!("\t\t\t\t{owner}.{field} = values[len(values)-1]")
            };
            let _ = writeln!(out, "\t\t\tcase {}:\n{assign}", named.key);
        }
    }
}

/// Where a command's own entries are assigned.
fn owner_of(e: &Emitted) -> String {
    if e.root {
        "out".to_string()
    } else {
        format!("{}V", e.named.var)
    }
}

/// The index of the command that declares this one as a subcommand.
fn parent_of(commands: &[Emitted], at: usize) -> Option<usize> {
    let path = &commands[at].cmd.full_cmd;
    if path.len() <= 1 {
        return Some(0);
    }
    commands
        .iter()
        .position(|e| e.cmd.full_cmd[..] == path[..path.len() - 1])
}

fn name(e: &Emitted) -> String {
    if e.root {
        return "Cli".to_string();
    }
    format!("{}Cmd", &e.named.key["Cmd".len()..])
}

fn flag_type(flag: &SpecFlag) -> &'static str {
    match () {
        _ if flag.count => "int",
        _ if flag.arg.is_none() => "bool",
        _ if flag.var || flag.arg.as_ref().is_some_and(|a| a.var) => "[]string",
        _ => "string",
    }
}

fn arg_type(arg: &SpecArg) -> &'static str {
    if arg.var {
        "[]string"
    } else {
        "string"
    }
}

fn flag_assign(flag: &SpecFlag, owner: &str, field: &str) -> String {
    match flag_type(flag) {
        // A count is the number of occurrences, which is what the parser reports
        // one event at a time.
        "int" => format!("\t\t\t\t{owner}.{field}++"),
        "bool" if flag.bool_value => format!(
            "\t\t\t\tif ev.HasValue {{\n\t\t\t\t\t{owner}.{field} = (ev.Value == \"true\") != ev.Negated\n\t\t\t\t}} else {{\n\t\t\t\t\t{owner}.{field} = !ev.Negated\n\t\t\t\t}}"
        ),
        "bool" => format!("\t\t\t\t{owner}.{field} = !ev.Negated"),
        "[]string" => format!(
            "\t\t\t\tif ev.HasValue {{\n\t\t\t\t\t{owner}.{field} = append({owner}.{field}, argv.SplitValue(ev.Value, ev.Flag.Delimiter, true)...)\n\t\t\t\t}}"
        ),
        _ => format!("\t\t\t\t{owner}.{field} = ev.Value"),
    }
}
