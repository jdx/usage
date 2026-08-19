use usage::Cli;

static IDENTITY_CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn program_name() -> &'static str {
    IDENTITY_CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    "runtime-ex"
}

fn version() -> &'static str {
    "6.0.1+host"
}

#[derive(Cli)]
#[usage(
    name = program_name(),
    name_spec = "portable-ex",
    bin = program_name(),
    bin_spec = "portable-ex",
    version = version(),
    version_spec = "6.0.0",
    unknown_flags = "error"
)]
struct Cli;

fn main() {
    let Cli = Cli::parse();
    assert_eq!(
        IDENTITY_CALLS.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "successful parsing must not evaluate cold runtime identity"
    );
}
