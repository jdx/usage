use serde::Serialize;
use strum::{Display, EnumString};

/// The type a config property's values take.
///
/// `Display` is the KDL spelling, so a parsed spec can be written back out — without it
/// `data_type` was read and then silently dropped by the serializer, which is how a
/// `data_type="integer"` came back as `Null` after one round trip.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Display, EnumString, Serialize, Default)]
#[strum(serialize_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum SpecDataTypes {
    #[default]
    Null,
    String,
    Integer,
    Float,
    Boolean,
}
