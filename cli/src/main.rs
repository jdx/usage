use env_logger::Env;
use usage_cli::env;

fn main() -> miette::Result<()> {
    set_log_env_vars();
    env_logger::builder()
        .format_timestamp(None)
        .parse_env(Env::default().filter_or("USAGE_LOG", "info"))
        .init();

    let args: Vec<_> = env::args().collect();
    usage_cli::run(&args)
}

fn set_log_env_vars() {
    if env::var_true("USAGE_DEBUG") {
        env::set_var("USAGE_LOG", "debug");
    }
    if env::var_true("USAGE_TRACE") {
        env::set_var("USAGE_LOG", "trace");
    }
}
