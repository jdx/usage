//! The KDL document model and parser used by usage.
//!
//! This is a trimmed copy of kdl-rs 6.7.1: serde, queries, KDL v1 fallback, and the miette
//! integration are omitted. The parser and formatting-preserving document types remain intact.
//! See `NOTICE.md` and `lib/third-party/LICENSE-APACHE-2.0` for attribution and license terms.

pub use document::*;
pub use entry::*;
pub use error::*;
pub(crate) use fmt::*;
pub use identifier::*;
pub use node::*;
pub use value::*;

mod document;
mod entry;
mod error;
mod fmt;
mod identifier;
mod node;
mod v2_parser;
mod value;
