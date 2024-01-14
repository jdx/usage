use serde::Serialize;
use strum::EnumString;

#[derive(Debug, Copy, Clone, EnumString, Serialize, Default)]
#[strum(serialize_all = "snake_case")]
pub enum SpecDataTypes {
    #[default]
    Null,
    String,
    Integer,
    Float,
    Boolean,
}
