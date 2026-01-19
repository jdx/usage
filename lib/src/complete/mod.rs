use crate::error::UsageErr;
use crate::Spec;

mod bash;
mod fish;
mod powershell;
mod zsh;

pub struct CompleteOptions {
    pub usage_bin: String,
    pub shell: String,
    pub bin: String,
    pub cache_key: Option<String>,
    pub spec: Option<Spec>,
    pub usage_cmd: Option<String>,
    pub include_bash_completion_lib: bool,
    pub source_file: Option<String>,
}

pub fn complete(options: &CompleteOptions) -> Result<String, UsageErr> {
    match options.shell.as_str() {
        "bash" => Ok(bash::complete_bash(options)),
        "fish" => Ok(fish::complete_fish(options)),
        "powershell" => Ok(powershell::complete_powershell(options)),
        "zsh" => Ok(zsh::complete_zsh(options)),
        _ => Err(UsageErr::UnsupportedShell(options.shell.clone())),
    }
}
