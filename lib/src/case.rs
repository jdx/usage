//! The identifier case conversions Usage needs when generating code and completions.
//!
//! This is intentionally smaller than a general-purpose casing crate: Usage only emits
//! snake_case, lowerCamelCase, and PascalCase identifiers. The word-boundary rules follow
//! `heck` so generated APIs do not change when the dependency is removed.

#[derive(Clone, Copy, PartialEq)]
enum WordMode {
    Boundary,
    Lowercase,
    Uppercase,
}

fn words(input: &str) -> Vec<&str> {
    let mut words = Vec::new();
    for segment in input.split(|c: char| !c.is_alphanumeric()) {
        let mut chars = segment.char_indices().peekable();
        let mut start = 0;
        let mut mode = WordMode::Boundary;

        while let Some((index, current)) = chars.next() {
            if let Some(&(next_index, next)) = chars.peek() {
                let next_mode = if current.is_lowercase() {
                    WordMode::Lowercase
                } else if current.is_uppercase() {
                    WordMode::Uppercase
                } else {
                    mode
                };

                if next_mode == WordMode::Lowercase && next.is_uppercase() {
                    words.push(&segment[start..next_index]);
                    start = next_index;
                    mode = WordMode::Boundary;
                } else if mode == WordMode::Uppercase
                    && current.is_uppercase()
                    && next.is_lowercase()
                {
                    if start != index {
                        words.push(&segment[start..index]);
                    }
                    start = index;
                    mode = WordMode::Boundary;
                } else {
                    mode = next_mode;
                }
            } else if start < segment.len() {
                words.push(&segment[start..]);
            }
        }
    }
    words
}

fn lowercase(word: &str) -> String {
    let mut output = String::new();
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        if c == 'Σ' && chars.peek().is_none() {
            output.push('ς');
        } else {
            output.extend(c.to_lowercase());
        }
    }
    output
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut output = first.to_uppercase().collect::<String>();
    output.push_str(&lowercase(chars.as_str()));
    output
}

pub(crate) fn snake(input: &str) -> String {
    words(input)
        .into_iter()
        .map(lowercase)
        .collect::<Vec<_>>()
        .join("_")
}

pub(crate) fn pascal(input: &str) -> String {
    words(input).into_iter().map(capitalize).collect()
}

pub(crate) fn lower_camel(input: &str) -> String {
    let mut words = words(input).into_iter();
    let mut output = words.next().map(lowercase).unwrap_or_default();
    output.extend(words.map(capitalize));
    output
}

pub(crate) trait ToSnakeCase {
    fn to_snake_case(&self) -> String;
}

impl ToSnakeCase for str {
    fn to_snake_case(&self) -> String {
        snake(self)
    }
}

pub(crate) struct AsSnakeCase<T: AsRef<str>>(pub T);
pub(crate) struct AsPascalCase<T: AsRef<str>>(pub T);
pub(crate) struct AsLowerCamelCase<T: AsRef<str>>(pub T);

impl<T: AsRef<str>> std::fmt::Display for AsSnakeCase<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&snake(self.0.as_ref()))
    }
}

impl<T: AsRef<str>> std::fmt::Display for AsPascalCase<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&pascal(self.0.as_ref()))
    }
}

impl<T: AsRef<str>> std::fmt::Display for AsLowerCamelCase<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&lower_camel(self.0.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_heck_word_boundaries() {
        assert_eq!(snake("MixedUP CamelCase"), "mixed_up_camel_case");
        assert_eq!(snake("XMLHttpRequest"), "xml_http_request");
        assert_eq!(pascal("SHOUTY_SNAKE_CASE"), "ShoutySnakeCase");
        assert_eq!(lower_camel("XMLHttpRequest"), "xmlHttpRequest");
    }
}
