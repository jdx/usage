use crate::error::UsageErr;
use kdl::KdlNode;

#[derive(Debug, Default)]
pub struct SpecConfig {}

impl TryFrom<&KdlNode> for SpecConfig {
    type Error = UsageErr;
    fn try_from(node: &KdlNode) -> Result<Self, Self::Error> {
        let mut config = Self::default();
        for entry in node.entries() {
            match entry.name().unwrap().value() {
                "prop" => {
                    todo!()
                }
                _ => unimplemented!("config entry"),
            }
        }
        Ok(config)
    }
}
