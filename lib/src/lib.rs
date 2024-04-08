// #[macro_use]
// extern crate miette;
#[cfg(test)]
#[macro_use]
extern crate insta;
#[macro_use]
extern crate log;

pub use crate::parse::arg::SpecArg;
pub use crate::parse::cmd::SpecCommand;
pub use crate::parse::complete::Complete;
pub use crate::parse::flag::SpecFlag;
pub use crate::parse::spec::Spec;

#[macro_use]
pub mod error;
pub mod complete;
pub mod context;
pub(crate) mod env;
pub mod parse;

pub mod cli;
#[cfg(test)]
mod test;
