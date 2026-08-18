//! Resolve how the adopter depended on usage's runtime and derive crates.
//!
//! Replaces `proc-macro-crate` so the derive does not pull `toml_edit` and friends into
//! every adopter's compile. Only the dependency forms usage actually documents are
//! recognised: a bare key, a `{ package = "…", … }` rename, a multi-line `{ … }` table,
//! and a `[dependencies.foo]` header. That covers the facade alias, direct
//! `usage-argv` / `usage-derive`, and the mixed-dependency fixture.

use std::fs;
use std::path::PathBuf;

/// How the searched package appears in the adopter's crate graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FoundCrate {
    /// The adopter *is* that package (e.g. a derive inside `usage-rs` itself).
    Itself,
    /// The package is a dependency under this rustc crate name (hyphens already folded).
    Name(String),
}

/// Look up `package` in the crate currently being compiled.
pub fn crate_name(package: &str) -> Result<FoundCrate, ()> {
    let dir = std::env::var_os("CARGO_MANIFEST_DIR").ok_or(())?;
    let manifest = PathBuf::from(dir).join("Cargo.toml");
    let text = fs::read_to_string(manifest).map_err(|_| ())?;
    find_in_manifest(&text, package)
}

fn find_in_manifest(text: &str, package: &str) -> Result<FoundCrate, ()> {
    let pkg_name = package_name(text).ok_or(())?;
    if pkg_name == package {
        return Ok(FoundCrate::Itself);
    }

    let mut in_dependencies = false;
    // Open multi-line dependency table: key is the rustc rename, package field may follow.
    let mut open: Option<OpenTable> = None;

    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(header) = section_header(line) {
            if let Some(found) = finish_open(&mut open, package) {
                return Ok(found);
            }
            if let Some(key) = dependency_table_key(header) {
                // `[dependencies.usage-argv]` or `[dependencies.usage]` — body may set package.
                in_dependencies = true;
                open = Some(OpenTable {
                    key: key.to_string(),
                    package: None,
                });
                continue;
            }
            in_dependencies = is_dependencies_section(header);
            continue;
        }

        if !in_dependencies {
            continue;
        }

        if open.is_some() {
            // The body of a `{ … }` dependency, possibly ending on this line. Read a `package`
            // field from the part before any `}`, then close the table when the `}` arrives.
            let (fields, closed) = match line.split_once('}') {
                Some((before, _)) => (before, true),
                None => (line, false),
            };
            if let Some(pkg) = package_from_fields(fields) {
                if let Some(t) = open.as_mut() {
                    t.package = Some(pkg);
                }
            }
            if closed {
                if let Some(found) = finish_open(&mut open, package) {
                    return Ok(found);
                }
            }
            continue;
        }

        if let Some((key, value)) = inline_dependency(line) {
            if let Some(found) = match_dep(&key, value.as_deref(), package) {
                return Ok(found);
            }
            continue;
        }

        if let Some((key, package)) = multiline_table_start(line) {
            open = Some(OpenTable { key, package });
        }
    }

    finish_open(&mut open, package).ok_or(())
}

struct OpenTable {
    key: String,
    package: Option<String>,
}

fn finish_open(open: &mut Option<OpenTable>, wanted: &str) -> Option<FoundCrate> {
    let table = open.take()?;
    match_dep(&table.key, table.package.as_deref(), wanted)
}

fn match_dep(key: &str, package_field: Option<&str>, wanted: &str) -> Option<FoundCrate> {
    let resolved = package_field.unwrap_or(key);
    if resolved != wanted {
        return None;
    }
    Some(FoundCrate::Name(key.replace('-', "_")))
}

fn package_name(text: &str) -> Option<String> {
    let mut in_package = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = section_header(line) {
            in_package = header == "package";
            continue;
        }
        if in_package {
            if let Some(name) = string_assignment(line, "name") {
                return Some(name);
            }
        }
    }
    None
}

fn section_header(line: &str) -> Option<&str> {
    let line = line.trim();
    if !line.starts_with('[') || !line.ends_with(']') {
        return None;
    }
    Some(line[1..line.len() - 1].trim())
}

fn is_dependencies_section(header: &str) -> bool {
    matches!(
        header,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    ) || header.starts_with("target.")
        && (header.ends_with(".dependencies")
            || header.ends_with(".dev-dependencies")
            || header.ends_with(".build-dependencies"))
}

fn dependency_table_key(header: &str) -> Option<&str> {
    for prefix in ["dependencies.", "dev-dependencies.", "build-dependencies."] {
        if let Some(key) = header.strip_prefix(prefix) {
            if is_ident_key(key) {
                return Some(key);
            }
        }
    }
    None
}

