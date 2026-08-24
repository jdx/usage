//! Manual string-enum implementations shared by the spec model.

pub(crate) trait StringEnum {
    const VARIANTS: &'static [&'static str];
}

macro_rules! impl_string_enum {
    ($type:ty { $($variant:path => $value:literal),+ $(,)? }) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let value = match self {
                    $($variant => $value),+
                };
                f.write_str(value)
            }
        }

        impl std::str::FromStr for $type {
            type Err = crate::error::EnumParseError;

            fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
                match value {
                    $($value => Ok($variant)),+,
                    _ => Err(crate::error::EnumParseError(value.to_string())),
                }
            }
        }

        impl crate::enum_value::StringEnum for $type {
            const VARIANTS: &'static [&'static str] = &[$($value),+];
        }
    };
}
