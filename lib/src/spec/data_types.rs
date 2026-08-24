use serde::Serialize;

/// The type a config property's values take.
///
/// `Display` is the KDL spelling, so a parsed spec can be written back out — without it
/// `data_type` was read and then silently dropped by the serializer, which is how a
/// `data_type="integer"` came back as `Null` after one round trip.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpecDataTypes {
    #[default]
    Null,
    String,
    Integer,
    Float,
    Boolean,
}

impl_string_enum!(SpecDataTypes {
    SpecDataTypes::Null => "null",
    SpecDataTypes::String => "string",
    SpecDataTypes::Integer => "integer",
    SpecDataTypes::Float => "float",
    SpecDataTypes::Boolean => "boolean",
});
