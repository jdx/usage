#[cfg(feature = "cli-help")]
pub mod cli;
#[cfg(feature = "cli-help")]
mod layout;
#[cfg(feature = "docs")]
pub mod manpage;
#[cfg(feature = "docs")]
pub mod markdown;
#[cfg(feature = "cli-help")]
pub(crate) mod models;
