use crate::Spec;

mod bash;
mod fish;
mod zsh;

pub struct CompleteOptions {
    pub shell: String,
    pub bin: String,
    pub cache_key: Option<String>,
    pub spec: Option<Spec>,
    pub usage_cmd: Option<String>,
}

pub fn complete(options: &CompleteOptions) -> String {
    match options.shell.as_str() {
        "bash" => bash::complete_bash(options),
        "fish" => fish::complete_fish(options),
        "zsh" => zsh::complete_zsh(options),
        _ => unimplemented!("unsupported shell: {}", options.shell),
    }
}
