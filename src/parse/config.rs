use crate::error::UsageErr;
use crate::{parse_bail, Spec};
use kdl::KdlNode;
use miette::NamedSource;
use std::ops::Deref;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct SpecConfigProp {
    name: String,
    value: String,
}

#[derive(Debug, Default)]
pub struct SpecConfig {
    props: Vec<SpecConfigProp>,
}

impl TryFrom<&KdlNode> for SpecConfig {
    type Error = UsageErr;
    fn try_from(doc: &KdlNode) -> Result<Self, Self::Error> {
        let mut config = Self::default();
        if let Some(children) = doc.children().map(|doc| doc.nodes()) {
            for node in children {
                match node.name().value() {
                    "prop" => {
                        dbg!(node);
                        let name = node.get("name").unwrap().value().as_string().unwrap();
                        let value = node.get("value").unwrap().value().as_string().unwrap();
                        dbg!(name, value);
                        // config.props.push(SpecConfigProp { name, value });
                    }
                    k => parse_bail!(node.name(), "unsupported key {k}"),
                }
            }
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        let spec: Spec = r#"
config {
    prp "color" default=true
    prop "yes" default=false
}
        "#
        .parse()
        .unwrap();
        assert_display_snapshot!(spec, @r###"
        "###);
    }
}