fn inline_dependency(line: &str) -> Option<(String, Option<String>)> {
    let (key, rest) = split_assignment(line)?;
    if !is_ident_key(key) {
        return None;
    }
    let rest = rest.trim();
    if rest.starts_with('{') && rest.contains('}') {
        let package = inline_table_package(rest);
        return Some((key.to_string(), package));
    }
    if is_string_literal(rest) || rest.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Some((key.to_string(), None));
    }
    None
}

fn multiline_table_start(line: &str) -> Option<(String, Option<String>)> {
    let (key, rest) = split_assignment(line)?;
    if !is_ident_key(key) {
        return None;
    }
    let rest = rest.trim();
    if rest == "{" || (rest.starts_with('{') && !rest.contains('}')) {
        // Fields may already sit on the opening line, e.g. `usage = { package = "usage-rs",`.
        let body = rest.strip_prefix('{').unwrap_or(rest);
        let package = package_from_fields(body);
        return Some((key.to_string(), package));
    }
    None
}

/// Find `package = "…"` among comma-separated `key = value` fields, tolerating a trailing comma.
fn package_from_fields(body: &str) -> Option<String> {
    for part in body.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(pkg) = string_assignment(part, "package") {
            return Some(pkg);
        }
    }
    None
}

fn inline_table_package(table: &str) -> Option<String> {
    let mut body = table.trim();
    body = body.strip_prefix('{')?.trim();
    body = body.strip_suffix('}')?.trim();
    package_from_fields(body)
}

fn string_assignment(line: &str, field: &str) -> Option<String> {
    let (key, rest) = split_assignment(line)?;
    if key != field {
        return None;
    }
    parse_string(rest.trim())
}

fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let eq = line.find('=')?;
    let key = line[..eq].trim();
    let rest = line[eq + 1..].trim();
    Some((key, rest))
}

fn parse_string(value: &str) -> Option<String> {
    // A field inside an inline table carries its separator: `package = "usage-rs",`. Drop a
    // single trailing comma before matching the quotes.
    let value = value.trim().trim_end_matches(',').trim();
    if let Some(v) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return Some(v.to_string());
    }
    if let Some(v) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return Some(v.to_string());
    }
    None
}

fn is_string_literal(value: &str) -> bool {
    parse_string(value).is_some()
}

fn is_ident_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn strip_comment(line: &str) -> &str {
    // Dependency lines usage writes never put `#` inside a string; keep this dumb on purpose.
    line.split('#').next().unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_renamed_facade() {
        let manifest = r#"
[package]
name = "app"

[dependencies]
usage = { package = "usage-rs", version = "5" }
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Name("usage".into())
        );
    }

    #[test]
    fn finds_direct_argv() {
        let manifest = r#"
[package]
name = "app"
[dependencies]
usage-argv = { path = "../argv", features = ["spec"] }
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-argv").unwrap(),
            FoundCrate::Name("usage_argv".into())
        );
    }

    #[test]
    fn itself_when_expanding_inside_the_package() {
        let manifest = r#"
[package]
name = "usage-rs"
[dependencies]
usage-argv = { path = "../argv" }
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Itself
        );
    }

    #[test]
    fn finds_multiline_rename() {
        let manifest = r#"
[package]
name = "app"
[dependencies]
usage = {
  package = "usage-rs"
  version = "5"
}
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Name("usage".into())
        );
    }

    #[test]
    fn finds_multiline_rename_with_trailing_commas() {
        // Cargo's TOML 1.1 multi-line inline table: each field carries a trailing comma.
        let manifest = r#"
[package]
name = "app"
[dependencies]
usage = {
    package = "usage-rs",
    version = "5",
}
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Name("usage".into())
        );
    }

    #[test]
    fn finds_package_on_the_opening_brace_line() {
        let manifest = r#"
[package]
name = "app"
[dependencies]
usage = { package = "usage-rs",
          version = "5" }
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Name("usage".into())
        );
    }

    #[test]
    fn finds_package_sharing_the_closing_brace_line() {
        let manifest = r#"
[package]
name = "app"
[dependencies]
usage = {
  version = "5",
  package = "usage-rs" }
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Name("usage".into())
        );
    }

    #[test]
    fn finds_named_dependency_table() {
        let manifest = r#"
[package]
name = "app"
[dependencies.usage-argv]
path = "../argv"
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-argv").unwrap(),
            FoundCrate::Name("usage_argv".into())
        );
    }

    #[test]
    fn finds_renamed_named_dependency_table() {
        let manifest = r#"
[package]
name = "app"
[dependencies.usage]
package = "usage-rs"
version = "5"
"#;
        assert_eq!(
            find_in_manifest(manifest, "usage-rs").unwrap(),
            FoundCrate::Name("usage".into())
        );
    }

    #[test]
    fn prefers_nothing_when_absent() {
        let manifest = r#"
[package]
name = "app"
[dependencies]
serde = "1"
"#;
        assert!(find_in_manifest(manifest, "usage-rs").is_err());
    }
}
