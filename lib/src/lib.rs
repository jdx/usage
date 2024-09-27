#[cfg(test)]
#[macro_use]
extern crate insta;
#[macro_use]
extern crate log;

pub use crate::parse::parse;
pub use crate::spec::arg::SpecArg;
pub use crate::spec::choices::SpecChoices;
pub use crate::spec::cmd::SpecCommand;
pub use crate::spec::complete::SpecComplete;
pub use crate::spec::flag::SpecFlag;
pub use crate::spec::mount::SpecMount;
pub use crate::spec::Spec;

#[macro_use]
pub mod error;
pub mod complete;
pub mod spec;

#[cfg(feature = "docs")]
mod docs;
pub mod parse;
pub(crate) mod sh;
#[cfg(test)]
mod test;
