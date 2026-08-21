//! Async commands, dispatched.
//!
//! `Output` is whatever the command produces, and a future is a value like any other — so an
//! async command's dispatch is the same generated match, returning a future to await rather
//! than a result to inspect. There is nothing async in the traits themselves: they would have
//! to name a future type the CLI owns, and boxing one is the CLI's decision rather than this
//! crate's.

use std::ffi::OsStr;
use std::future::Future;
use std::pin::Pin;

use usage_argv::{Run, RunWith};
use usage_derive::{Args, Cli, Subcommands};

/// What an async command returns: a future the caller awaits.
///
/// Boxed because an `async` block's type cannot be named, and an associated type has to be.
/// One allocation per invocation, on the path that is about to do I/O anyway.
type Task<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Install a tool
#[derive(Args)]
struct Install {
    #[usage(long)]
    force: bool,
}

/// Show who pays for this
#[derive(Args)]
struct Sponsors;

#[derive(Subcommands)]
#[usage(run, run_with)]
enum Command {
    Install(Install),
    Sponsors(Sponsors),
}

/// A tool that does things
#[derive(Cli)]
#[usage(bin = "ex")]
struct Ex {
    #[usage(subcommand)]
    command: Command,
}

impl Run for Install {
    type Output = Task<'static, Result<String, String>>;
    fn run(self) -> Self::Output {
        Box::pin(async move {
            yield_once().await;
            Ok(format!("install force={}", self.force))
        })
    }
}

impl Run for Sponsors {
    type Output = Task<'static, Result<String, String>>;
    fn run(self) -> Self::Output {
        Box::pin(async move {
            yield_once().await;
            Ok("sponsors".to_string())
        })
    }
}

/// What a CLI hands its commands. A borrowed context is what ties the future's lifetime, which
/// is why `Task` takes one.
struct App {
    jobs: usize,
}

impl<'a> RunWith<&'a App> for Install {
    type Output = Task<'a, Result<String, String>>;
    fn run_with(self, app: &'a App) -> Self::Output {
        Box::pin(async move {
            yield_once().await;
            Ok(format!("install force={} jobs={}", self.force, app.jobs))
        })
    }
}

impl<'a> RunWith<&'a App> for Sponsors {
    type Output = Task<'a, Result<String, String>>;
    fn run_with(self, _: &'a App) -> Self::Output {
        Box::pin(async move {
            yield_once().await;
            Ok("sponsors".to_string())
        })
    }
}

fn parse(words: &[&str]) -> Ex {
    let argv: Vec<&OsStr> = words.iter().map(OsStr::new).collect();
    Ex::parse_from(&argv).expect("valid command line")
}

#[test]
fn an_async_command_dispatches_to_a_future() {
    let ex = parse(&["install", "--force"]);
    assert_eq!(
        block_on(ex.command.run()),
        Ok("install force=true".to_string())
    );
}

#[test]
fn an_async_command_dispatches_with_a_borrowed_context() {
    let app = App { jobs: 4 };
    let ex = parse(&["install"]);
    assert_eq!(
        block_on(ex.command.run_with(&app)),
        Ok("install force=false jobs=4".to_string())
    );
    let ex = parse(&["sponsors"]);
    assert_eq!(block_on(ex.command.run_with(&app)), Ok("sponsors".into()));
}

/// The smallest executor that proves these are real futures: no runtime dependency in the
/// conformance crate, and a command that yields is resumed rather than run to completion on
/// the first poll.
fn block_on<F: Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};

    let mut future = Box::pin(future);
    // Spinning rather than parking, which is the whole executor this needs: nothing here waits
    // on anything outside the test.
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
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
