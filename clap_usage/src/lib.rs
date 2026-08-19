//! Build [`usage`] specs from [`clap`] commands.
//!
//! [`generate`] writes a spec straight out; [`spec`] hands you the [`usage::Spec`]
//! first so you can set things clap cannot express.
//!
//! The `usage` crate is re-exported, so depending on `clap_usage` alone is
//! enough to name the spec types you get back:
//!
//! ```no_run
//! use clap_usage::usage::SpecCommandEffect;
//! ```

mod generate;
mod report;

/// The [`usage`] crate, re-exported.
///
/// [`spec`] returns `usage` types, so consumers need to name them. Re-exporting
/// the whole crate means `clap_usage` on its own is a sufficient dependency,
/// and nothing here goes stale as `usage` grows.
pub use usage;

pub use crate::generate::{generate, generate_with_report, spec, spec_with_report};
pub use crate::report::{FidelityFeature, FidelityLoss, FidelityReport};
