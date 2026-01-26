#[cfg(test)]
extern crate insta;
extern crate log;

pub use crate::parse::{parse, Parser};
pub use crate::spec::arg::SpecArg;
pub use crate::spec::builder::{SpecArgBuilder, SpecCommandBuilder, SpecFlagBuilder};
pub use crate::spec::choices::SpecChoices;
pub use crate::spec::cmd::SpecCommand;
pub use crate::spec::complete::SpecComplete;
pub use crate::spec::flag::SpecFlag;
pub use crate::spec::mount::SpecMount;
pub use crate::spec::Spec;

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
pub mod parse;
pub(crate) mod sh;
pub(crate) mod string;
#[cfg(test)]
mod test;
