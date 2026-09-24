//! A generated async dispatch keeps the selected command's future off the stack.
//!
//! Without optimizations, the compiler gives every temporary in an `async fn` its own stack
//! slot, and the poll function's frame stays on the stack for as long as the future runs. A
//! dispatch that wrote `Command::run_async(inner).await` in each arm therefore held a slot the
//! size of *every* command's future while running any one of them: mise's top-level dispatch
//! took 1.8 MB of a debug build's main stack that way, and its nested ones another 1.8 MB,
//! which is how `mise bootstrap dotfiles apply` came to overflow Windows' 8 MB main thread.
//!
//! Here, sixteen commands each hold 128 KiB across an await, and the dispatch runs on a thread
//! with 1 MiB of stack. A dispatch frame with a slot per command would need 2 MiB and abort the
//! test binary with a stack overflow, which is why this is a file of its own.

use std::ffi::OsStr;
use std::future::Future;
use std::pin::Pin;

use usage_argv::RunAsync;
use usage_derive::{Args, Cli, Subcommands};

/// What each command keeps across its await: large enough that sixteen of them cannot share a
/// 1 MiB stack, small enough that one of them can.
const HELD: usize = 128 * 1024;

macro_rules! commands {
    ($($name:ident = $word:literal,)*) => {
        $(
            #[derive(Args)]
            #[usage(name = $word)]
            struct $name;

            impl RunAsync for $name {
                type Output = usize;
                async fn run_async(self) -> Self::Output {
                    let held = std::hint::black_box([1u8; HELD]);
                    yield_once().await;
                    held.iter().map(|&byte| usize::from(byte)).sum()
                }
            }
        )*

        #[derive(Subcommands)]
        #[usage(run_async)]
        enum Command {
            $($name($name),)*
        }
    };
}

commands! {
    C00 = "c00", C01 = "c01", C02 = "c02", C03 = "c03",
    C04 = "c04", C05 = "c05", C06 = "c06", C07 = "c07",
    C08 = "c08", C09 = "c09", C10 = "c10", C11 = "c11",
    C12 = "c12", C13 = "c13", C14 = "c14", C15 = "c15",
}

/// Work with many large commands
#[derive(Args)]
#[usage(run_async)]
struct Group {
    #[usage(subcommand)]
    command: Command,
}

#[derive(Subcommands)]
#[usage(run_async)]
enum Top {
    /// Work with many large commands
    Group(Group),
}

/// A tool whose commands keep a lot across an await
#[derive(Cli)]
#[usage(bin = "big")]
struct Big {
    #[usage(subcommand)]
    command: Top,
}

#[test]
fn a_dispatch_does_not_reserve_stack_for_every_command() {
    let ran = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(|| {
            let argv = [OsStr::new("group"), OsStr::new("c15")];
            let big = Big::parse_from(&argv).expect("valid command line");
            block_on(big.command.run_async())
        })
        .expect("spawn")
        .join()
        .expect("the dispatch finished");
    assert_eq!(ran, HELD);
}

/// The dispatch's own future is a handful of boxes, however large the commands it reaches, so
/// a caller that awaits it does not reserve room for them either.
#[test]
fn a_dispatch_future_is_small() {
    let argv = [OsStr::new("group"), OsStr::new("c00")];
    let big = Big::parse_from(&argv).expect("valid command line");
    let future = big.command.run_async();
    assert!(
        size_of_val(&future) < 1024,
        "{} bytes",
        size_of_val(&future)
    );
    assert_eq!(block_on(future), HELD);
}

/// The executor from `dispatch_async.rs`: no runtime dependency, and the future is boxed so
/// the test's own frame does not hold it.
fn block_on<F: Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Waker};

    let mut future = Box::pin(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
    }
}

/// One `Pending` before finishing, so each command's state is really held across an await.
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
