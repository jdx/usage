use std::env;

fn main() -> miette::Result<()> {
    env_logger::init();

    let args: Vec<_> = env::args().collect();
    usage_cli::run(&args)
}
