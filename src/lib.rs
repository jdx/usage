// #[macro_use]
// extern crate log;
#[cfg(test)]
#[macro_use]
extern crate insta;
pub mod complete;
pub mod error;
pub mod parse;

pub use parse::arg::Arg;
pub use parse::cmd::SchemaCmd;
pub use parse::flag::Flag;
pub use parse::spec::Spec;
