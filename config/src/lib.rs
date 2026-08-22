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
//! Here they are one thing. `#[derive(usage::Config)]` reads the settings struct and emits a
//! [`Registry`] of consts beside it; this crate resolves values against it. Nothing here
//! parses KDL, so a CLI carries a resolver rather than a spec parser.
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
//! - **Lifecycle gates are explicit.** `deprecated_warn_at` and `deprecated_remove_at` act
//!   against the running CLI version supplied to [`resolve_with_context`]. This crate's own
//!   package version is never assumed.
//!
//! # Example
//!
//! ```
//! use usage_config::{resolve, Const, EnvLayer, Layers, PropMeta, Registry, Ty, Value};
//!
//! // Normally generated from the settings struct by `#[derive(usage::Config)]`.
//! static PROPS: &[PropMeta] = &[PropMeta {
//!     envs: &["MYCLI_JOBS"],
//!     default: Some(Const::Int(4)),
//!     ..PropMeta::new("jobs", Ty::Uint)
//! }];
//! const REGISTRY: Registry = Registry::new(PROPS);
//!
//! // The environment is described rather than reached for, so a test never touches the process.
//! // `EnvLayer::from_process` is what a CLI uses.
//! let env = EnvLayer::new([("MYCLI_JOBS".to_string(), "8".to_string())]);
//! let resolved = resolve(REGISTRY, Layers::new().then(&env))?;
//!
//! assert_eq!(resolved.get_key("jobs"), Some(&Value::Int(8)));
//! // And where it came from is the variable the user set, not "the environment".
//! assert_eq!(
//!     resolved.origin_key("jobs").unwrap().describe(),
//!     "MYCLI_JOBS",
//! );
//! # Ok::<(), usage_config::LayerError>(())
//! ```

pub mod cli;
pub mod env;
pub mod explain;
#[cfg(any(feature = "toml", feature = "json", feature = "yaml"))]
pub mod files;
pub mod layer;
pub mod props;
pub mod read;
pub mod registry;
pub mod resolve;
pub mod source;
pub mod spec;
pub mod ty;
pub mod value;

pub use cli::CliLayer;
pub use env::EnvLayer;
pub use explain::explain;
#[cfg(any(feature = "toml", feature = "json", feature = "yaml"))]
pub use files::{FileLayer, Format};
pub use layer::{Entry, Layer, LayerCtx, LayerError, LayerOutput, Warning, WarningKind};
pub use props::{concat_prop_specs, concat_props, Props};
pub use read::{Fold, FromValue, ReadError, ReadErrorKind, ReadErrors};
pub use registry::{Lookup, Merge, PropId, PropMeta, Registry, Scope};
pub use resolve::{resolve, resolve_with_context, Layers, ResolutionContext, Resolved};
pub use source::{FileScope, Origin, SourceKind, Trust};
pub use spec::{spec_kdl, spec_kdl_with, ConfigSpec, PropSpec, SpecFile, SpecSource};
pub use ty::{Parser, Ty, TypeError};
pub use value::{Const, Value};
