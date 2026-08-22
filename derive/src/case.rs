//! Case conversion for `rename_all`, in the shapes clap's derive produces.
//!
//! Replaces `heck`, so the only third-party crates an adopter compiles for the derive are
//! `proc-macro2`, `quote` and `syn` — the ones a proc macro cannot do without. Same reasoning
//! as `crate_name.rs`, which replaced `proc-macro-crate`.
//!
//! usage's `rename_all` vocabulary is a clone of `clap_derive`'s, and `clap_derive` uses heck.
//! So this reproduces heck 0.5's word boundaries rather than something merely reasonable: a
//! declaration ported from clap has to yield the same flag names, or the port silently changes
//! a CLI's interface. `tests::matches_heck_for_every_short_string` checks that against heck
//! itself rather than against what anyone here believes heck does.
//!
//! This is *not* the default naming policy. With no `rename_all`, names come from
//! `model::to_kebab`, which breaks before every uppercase char: `HTTPServer` is
//! `h-t-t-p-server` there and `http-server` here. Both spellings are pinned by
//! `model::tests::a_value_enum_supports_container_casing`. Do not unify them.

/// The words in `s`, as heck segments them.
///
/// Boundaries fall between non-alphanumeric runs, after a lowercase char followed by an
/// uppercase one, and before the last uppercase of a run that is followed by a lowercase one —
/// so `XMLHttpRequest` is `XML|Http|Request`. Digits are alphanumeric but neither upper nor
/// lower, so they belong to the word on their left and leave the case state alone: `Foo2Bar` is
/// `Foo2|Bar`, `Foo2bar` is one word.
///
/// heck streams words into a `Formatter` and writes a separator before each one after the
/// first. Collecting them is the same output, not an approximation: heck never emits an empty
/// word, so there is no case where "join with the separator" and "write a separator first"
/// disagree.
fn words(s: &str) -> Vec<&str> {
    /// The case of the last cased char seen since the current word began.
    #[derive(PartialEq, Eq, Clone, Copy)]
    enum Mode {
        Boundary,
        Lowercase,
        Uppercase,
    }

    let mut words = Vec::new();
    for chunk in s.split(|c: char| !c.is_alphanumeric()) {
        let mut chars = chunk.char_indices().peekable();
        let mut init = 0;
        let mut mode = Mode::Boundary;

        while let Some((i, c)) = chars.next() {
            let Some(&(next_i, next)) = chars.peek() else {
                // The last char of the chunk closes whatever word is open.
                words.push(&chunk[init..]);
                break;
            };
            // What the mode becomes if `c` does not end a word. A caseless char — a digit —
            // inherits it, which is what keeps `HTTP2Server` splitting as `HTTP2|Server`.
            let next_mode = if c.is_lowercase() {
                Mode::Lowercase
            } else if c.is_uppercase() {
                Mode::Uppercase
            } else {
                mode
            };

            if next_mode == Mode::Lowercase && next.is_uppercase() {
                // lower then upper: the boundary is after `c`.
                words.push(&chunk[init..next_i]);
                init = next_i;
                mode = Mode::Boundary;
            } else if mode == Mode::Uppercase && c.is_uppercase() && next.is_lowercase() {
                // The tail of an uppercase run, followed by lowercase: `c` starts the next
                // word, so the boundary is before it.
                words.push(&chunk[init..i]);
                init = i;
                mode = Mode::Boundary;
            } else {
                mode = next_mode;
            }
        }
    }
    words
}

