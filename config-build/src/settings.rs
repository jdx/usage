//! The settings struct a CLI actually reads.
//!
//! The registry is what resolution needs; this is what the code needs. Between them they are the
//! whole reason the fleet's hand-written key↔field match arms exist — pitchfork writes about three
//! hundred and thirty lines of them, twice, and five settings still cannot be reached from its own
//! `settings get`.
//!
//! Dotted keys become nested structs, because `settings.task.output` is how the setting reads in
//! the file and there is no reason for the code to spell it differently.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use usage::spec::config::SpecConfigProp;
use usage::spec::config_type::{Base, SpecConfigType};

/// A group of settings under a common dotted prefix, which becomes one struct.
#[derive(Default)]
struct Group<'a> {
    /// Settings whose key ends here, by their last segment.
    leaves: BTreeMap<&'a str, Leaf<'a>>,
    /// Groups below this one, by their segment.
    groups: BTreeMap<&'a str, Group<'a>>,
}

struct Leaf<'a> {
    /// The whole dotted key.
    key: &'a str,
    /// The const in `prop`, and the local this is read into.
    ident: &'a str,
    prop: &'a SpecConfigProp,
}

/// The `Settings` struct, its nested structs, and the one function that reads them.
pub(crate) fn settings(
    props: &[(&String, &SpecConfigProp)],
    idents: &[String],
    problems: &mut Vec<String>,
) -> String {
    let mut root = Group::default();
    for ((key, prop), ident) in props.iter().zip(idents) {
        // An old name is not a field: every read of it folds to the setting that replaced it, so a
        // field for it would be a second name for one value — and `config set old.key` writing to a
        // field nothing reads is exactly the pitchfork bug.
        if prop.renamed_to.is_some() {
            continue;
        }
        // A key with a piece that cannot be a name is already refused by name, and building a tree
        // from it invents a group called nothing that then collides with its own parent — two
        // messages for one mistake, one of them about something the author never wrote.
        if !crate::emit::nameable(key) {
            continue;
        }
        let segments: Vec<&str> = key.split('.').collect();
        let (last, path) = segments.split_last().expect("a key has a segment");
        let mut group = &mut root;
        for segment in path {
            group = group.groups.entry(segment).or_default();
        }
        group.leaves.insert(last, Leaf { key, ident, prop });
    }

    let mut structs: BTreeMap<String, String> = BTreeMap::new();
    check_names(&root, "", "Settings", &mut structs, problems);

    let mut structs = String::new();
    let mut reads = String::new();
    let root_body = emit(&root, "Settings", &mut structs, &mut reads);

    format!(
        "\n/// Every setting, as the types a CLI holds them in.\n\
         ///\n\
         /// Read with [`Settings::read`] from a resolution, which is the only way to build one:\n\
         /// the values, and every reason one could not be read, come from the merge.\n\
         #[derive(Debug, Clone, PartialEq)]\n\
         pub struct Settings {{\n{root_body}}}\n\
         {structs}\n\
         impl Settings {{\n    \
         /// This resolution's values, as their types.\n    \
         ///\n    \
         /// Every setting is read before anything is returned, so the error is the whole list of\n    \
         /// what is wrong rather than the first thing found.\n    \
         pub fn read(\n        \
         resolved: &::usage_config::Resolved,\n    \
         ) -> ::std::result::Result<Self, ::usage_config::ReadErrors> {{\n        \
         let mut fold = resolved.fold();\n{reads}        \
         fold.finish()?;\n        \
         ::std::result::Result::Ok(Self {{\n{}        }})\n    }}\n}}\n",
        construct(&root, "Settings", 3),
    )
}

/// One group's fields, its descendants' structs, and the reads that fill them.
fn emit(group: &Group<'_>, name: &str, structs: &mut String, reads: &mut String) -> String {
    let mut fields = String::new();
    for (segment, sub) in &group.groups {
        let sub_name = format!("{name}{}", camel(segment));
        let body = emit(sub, &sub_name, structs, reads);
        let _ = write!(
            structs,
            "\n/// The `{}` settings.\n\
             #[derive(Debug, Clone, PartialEq)]\n\
             pub struct {sub_name} {{\n{body}}}\n",
            crate::emit::one_line(segment)
        );
        let _ = writeln!(fields, "    pub {}: {sub_name},", field(segment));
    }
    for (segment, leaf) in &group.leaves {
        let ty = rust_ty(leaf);
        if let Some(help) = leaf.prop.help.as_deref() {
            let _ = writeln!(fields, "    /// {}", one_line(help));
        }
        // The key as well as the help, because the field name is a translation of it and a reader
        // going from code to a config file needs the name the file uses.
        let _ = writeln!(fields, "    /// (`{}`)", crate::emit::one_line(leaf.key));
        if optional(leaf) {
            let _ = writeln!(fields, "    pub {}: Option<{ty}>,", field(segment));
            let _ = writeln!(
                reads,
                "        let {}: Option<{ty}> = fold.optional(prop::{});",
                local(leaf.ident),
                leaf.ident
            );
        } else {
            let _ = writeln!(fields, "    pub {}: {ty},", field(segment));
            let _ = writeln!(
                reads,
                "        let {}: Option<{ty}> = fold.required(prop::{});",
                local(leaf.ident),
                leaf.ident
            );
        }
    }
    fields
}

