use env_logger::Env;
use std::env;

fn main() -> miette::Result<()> {
    env_logger::builder()
        .format_timestamp(None)
        .parse_env(Env::default().filter_or("USAGE_LOG", "info"))
        .init();

    let args: Vec<_> = env::args().collect();
    usage_cli::run(&args)
}