/// Lowercase `word`, with heck's Greek final-sigma rule.
///
/// A word-final `Σ` lowercases to `ς` rather than `σ`. Kept because it is heck's, and a
/// divergence here would be one nobody could explain later — heck pins it in its own tests.
fn lower(word: &str) -> String {
    let mut out = String::with_capacity(word.len());
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'Σ' && chars.peek().is_none() {
            out.push('ς');
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

fn upper(word: &str) -> String {
    word.chars().flat_map(char::to_uppercase).collect()
}

/// Uppercase the first char of `word` and lowercase the rest.
fn capitalize(word: &str) -> String {
    let mut chars = word.char_indices();
    let Some((_, first)) = chars.next() else {
        return String::new();
    };
    let mut out: String = first.to_uppercase().collect();
    if let Some((i, _)) = chars.next() {
        out.push_str(&lower(&word[i..]));
    }
    out
}

fn join(s: &str, separator: &str, word: impl Fn(&str) -> String) -> String {
    words(s)
        .into_iter()
        .map(word)
        .collect::<Vec<_>>()
        .join(separator)
}

pub(crate) fn to_kebab_case(s: &str) -> String {
    join(s, "-", lower)
}

pub(crate) fn to_snake_case(s: &str) -> String {
    join(s, "_", lower)
}

pub(crate) fn to_shouty_snake_case(s: &str) -> String {
    join(s, "_", upper)
}

pub(crate) fn to_upper_camel_case(s: &str) -> String {
    join(s, "", capitalize)
}

pub(crate) fn to_lower_camel_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, word) in words(s).into_iter().enumerate() {
        if i == 0 {
            out.push_str(&lower(word));
        } else {
            out.push_str(&capitalize(word));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use heck::{
        ToKebabCase, ToLowerCamelCase, ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase as _,
    };

    /// Every conversion of one input, so a mismatch names the input once rather than five times.
    fn ours(s: &str) -> [String; 5] {
        [
            super::to_kebab_case(s),
            super::to_snake_case(s),
            super::to_shouty_snake_case(s),
            super::to_lower_camel_case(s),
            super::to_upper_camel_case(s),
        ]
    }

    fn hecks(s: &str) -> [String; 5] {
        [
            s.to_kebab_case(),
            s.to_snake_case(),
            s.to_shouty_snake_case(),
            s.to_lower_camel_case(),
            s.to_upper_camel_case(),
        ]
    }

    /// Exhaustive over the state machine rather than sampled.
    ///
    /// The segmenter's whole window is `(mode, char, next char)`, so every string up to length
    /// four over an alphabet that reaches each transition — a lowercase, two uppercase (for a
    /// run), a digit, two kinds of separator, and a char whose lowercasing is special — covers
    /// it completely. ~4700 inputs, and it runs in milliseconds.
    #[test]
    fn matches_heck_for_every_short_string() {
        const ALPHABET: [char; 8] = ['a', 'B', 'c', 'D', '0', '_', '-', 'Σ'];
        let mut inputs = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..4 {
            let mut next = Vec::with_capacity(frontier.len() * ALPHABET.len());
            for base in &frontier {
                for c in ALPHABET {
                    let mut candidate = base.clone();
                    candidate.push(c);
                    next.push(candidate);
                }
            }
            inputs.extend(next.iter().cloned());
            frontier = next;
        }
        for input in &inputs {
            assert_eq!(ours(input), hecks(input), "input {input:?}");
        }
    }

    /// The readable half of the pair: the names a declaration actually carries.
    #[test]
    fn matches_heck_for_realistic_names() {
        for input in [
            "",
            "a",
            "A",
            "T",
            "api_token",
            "type",
            "HTTPServer",
            "XMLHttpRequest",
            "OnePassword",
            "ApiServer",
            "onePassword",
            "ONE_PASSWORD",
            "IPv6Addr",
            "OAuth2Token",
            "PDFReader",
            "AWSConfigFile",
            "Utf8Path",
            "HTTP2Server",
            "Foo2Bar",
            "Foo2bar",
            "S3Bucket",
            "v2",
            "x509",
            "_leading",
            "trailing_",
            "__double__",
            "a__b",
            // `rename_all_env` re-cases a name that has already been renamed, so these are
            // inputs the derive really produces.
            "already-kebab",
            "API_SERVER",
            // And `id = "…"` / `name = "…"` are arbitrary strings, not identifiers.
            "service credential",
            "foo.bar",
            "Ünïcode",
            "XΣXΣ",
            "straße",
        ] {
            assert_eq!(ours(input), hecks(input), "input {input:?}");
        }
    }

    /// heck's own documented vectors, so a heck bump that changes what it means fails here
    /// rather than quietly making this module the odd one out.
    #[test]
    fn matches_hecks_own_vectors() {
        for input in [
            "CamelCase",
            "This is Human case.",
            "MixedUP CamelCase, with some Spaces",
            "mixed_up snake_case with some _spaces",
            "this-contains_ ALLKinds OfWord_Boundaries",
            "XΣXΣ baﬄe",
            "XMLHttpRequest",
            "ファイルを読み込み",
        ] {
            assert_eq!(ours(input), hecks(input), "input {input:?}");
        }
    }
}
