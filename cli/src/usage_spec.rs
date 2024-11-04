use crate::cli::Cli;
use clap::CommandFactory;
use miette::Result;

pub(crate) fn generate() -> Result<()> {
    let mut cli = Cli::command().version(env!("CARGO_PKG_VERSION"));
    clap_usage::generate(&mut cli, "usage", &mut std::io::stdout());
    println!("{}", include_str!("../usage-extra.usage.kdl").trim());

    Ok(())
}

pub(crate) fn complete(shell: &str) -> Result<()> {
    match shell {
        "bash" => print!("{}", include_str!("../completions/usage.bash")),
        "fish" => print!("{}", include_str!("../completions/usage.fish")),
        "zsh" => print!("{}", include_str!("../completions/_usage")),
        _ => unimplemented!("unsupported shell: {}", shell),
    };

    Ok(())
}
