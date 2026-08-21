use usage_cli::env;

fn main() -> miette::Result<()> {
    // Logging is set up after the parse, in `Cli::run`: `--verbose`, `--quiet` and the rest
    // are declarations on the CLI itself now, so the command line is what says how much to
    // say. `USAGE_DEBUG` and `USAGE_TRACE` are `env` fallbacks on those flags rather than
    // two lines here that rewrote `USAGE_LOG` behind the user's back.
    let args: Vec<_> = env::args().collect();
    usage_cli::run(&args)
}