/// The expression that builds one group, once the fold has proved every value read.
fn construct(group: &Group<'_>, name: &str, depth: usize) -> String {
    let pad = "    ".repeat(depth);
    let mut out = String::new();
    for (segment, sub) in &group.groups {
        // Named, not inferred: a nested group is its own struct, and the name is the same one its
        // declaration got.
        let sub_name = format!("{name}{}", camel(segment));
        let _ = writeln!(out, "{pad}{}: {sub_name} {{", field(segment));
        let _ = write!(out, "{}", construct(sub, &sub_name, depth + 1));
        let _ = writeln!(out, "{pad}}},");
    }
    for (segment, leaf) in &group.leaves {
        if optional(leaf) {
            let name = field(segment);
            let read_into = local(leaf.ident);
            // `ci` rather than `ci: ci` where the two agree, which they do for every setting whose
            // key has one segment: clippy will not have the long form, and nor should it.
            if name == read_into {
                let _ = writeln!(out, "{pad}{name},");
            } else {
                let _ = writeln!(out, "{pad}{name}: {read_into},");
            }
        } else {
            // The unwrap the fold's own contract allows: `required` returns `None` only when it has
            // recorded an error, and `finish` has already turned any error into a return.
            // The message is a literal in generated code, so the key goes through the same escaper
            // `PropMeta` uses: a key holding a quote or a backslash — `a"b` is a nameable key —
            // otherwise ended the literal early and the rest of it read as code.
            let message = crate::emit::rust_str(&format!(
                "`{}` has a declared default, so the fold has already reported any absence",
                leaf.key
            ));
            let _ = writeln!(
                out,
                "{pad}{}: {}.expect({message}),",
                field(segment),
                local(leaf.ident),
            );
        }
    }
    out
}

/// Whether a field holds an `Option`.
///
/// Two ways to be optional and they mean the same thing to the code: the spec said `option<T>`, or
/// nothing declared a default, in which case the resolution can perfectly well come back with no
/// value. Writing the field as `T` would mean a failure at run time for a shape the spec allows.
fn optional(leaf: &Leaf<'_>) -> bool {
    if let Some(optional) = leaf.prop.optional {
        return optional;
    }
    leaf.prop.default.is_none() && leaf.prop.default_list.is_empty()
        || leaf
            .prop
            .value_type
            .as_ref()
            .is_some_and(SpecConfigType::is_optional)
}

/// The Rust type for one setting, without the `Option` around it.
fn rust_ty(leaf: &Leaf<'_>) -> String {
    fn of(ty: &SpecConfigType) -> String {
        match ty {
            SpecConfigType::Base(base) => base_ty(base).to_string(),
            // A `set` is a `Vec` too: the merge has already dropped duplicates, and a list keeps
            // the order the files were read in — which for a `PATH`-like setting is the meaning.
            SpecConfigType::List(inner) | SpecConfigType::Set(inner) => {
                format!("Vec<{}>", of(inner))
            }
            SpecConfigType::Map(_, value) => {
                format!("::std::collections::BTreeMap<String, {}>", of(value))
            }
            SpecConfigType::Option(inner) => of(inner),
            // A union is not a Rust type. The value arrives as it was written and the CLI decides,
            // which is what declaring a union asked for.
            SpecConfigType::Union(_) => "::usage_config::Value".to_string(),
        }
    }
    let Some(declared) = leaf.prop.value_type.as_ref() else {
        return "String".to_string();
    };
    // Every `Value` this can produce is one the spec asked for: an `object` says its keys are not
    // described, a union says usage cannot decide, and a name usage does not know says the same. So
    // there is nothing to refuse here — the type is as narrow as the declaration was.
    of(declared)
}

fn base_ty(base: &Base) -> &'static str {
    match base {
        Base::Bool => "bool",
        Base::Int => "i64",
        Base::Uint => "u64",
        Base::Float => "f64",
        Base::String => "String",
        Base::Path => "::std::path::PathBuf",
        // Read as text, both of them. What makes a string a URL is what the CLI does with it, and
        // the crate that owns the duration type owns its spelling — inventing one here would put a
        // dependency in every adopter's binary for a value they may only ever print.
        Base::Url | Base::Duration => "String",
        // A table whose keys the spec does not describe, and a name usage does not know: the value
        // as it was written.
        Base::Object | Base::Custom(_) => "::usage_config::Value",
    }
}

