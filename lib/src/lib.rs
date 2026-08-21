#[cfg(test)]
extern crate insta;
extern crate log;

pub use crate::parse::{available_flags, parse, Parser};
pub use crate::spec::arg::{SpecArg, SpecDoubleDashChoices, SpecRequiredIfEq};
pub use crate::spec::builder::{SpecArgBuilder, SpecCommandBuilder, SpecFlagBuilder};
pub use crate::spec::choices::{SpecChoice, SpecChoiceAlias, SpecChoices};
pub use crate::spec::cmd::SpecCommand;
pub use crate::spec::complete::SpecComplete;
pub use crate::spec::effect::SpecCommandEffect;
pub use crate::spec::flag::{SpecDefaultIf, SpecFlag, SpecFlagAction, SpecRequiresIf};
pub use crate::spec::group::SpecGroup;
pub use crate::spec::mount::SpecMount;
pub use crate::spec::unknown_flags::UnknownFlags;
pub use crate::spec::view::SpecView;
pub use crate::spec::Spec;
pub use crate::warn::{Warning, WarningKind};

#[macro_use]
#[allow(unused_assignments)] // Fields in struct variants are read by derive macros
pub mod error;
#[macro_use]
pub mod macros;
pub mod complete;
pub mod spec;
pub use error::Result;

#[cfg(feature = "docs")]
pub mod docs;
pub mod go;
pub mod parse;
pub mod sdk;
pub mod sh;
pub(crate) mod string;
#[cfg(test)]
mod test;
pub mod warn;
