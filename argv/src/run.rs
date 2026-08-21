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
//! Every one of these traits takes `self` by value. A command is finished when it has run, and
//! the values it parsed are its own — taking them by reference would mean every handler
//! borrowing what nothing else can want.
//!
//! # Which one
//!
//! |                | no context | a context            |
//! | -------------- | ---------- | -------------------- |
//! | **sync**       | [`Run`]    | [`RunWith`]          |
//! | **async**      | [`RunAsync`] | [`RunAsyncWith`]   |
//!
//! The context is whatever the CLI has to give — a resolved config, an output handle, a
//! database connection — and the `With` traits are generic over it, so `RunWith<&mut App>` and
//! `RunAsyncWith<Arc<Ctx>>` are ordinary implementations rather than shapes this crate has to
//! anticipate.
//!
//! A context is a separate trait rather than one defaulted to `()` because the noise otherwise
//! falls on the wrong side of a CLI: a hundred commands that need no context would each carry
//! `fn run(self, _: ())`, which says nothing and cannot be left out. One type may implement
//! several of these, and one enum may dispatch several, which is what a CLI part-way through
//! adopting a context — or an async runtime — needs.
//!
//! # Async commands
//!
//! [`RunAsync`] and [`RunAsyncWith`] are the async pair: an implementation writes `async fn`,
//! and the generated dispatch is an `async fn` that awaits the selected command.
//!
//! ```
//! use usage_argv::RunAsync;
//!
//! struct Install {
//!     force: bool,
//! }
//!
//! impl RunAsync for Install {
//!     type Output = Result<(), String>;
//!     async fn run_async(self) -> Self::Output {
//!         // .await here
//!         Ok(())
//!     }
//! }
//! ```
//!
//! The trait declares `-> impl Future<Output = Self::Output>` rather than `async fn`, which is
//! the same thing on the implementing side and **deliberately imposes no `Send` bound**: a CLI
//! on a single-threaded runtime keeps futures that hold an `Rc` across an await, and one that
//! spawns gets `Send` by inference, since it leaks out of the concrete commands the dispatch
//! reaches. What this cannot do is *demand* `Send` in generic code, which is the trade the
//! alternative — `-> impl Future + Send` in the trait — makes in the other direction, and
//! there is no way to have both without duplicating the trait.
//!
//! The sync pair can carry a future too, since [`Output`](Run::Output) is whatever the command
//! produces: a boxed `Pin<Box<dyn Future<Output = T>>>` (plus `+ Send` if the CLI wants it) is
//! a value like any other. That costs an allocation and names a type; the async traits exist so
//! that neither is necessary.
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

/// An async command: [`Run`], awaited.
///
/// The signature is `-> impl Future` rather than `async fn` so that no `Send` bound is implied
/// either way — an implementation still writes `async fn run_async(self)`, and whether its
/// future is `Send` is decided by what the command does rather than by this trait. See the
/// [module docs](self#async-commands).
pub trait RunAsync {
    /// What running the command produces, once awaited.
    type Output;

    /// Carry out the command.
    fn run_async(self) -> impl core::future::Future<Output = Self::Output>;
}

/// An async command that is handed something shared when it runs: [`RunWith`], awaited.
///
/// A borrowed context is the ordinary case, and the future borrows it for as long as it runs:
/// `impl<'a> RunAsyncWith<&'a App> for Install`.
pub trait RunAsyncWith<Ctx> {
    /// What running the command produces, once awaited.
    type Output;

    /// Carry out the command, with `ctx`.
    fn run_async_with(self, ctx: Ctx) -> impl core::future::Future<Output = Self::Output>;
}
