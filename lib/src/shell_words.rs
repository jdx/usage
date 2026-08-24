//! Minimal POSIX shell word splitting and quoting used by specs and parser output.
//!
//! Adapted from `shell-words` 1.1.1. Keeping this narrow implementation in-tree avoids
//! making every `usage-lib` consumer compile a crate for four internal call sites.

use std::borrow::Cow;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError;

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("missing closing quote")
    }
}

impl std::error::Error for ParseError {}

enum State {
    Delimiter,
    Backslash,
    Unquoted,
    UnquotedBackslash,
    SingleQuoted,
    DoubleQuoted,
    DoubleQuotedBackslash,
    Comment,
}

pub fn split(input: &str) -> Result<Vec<String>, ParseError> {
    use State::*;

    let mut words = Vec::new();
    let mut word = String::new();
    let mut chars = input.chars();
    let mut state = Delimiter;

    loop {
        let current = chars.next();
        state = match state {
            Delimiter => match current {
                None => break,
                Some('\'') => SingleQuoted,
                Some('"') => DoubleQuoted,
                Some('\\') => Backslash,
                Some('\t' | ' ' | '\n') => Delimiter,
                Some('#') => Comment,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            Backslash => match current {
                None => {
                    word.push('\\');
                    words.push(std::mem::take(&mut word));
                    break;
                }
                Some('\n') => Delimiter,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            Unquoted => match current {
                None => {
                    words.push(std::mem::take(&mut word));
                    break;
                }
                Some('\'') => SingleQuoted,
                Some('"') => DoubleQuoted,
                Some('\\') => UnquotedBackslash,
                Some('\t' | ' ' | '\n') => {
                    words.push(std::mem::take(&mut word));
                    Delimiter
                }
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            UnquotedBackslash => match current {
                None => {
                    word.push('\\');
                    words.push(std::mem::take(&mut word));
                    break;
                }
                Some('\n') => Unquoted,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            SingleQuoted => match current {
                None => return Err(ParseError),
                Some('\'') => Unquoted,
                Some(c) => {
                    word.push(c);
                    SingleQuoted
                }
            },
            DoubleQuoted => match current {
                None => return Err(ParseError),
                Some('"') => Unquoted,
                Some('\\') => DoubleQuotedBackslash,
                Some(c) => {
                    word.push(c);
                    DoubleQuoted
                }
            },
            DoubleQuotedBackslash => match current {
                None => return Err(ParseError),
                Some('\n') => DoubleQuoted,
                Some(c @ ('$' | '`' | '"' | '\\')) => {
                    word.push(c);
                    DoubleQuoted
                }
                Some(c) => {
                    word.push('\\');
                    word.push(c);
                    DoubleQuoted
                }
            },
            Comment => match current {
                None => break,
                Some('\n') => Delimiter,
                Some(_) => Comment,
            },
        };
    }

    Ok(words)
}

fn quote(word: &str) -> Cow<'_, str> {
    if !word.is_empty()
        && !word.chars().any(|c| {
            matches!(
                c,
                '\n' | '\''
                    | '|'
                    | '&'
                    | ';'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | '$'
                    | '`'
                    | '\\'
                    | '"'
                    | ' '
                    | '\t'
                    | '*'
                    | '?'
                    | '['
                    | '#'
                    | '~'
                    | '='
                    | '%'
            )
        })
    {
        return Cow::Borrowed(word);
    }

    let mut quoted = String::with_capacity(word.len() + 2);
    quoted.push('\'');
    for c in word.chars() {
        if c == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(c);
        }
    }
    quoted.push('\'');
    Cow::Owned(quoted)
}

pub fn join<I, S>(words: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    words
        .into_iter()
        .map(|word| quote(word.as_ref()).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_join_shell_words() {
        let words = split("one 'two three' four\\ five # ignored").unwrap();
        assert_eq!(words, ["one", "two three", "four five"]);
        assert_eq!(split(&join(&words)).unwrap(), words);
        assert!(split("'unterminated").is_err());
    }
}
