use crate::Spec;

mod bash;
mod fish;
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

pub fn complete(options: &CompleteOptions) -> String {
    match options.shell.as_str() {
        "bash" => bash::complete_bash(options),
        "fish" => fish::complete_fish(options),
        "zsh" => zsh::complete_zsh(options),
        _ => unimplemented!("unsupported shell: {}", options.shell),
    }
}
