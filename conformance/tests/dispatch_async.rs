//! Async commands, dispatched.
//!
//! Two ways to have one, and both are held here. `RunAsync` / `RunAsyncWith` are the async
//! pair: an implementation writes `async fn` and the generated dispatch awaits the selected
//! command. The sync pair can also carry a future, since `Output` is whatever the command
//! produces — at the cost of naming and boxing it.
//!
//! What the async traits deliberately do *not* impose is `Send`: a CLI on a single-threaded
//! runtime keeps futures that hold an `Rc` across an await, which is what
//! `a_future_that_is_not_send_still_dispatches` is for.

use std::ffi::OsStr;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use usage_argv::{Run, RunAsync, RunAsyncWith, RunWith};
use usage_derive::{Args, Cli, Subcommands};

/// Install a tool
#[derive(Args)]
struct Install {
    #[usage(long)]
    force: bool,
}

/// Show who pays for this
#[derive(Args)]
struct Sponsors;

/// List the configuration
#[derive(Args)]
struct ConfigLs {
    #[usage(long)]
    no_header: bool,
}

#[derive(Subcommands)]
#[usage(run_async, run_async_with)]
enum ConfigCommand {
    /// List the configuration
    Ls(ConfigLs),
}

/// Work with the configuration
#[derive(Args)]
#[usage(run_async, run_async_with)]
struct Config {
    #[usage(subcommand)]
    command: ConfigCommand,
}

#[derive(Subcommands)]
#[usage(run_async, run_async_with)]
enum Command {
    /// Install a tool
    Install(Box<Install>),
    /// Show who pays for this
    Sponsors(Sponsors),
    /// Work with the configuration
    Config(Config),
}

/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Command,
}

impl RunAsync for Install {
    type Output = Result<String, String>;
    async fn run_async(self) -> Self::Output {
        yield_once().await;
        Ok(format!("install force={}", self.force))
    }
}

impl RunAsync for Sponsors {
    type Output = Result<String, String>;
    async fn run_async(self) -> Self::Output {
        yield_once().await;
        Ok("sponsors".to_string())
    }
}

impl RunAsync for ConfigLs {
    type Output = Result<String, String>;
    async fn run_async(self) -> Self::Output {
        yield_once().await;
        Ok(format!("config ls no_header={}", self.no_header))
    }
}

/// What a CLI hands its commands. Borrowed, which is the ordinary case: the future borrows it
/// for as long as it runs.
struct App {
    jobs: usize,
}

impl RunAsyncWith<&App> for Install {
    type Output = Result<String, String>;
    async fn run_async_with(self, app: &App) -> Self::Output {
        yield_once().await;
        Ok(format!("install force={} jobs={}", self.force, app.jobs))
    }
}

impl RunAsyncWith<&App> for Sponsors {
    type Output = Result<String, String>;
    async fn run_async_with(self, _: &App) -> Self::Output {
        yield_once().await;
        Ok("sponsors".to_string())
    }
}

impl RunAsyncWith<&App> for ConfigLs {
    type Output = Result<String, String>;
    async fn run_async_with(self, app: &App) -> Self::Output {
        yield_once().await;
        Ok(format!("config ls jobs={}", app.jobs))
    }
}

fn parse(words: &[&str]) -> Ex {
    let argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    Ex::parse_from(&argv).expect("valid command line")
}

#[test]
fn the_selected_command_is_the_one_awaited() {
    let ex = parse(&["install", "--force"]);
    assert_eq!(
        block_on(ex.command.run_async()),
        Ok("install force=true".to_string())
    );
    let ex = parse(&["sponsors"]);
    assert_eq!(block_on(ex.command.run_async()), Ok("sponsors".to_string()));
}

/// Both levels generated: the enum's dispatch reaches a struct whose own dispatch awaits the
/// next enum.
#[test]
fn a_nested_async_command_dispatches_through_its_group() {
    let ex = parse(&["config", "ls", "--no-header"]);
    assert_eq!(
        block_on(ex.command.run_async()),
        Ok("config ls no_header=true".to_string())
    );
}

#[test]
fn a_context_reaches_an_async_command() {
    let app = App { jobs: 4 };
    let ex = parse(&["install"]);
    assert_eq!(
        block_on(ex.command.run_async_with(&app)),
        Ok("install force=false jobs=4".to_string())
    );
    let ex = parse(&["config", "ls"]);
    assert_eq!(
        block_on(ex.command.run_async_with(&app)),
        Ok("config ls jobs=4".to_string())
    );
}