/// `task` → `Task`, for a nested struct's name.
///
/// ASCII only, like every other name here: a Rust identifier is not "any alphanumeric character" —
/// `½` is one and is not allowed in an identifier, and a Unicode digit cannot start one — and the
/// consts in `prop` are built from ASCII already, so keeping more here made one key produce names
/// from two alphabets, one of which the adopter could not compile.
fn camel(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    let mut upper = true;
    for c in segment.chars() {
        if !c.is_ascii_alphanumeric() {
            upper = true;
            continue;
        }
        if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// A key segment as a field name.
fn field(segment: &str) -> String {
    let mut name: String = segment
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    // A field cannot start with a digit any more than a const can, and `2fa` is a real name for a
    // real setting. Prefixed the same way, so the two spellings of one key agree.
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    raw(&name)
}

/// The local a setting is read into.
///
/// Prefixed, and that is the whole point: unprefixed, a setting called `fold` generated
/// `let fold = fold.optional(…)` and shadowed the reader's own fold, so every read after it — and
/// `fold.finish()` — stopped compiling. A setting called `SELF` generated `let self`. The prefix
/// goes on every local, so no name a spec can choose reaches either state, and since the consts it
/// is built from are unique, the locals are too.
fn local(ident: &str) -> String {
    format!("read_{}", ident.to_lowercase())
}

/// `name`, spelled so it can be an identifier.
///
/// A `let` binding needs this as much as a field does: `match` is an unremarkable name for a
/// setting, and `let match: Option<String>` is not Rust.
fn raw(name: &str) -> String {
    if RAW_ALLOWED.contains(&name) {
        return format!("r#{name}");
    }
    name.to_string()
}

/// Keywords a field can still be named, spelled `r#`.
///
/// The four that cannot — `self`, `crate`, `super` and `Self` — are refused by name in
/// [`check_names`], because there is no way to write them.
const RAW_ALLOWED: [&str; 49] = [
    "as",
    "break",
    "const",
    "continue",
    "else",
    "enum",
    "extern",
    "false",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "static",
    "struct",
    "trait",
    "true",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
    "async",
    "await",
    "dyn",
    "abstract",
    "become",
    "box",
    "do",
    "final",
    "macro",
    "override",
    "priv",
    "typeof",
    "unsized",
    "virtual",
    "yield",
    "try",
    "gen",
    "macro_rules",
];

/// Keywords no identifier can be, raw or otherwise.
const NEVER_ALLOWED: [&str; 4] = ["self", "crate", "super", "Self"];

/// Refuse the names that cannot become fields, and the ones that would collide.
///
/// Checked on the names that are actually *emitted*, not on the key segments they come from: two
/// segments can differ and still translate to one field (`foo-bar` beside `foo_bar`), and two groups
/// can differ and still translate to one struct (`a` beside `A`, since a type name loses the case
/// of its first letter). Comparing the segments, as this did, refused neither — and the collision
/// surfaced as duplicate items in a file the adopter did not write. The `prop::` consts do not catch
/// these either: `foo-bar.x` and `foo_bar.y` make two distinct consts and one field.
fn check_names(
    group: &Group<'_>,
    path: &str,
    name: &str,
    structs: &mut BTreeMap<String, String>,
    problems: &mut Vec<String>,
) {
    // Fields are per level: each group is its own struct, so two groups may both have a `timeout`.
    // Every entry remembers whether it came from a setting or from a group, because a setting and a
    // group wanting one name is a mistake worth its own words.
    let mut fields: BTreeMap<String, (String, bool)> = BTreeMap::new();

    for (segment, leaf) in &group.leaves {
        check_segment(segment, leaf.key, problems);
        if let Some((other, _)) = fields.insert(field(segment), (leaf.key.to_string(), true)) {
            collide(&other, leaf.key, &field(segment), problems);
        }
    }
    for (segment, sub) in &group.groups {
        let key = format!("{path}{segment}");
        check_segment(segment, &key, problems);
        match fields.insert(field(segment), (key.clone(), false)) {
            // `python` as a setting and `python.compile` as another: one field name with two things
            // to be, a value and a table. The spec can say it and no struct can hold it.
            Some((setting, true)) => problems.push(format!(
                "`{setting}` is a setting and a group of settings: it cannot be both a value and \
                 a table"
            )),
            Some((other, false)) => collide(&other, &key, &field(segment), problems),
            None => {}
        }
        // Struct names are *not* per level. Every one of them is declared in the same module and
        // built by concatenation, so `http.client.x` and `http_client.y` both arrive at
        // `SettingsHttpClient` from different depths — compared among siblings, as this was, neither
        // was refused and the adopter's crate got two structs with one name.
        let sub_name = format!("{name}{}", camel(segment));
        if let Some(other) = structs.insert(sub_name.clone(), key.clone()) {
            problems.push(format!(
                "`{other}` and `{key}` are both groups named `{sub_name}`: rename one of them"
            ));
        }
        check_names(sub, &format!("{key}."), &sub_name, structs, problems);
    }
}

/// Whether one key segment can be a field at all.
fn check_segment(segment: &str, of: &str, problems: &mut Vec<String>) {
    if NEVER_ALLOWED.contains(&segment) {
        problems.push(format!(
            "`{of}` cannot be a field: `{segment}` is a keyword Rust has no spelling for"
        ));
    }
}

fn collide(one: &str, other: &str, name: &str, problems: &mut Vec<String>) {
    problems.push(format!(
        "`{one}` and `{other}` both generate the field `{name}`: rename one of them"
    ));
}

/// Help text as one line, for a doc comment.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}
