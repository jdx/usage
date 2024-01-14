use strum::EnumString;

#[derive(Debug, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum SpecDataTypes {
    Null,
    String,
    Integer,
    Float,
    Boolean,
}
