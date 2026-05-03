use std::path::PathBuf;

use crate::sdk::{SdkFile, SdkOptions, SdkOutput};
use crate::Spec;

mod runtime;
mod types;
mod wrappers;

pub fn generate(spec: &Spec, opts: &SdkOptions) -> SdkOutput {
    let package_name = opts
        .package_name
        .clone()
        .unwrap_or_else(|| spec.bin.clone());

    SdkOutput {
        files: vec![
            SdkFile {
                path: PathBuf::from("types.ts"),
                content: types::render(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("client.ts"),
                content: wrappers::render(spec, &package_name, &opts.source_file),
            },
            SdkFile {
                path: PathBuf::from("runtime.ts"),
                content: runtime::RUNTIME_TS.to_string(),
            },
            SdkFile {
                path: PathBuf::from("index.ts"),
                content: render_index(&package_name),
            },
        ],
    }
}

fn render_index(package_name: &str) -> String {
    let class_name = heck::AsPascalCase(package_name).to_string();
    format!("export {{ {class_name} }} from \"./client\";\nexport * from \"./types\";\n")
}
