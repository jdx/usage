use usage_cli::env;

fn main() -> usage::miette::Result<()> {
    // The filter is resolved by `env::log_filter` rather than named to `env_logger`, which falls
    // back to its default only when a variable is *unset* — a blank `USAGECLI_LOG` would be
    // taken as the filter rather than falling through to `USAGE_LOG`.
    //
    // Nothing is written back into the environment either. The old shape set `USAGE_LOG` with
    // `set_var`, which a spawned script then inherited — and on Windows, where variable names
    // are case-insensitive, that is the same variable a spec's own `log` argument writes, so
    // usage was overwriting a value the script was about to read.
    let mut builder = env_logger::builder();
    builder
        .format_timestamp(None)
        .parse_filters(&env::log_filter(|key| env::var(key).ok()));
    // `Env::default()` used to bring this along; `parse_filters` covers only the filter.
    if let Ok(style) = env::var(env_logger::DEFAULT_WRITE_STYLE_ENV) {
        builder.parse_write_style(&style);
    }
    builder.init();

    let args: Vec<_> = env::args().collect();
    usage_cli::run(&args)
}
