//! Resolve how the adopter depended on usage's runtime and derive crates.
//!
//! Cargo manifests are TOML; parse them rather than approximating the syntax so quoted values,
//! comments, inline tables, and dotted keys follow TOML rules.

use std::fs;
use std::path::PathBuf;
use toml::{Table, Value};

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
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").ok_or(())?);
    let text = fs::read_to_string(dir.join("Cargo.toml")).map_err(|_| ())?;
    let manifest = text.parse::<Table>().map_err(|_| ())?;
    if let Ok(found) = find_in_manifest(&manifest, package) {
        return Ok(found);
    }

    let inherited = workspace_dependency_keys(&manifest);
    if inherited.is_empty() {
        return Err(());
    }
    for ancestor in dir.ancestors() {
        let Ok(text) = fs::read_to_string(ancestor.join("Cargo.toml")) else {
            continue;
        };
        let Ok(workspace) = text.parse::<Table>() else {
            continue;
        };
        for key in &inherited {
            if workspace_dependency_resolves(&workspace, key, package) {
                return Ok(FoundCrate::Name(key.replace('-', "_")));
            }
        }
    }
    Err(())
}

fn find_in_manifest(manifest: &Table, package: &str) -> Result<FoundCrate, ()> {
    let pkg_name = manifest
        .get("package")
        .and_then(Value::as_table)
        .and_then(|table| table.get("name"))
        .and_then(Value::as_str)
        .ok_or(())?;
    if pkg_name == package {
        return Ok(FoundCrate::Itself);
    }

    for dependencies in dependency_tables(manifest) {
        for (key, value) in dependencies {
            if resolved_package(key, value) == package {
                return Ok(FoundCrate::Name(key.replace('-', "_")));
            }
        }
    }
    Err(())
}

fn dependency_tables(manifest: &Table) -> Vec<&Table> {
    let mut tables = Vec::new();
    for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(table) = manifest.get(name).and_then(Value::as_table) {
            tables.push(table);
        }
    }
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        for target in targets.values().filter_map(Value::as_table) {
            for name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(table) = target.get(name).and_then(Value::as_table) {
                    tables.push(table);
                }
            }
        }
    }
    tables
}

fn resolved_package<'a>(key: &'a str, value: &'a Value) -> &'a str {
    value
        .as_table()
        .and_then(|table| table.get("package"))
        .and_then(Value::as_str)
        .unwrap_or(key)
}

fn workspace_dependency_keys(manifest: &Table) -> Vec<String> {
    dependency_tables(manifest)
        .into_iter()
        .flat_map(|table| table.iter())
        .filter(|(_, value)| {
            value
                .as_table()
                .and_then(|table| table.get("workspace"))
                .and_then(Value::as_bool)
                == Some(true)
        })
        .map(|(key, _)| key.clone())
        .collect()
}

fn workspace_dependency_resolves(workspace: &Table, key: &str, package: &str) -> bool {
    workspace
        .get("workspace")
        .and_then(Value::as_table)
        .and_then(|table| table.get("dependencies"))
        .and_then(Value::as_table)
        .and_then(|table| table.get(key))
        .is_some_and(|value| resolved_package(key, value) == package)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Table {
        text.parse().unwrap()
    }

    #[test]
    fn finds_a_renamed_facade() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies]
usage = { package = "usage-rs", version = "5" }
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-rs"),
            Ok(FoundCrate::Name("usage".into()))
        );
    }

    #[test]
    fn finds_direct_argv() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies]
usage-argv = { path = "../argv", features = ["spec"] }
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-argv"),
            Ok(FoundCrate::Name("usage_argv".into()))
        );
    }

    #[test]
    fn itself_when_expanding_inside_the_package() {
        let manifest = parse(
            r#"
[package]
name = "usage-rs"
[dependencies]
usage-argv = { path = "../argv" }
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-rs"),
            Ok(FoundCrate::Itself)
        );
    }

    #[test]
    fn finds_multiline_rename_with_quoted_brace() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies]
usage = {
    path = "vendor/}foo",
    package = "usage-rs",
    version = "5",
}
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-rs"),
            Ok(FoundCrate::Name("usage".into()))
        );
    }

    #[test]
    fn finds_named_dependency_table() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies.usage-argv]
path = "../argv"
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-argv"),
            Ok(FoundCrate::Name("usage_argv".into()))
        );
    }

    #[test]
    fn finds_renamed_named_dependency_table() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies.usage]
package = "usage-rs"
version = "5"
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-rs"),
            Ok(FoundCrate::Name("usage".into()))
        );
    }

    #[test]
    fn finds_target_specific_dependency() {
        let manifest = parse(
            r#"
[package]
name = "app"
[target.'cfg(unix)'.dependencies]
usage = { package = "usage-rs", version = "5" }
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-rs"),
            Ok(FoundCrate::Name("usage".into()))
        );
    }

    #[test]
    fn keeps_dependency_declaration_order() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies]
z_usage = { package = "usage-rs", version = "6" }
a_usage = { package = "usage-rs", version = "6" }
"#,
        );
        assert_eq!(
            find_in_manifest(&manifest, "usage-rs"),
            Ok(FoundCrate::Name("z_usage".into()))
        );
    }

    #[test]
    fn prefers_nothing_when_absent() {
        let manifest = parse(
            r#"
[package]
name = "app"
[dependencies]
serde = "1"
"#,
        );
        assert!(find_in_manifest(&manifest, "usage-rs").is_err());
    }

    #[test]
    fn finds_workspace_inheritance_forms() {
        for member in [
            r#"
[package]
name = "app"
[dependencies]
usage = { workspace = true }
"#,
            r#"
[package]
name = "app"
[dependencies]
usage.workspace = true
"#,
            r#"
[package]
name = "app"
[dependencies.usage]
workspace = true
"#,
        ] {
            assert_eq!(workspace_dependency_keys(&parse(member)), ["usage"]);
        }

        let workspace = parse(
            r#"
[workspace]
[workspace.dependencies]
usage = { package = "usage-rs", version = "5" }
"#,
        );
        assert!(workspace_dependency_resolves(
            &workspace, "usage", "usage-rs"
        ));
    }

    #[test]
    fn finds_workspace_dependency_after_multiline_inline_table() {
        let workspace = parse(
            r#"
[workspace.dependencies]
some-crate = {
    path = "some-crate",
}
usage = { package = "usage-rs", version = "6" }
"#,
        );
        assert!(workspace_dependency_resolves(
            &workspace, "usage", "usage-rs"
        ));
    }

    #[test]
    fn finds_workspace_dependency_in_named_table() {
        let workspace = parse(
            r#"
[workspace.dependencies.usage]
version = "6"
package = "usage-rs"
[workspace.dependencies.other]
version = "1"
"#,
        );
        assert!(workspace_dependency_resolves(
            &workspace, "usage", "usage-rs"
        ));
    }

    #[test]
    fn ignores_braces_and_comments_in_quoted_values() {
        let workspace = parse(
            r#"
[workspace.dependencies]
other = {
    path = 'vendor/}#foo',
}
usage = { package = "usage-rs", version = "6" }
"#,
        );
        assert!(workspace_dependency_resolves(
            &workspace, "usage", "usage-rs"
        ));
    }
}
