use std::env;

fn main() -> miette::Result<()> {
    let args: Vec<_> = env::args().collect();
    usage_cli::run(&args)
}