/// `Send` is what a spawning CLI needs and what a single-threaded one cannot always give, so
/// the traits ask for neither. It is inferred where it holds — asserted here on the future the
/// generated dispatch returns — and a command whose future is not `Send` still dispatches.
#[test]
fn send_is_inferred_and_never_required() {
    fn assert_send<T: Send>(_: &T) {}

    let app = App { jobs: 1 };
    let sendable = parse(&["sponsors"]).command.run_async_with(&app);
    assert_send(&sendable);
    assert_eq!(block_on(sendable), Ok("sponsors".to_string()));
}

/// The command that proves the point above: its future holds an `Rc` across an await, so it is
/// not `Send`, and it is dispatched by the same generated code.
#[derive(Args)]
struct Local;

#[derive(Subcommands)]
#[usage(run_async)]
enum LocalCommand {
    Local(Local),
}

/// A tool with one local command
#[derive(Cli)]
#[usage(bin = "local-ex")]
struct LocalEx {
    #[usage(subcommand)]
    command: LocalCommand,
}

impl RunAsync for Local {
    type Output = String;
    async fn run_async(self) -> Self::Output {
        let counter = Rc::new(1);
        yield_once().await;
        format!("local {}", *counter)
    }
}

#[test]
fn a_future_that_is_not_send_still_dispatches() {
    let argv = [OsStr::new("local")];
    let ex = LocalEx::parse_from(&argv).expect("valid command line");
    assert_eq!(block_on(ex.command.run_async()), "local 1");
}

/// The other way to be async: a boxed future as the sync traits' `Output`. Nothing in either
/// trait says `Send`, so a CLI that wants it puts it in the type it names — as this one does,
/// since spawning is the usual reason to reach for the box at all.
type Task<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Args)]
struct Boxed {
    #[usage(long)]
    force: bool,
}

#[derive(Subcommands)]
#[usage(run, run_with)]
enum BoxedCommand {
    Boxed(Boxed),
}

/// A tool whose commands return futures
#[derive(Cli)]
#[usage(bin = "boxed-ex")]
struct BoxedEx {
    #[usage(subcommand)]
    command: BoxedCommand,
}

impl Run for Boxed {
    type Output = Task<'static, Result<String, String>>;
    fn run(self) -> Self::Output {
        Box::pin(async move {
            yield_once().await;
            Ok(format!("boxed force={}", self.force))
        })
    }
}

impl<'a> RunWith<&'a App> for Boxed {
    type Output = Task<'a, Result<String, String>>;
    fn run_with(self, app: &'a App) -> Self::Output {
        Box::pin(async move {
            yield_once().await;
            Ok(format!("boxed jobs={}", app.jobs))
        })
    }
}

#[test]
fn a_boxed_future_is_an_output_like_any_other() {
    let argv = [OsStr::new("boxed"), OsStr::new("--force")];
    let ex = BoxedEx::parse_from(&argv).expect("valid command line");
    assert_eq!(
        block_on(ex.command.run()),
        Ok("boxed force=true".to_string())
    );

    let app = App { jobs: 8 };
    let argv = [OsStr::new("boxed")];
    let ex = BoxedEx::parse_from(&argv).expect("valid command line");
    assert_eq!(
        block_on(ex.command.run_with(&app)),
        Ok("boxed jobs=8".to_string())
    );
}

/// Dispatch is still invisible to the spec, async or not.
#[test]
fn async_dispatch_says_nothing_in_the_spec() {
    let kdl = Ex::to_kdl();
    assert!(kdl.contains("cmd install"), "{kdl}");
    assert!(!kdl.contains("run"), "{kdl}");
}

/// The smallest executor that proves these are real futures: no runtime dependency in the
/// conformance crate, and a command that yields is resumed rather than run to completion on
/// the first poll.
fn block_on<F: Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};

    let mut future = Box::pin(future);
    // Spinning rather than parking, which is the whole executor this needs: nothing here waits
    // on anything outside the test.
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}

/// One `Pending` before finishing, so a future that is not resumed cannot pass these tests.
async fn yield_once() {
    struct YieldOnce(bool);
    impl Future for YieldOnce {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<()> {
            if self.0 {
                std::task::Poll::Ready(())
            } else {
                self.0 = true;
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }
    YieldOnce(false).await
}
