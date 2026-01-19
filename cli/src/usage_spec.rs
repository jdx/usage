use crate::cli::Cli;
use clap::CommandFactory;
use miette::Result;

pub(crate) fn generate() -> Result<()> {
    let mut cli = Cli::command().version(env!("CARGO_PKG_VERSION"));
    clap_usage::generate(&mut cli, "usage", &mut std::io::stdout());
    println!("{}", include_str!("../assets/usage-extra.usage.kdl").trim());

    Ok(())
}

pub(crate) fn complete(shell: &str) -> Result<()> {
    match shell {
        "bash" => print!("{}", include_str!("../assets/completions/usage.bash")),
        "fish" => print!("{}", include_str!("../assets/completions/usage.fish")),
        "zsh" => print!("{}", include_str!("../assets/completions/_usage")),
        _ => miette::bail!("unsupported shell: {}", shell),
    };

    Ok(())
}
