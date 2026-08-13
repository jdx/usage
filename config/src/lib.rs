//! Layered configuration resolution for CLIs that describe their settings in a usage spec.
//!
//! Every CLI in the jdx fleet has written this by hand, and every copy has rotted
//! differently: hk declares eighteen `sources.cli` bindings and reads five, pitchfork
//! documents a CLI layer it does not have, fnox's module doc describes a config-file layer
//! that does not exist, and mise hand-copies thirteen flags into its settings in a
//! forty-nine-line function. The drift is not carelessness — it is what happens when the
//! declaration of a setting and the code that resolves it are two separate things that have
//! to be kept in step by hand.
//!
//! Here they are one thing. `usage-config-build` reads the spec's `config` block at build
//! time and emits a [`Registry`] of consts; this crate resolves values against it. Nothing
//! here parses KDL, so a CLI carries a resolver rather than a spec parser.
//!
//! # What it guarantees
//!
//! - **One merge.** Provenance is the output of the only merge there is, so `config explain`
//!   cannot describe a resolution that did not happen — which a second, parallel merge
//!   function written for the purpose can.
//! - **Fixed precedence.** cli > env > files, nearest first > user > machine > declared
//!   defaults. Which layers a CLI has is its own business; their order is not.
//! - **Scope is enforced, not remembered.** A `scope="global"` setting refuses an untrusted
//!   place in the merge, not in each layer, because a check every layer has to make is one a
//!   new layer will forget. The question is [`Trust`], not "was it a file": a pkl file or a git
//!   config inside a checkout is exactly as much a thing a repository carries as `hk.toml` is,
//!   and a kind usage does not recognize gets the least trusting answer until its layer says
//!   otherwise.
//! - **Warnings, not output.** Nothing here prints. An unknown key, a value of the wrong
//!   type, a deprecated setting: all returned, for the CLI to render when its logging is up.
//!
//! # Example
//!
//! ```
//! use usage_config::{resolve, Layers, Origin, PropMeta, Registry, SourceKind, Ty, Value};
//! use usage_config::{Layer, LayerCtx, LayerError, LayerOutput};
//!
//! // Normally generated from the spec by usage-config-build.
//! static PROPS: &[PropMeta] = &[PropMeta {
//!     envs: &["MYCLI_JOBS"],
//!     default: Some(usage_config::Const::Int(4)),
//!     ..PropMeta::new("jobs", Ty::Uint)
//! }];
//! const REGISTRY: Registry = Registry::new(PROPS);
//!
//! // A layer reads one kind of place. This one stands in for the environment.
//! struct Env;
//! impl Layer for Env {
//!     fn source(&self) -> SourceKind {
//!         SourceKind::ENV
//!     }
//!     fn load(&self, ctx: &LayerCtx) -> Result<LayerOutput, LayerError> {
//!         let mut out = LayerOutput::new();
//!         let id = ctx.prop("jobs").expect("declared").id;
//!         match ctx.entry(id, "8", Origin::new(SourceKind::ENV, "MYCLI_JOBS")) {
//!             Ok(entry) => out.push(entry),
//!             Err(warning) => out.warn(warning),
//!         }
//!         Ok(out)
//!     }
//! }
//!
//! let env = Env;
//! let resolved = resolve(REGISTRY, Layers::new().then(&env))?;
//! assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
//! assert_eq!(
//!     resolved.origin(REGISTRY.lookup("jobs").unwrap().id).unwrap().describe(),
//!     "MYCLI_JOBS",
//! );
//! # Ok::<(), LayerError>(())
//! ```

pub mod explain;
#[cfg(any(feature = "toml", feature = "json"))]
pub mod files;
pub mod layer;
pub mod registry;
pub mod resolve;
pub mod source;
pub mod ty;
pub mod value;

pub use explain::explain;
#[cfg(any(feature = "toml", feature = "json"))]
pub use files::{FileLayer, Format};
pub use layer::{Entry, Layer, LayerCtx, LayerError, LayerOutput, Warning};
pub use registry::{Lookup, Merge, PropId, PropMeta, Registry, Scope};
pub use resolve::{resolve, Layers, Resolved};
pub use source::{FileScope, Origin, SourceKind, Trust};
pub use ty::{Parser, Ty, TypeError};
pub use value::{Const, Value};
