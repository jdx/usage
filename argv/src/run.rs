//! Dispatch: handing a parsed command to the code that carries it out.
//!
//! A parse ends with a value — an enum whose selected variant holds the command's own
//! struct — and every CLI then writes the same thing: a `match` over that enum, one arm per
//! command, each arm calling the one function that command exists to call. At mise's size
//! that match is 210 arms of pure routing, and the compiler cannot tell that an arm calling
//! the wrong function is wrong, because every arm has the same shape.
//!
//! So the derive writes it. A command implements [`Run`] (or [`RunWith`], when the CLI hands
//! its commands shared state), the enum says `#[usage(run)]`, and the match is generated from
//! the same declaration the parser and the spec come from. Nothing about it reaches the
//! spec: which Rust function carries out a command is not part of what the CLI *is*, and a
//! spec that recorded it could not be read by anything that is not this program. It is the
//! same rule `#[usage(skip)]` follows.
//!
//! Both traits take `self` by value. A command is finished when it has run, and the values
//! it parsed are its own — taking them by reference would mean every handler borrowing what
//! nothing else can want.
//!
//! # Which one
//!
//! [`Run`] is for a CLI whose commands need nothing but what they parsed. [`RunWith`] is for
//! one that hands them something — a resolved config, an output handle, a database
//! connection — and is generic over what that something is, so `RunWith<&mut App>` and
//! `RunWith<Arc<Ctx>>` are both ordinary implementations rather than a shape this crate has
//! to anticipate.
//!
//! They are two traits rather than one with a defaulted context because the noise falls on
//! the wrong side of a CLI otherwise: a hundred commands that need no context would each
//! carry `fn run(self, _: ())`, which says nothing and cannot be left out. A type may
//! implement both, and an enum may dispatch both, when some invocations have a context and
//! others do not.
//!
//! # An example
//!
//! ```
//! use usage_argv::Run;
//!
//! struct Install {
//!     force: bool,
//! }
//! struct Sponsors;
//!
//! impl Run for Install {
//!     type Output = Result<(), String>;
//!     fn run(self) -> Self::Output {
//!         if self.force {
//!             Ok(())
//!         } else {
//!             Err("refusing without --force".into())
//!         }
//!     }
//! }
//!
//! impl Run for Sponsors {
//!     type Output = Result<(), String>;
//!     fn run(self) -> Self::Output {
//!         println!("thanks");
//!         Ok(())
//!     }
//! }
//!
//! // What `#[usage(run)]` on the subcommand enum generates, written out.
//! enum Command {
//!     Install(Install),
//!     Sponsors(Sponsors),
//! }
//!
//! impl Run for Command
//! where
//!     Install: Run,
//!     Sponsors: Run<Output = <Install as Run>::Output>,
//! {
//!     type Output = <Install as Run>::Output;
//!     fn run(self) -> Self::Output {
//!         match self {
//!             Command::Install(inner) => Run::run(inner),
//!             Command::Sponsors(inner) => Run::run(inner),
//!         }
//!     }
//! }
//!
//! assert!(Command::Install(Install { force: true }).run().is_ok());
//! ```

/// A command that can be carried out with nothing but what it parsed.
///
/// The output is the implementation's own: `Result<(), E>` for a CLI whose commands can
/// fail, `()` for one whose commands cannot, [`ExitCode`](std::process::ExitCode) for one
/// that decides its own status. A generated dispatcher takes its output from the first
/// command it routes to and requires the rest to agree, since a `match` has one type.
pub trait Run {
    /// What running the command produces.
    type Output;

    /// Carry out the command.
    fn run(self) -> Self::Output;
}

/// A command that is handed something shared when it runs.
///
/// `Ctx` is whatever the CLI has to give: `&Config`, `&mut App`, an owned handle. It is a
/// parameter of the trait rather than of the method so that one command may be runnable with
/// several — a leaf that needs only a config can implement `RunWith<&Config>` while its
/// siblings implement `RunWith<&mut App>`, as long as the enum dispatching them agrees on
/// one.
pub trait RunWith<Ctx> {
    /// What running the command produces.
    type Output;

    /// Carry out the command, with `ctx`.
    fn run_with(self, ctx: Ctx) -> Self::Output;
}
