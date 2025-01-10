use std::fs;
use std::os::unix::prelude::*;
use std::path::{Path, PathBuf};

use miette7::IntoDiagnostic;

use usage::Spec;

use crate::{env, hash};

pub fn execute(script: &Path, args: &[String]) -> miette7::Result<()> {
    let (_schema, body) = Spec::parse_file(script)?;
    // let cmd: Command = (&schema).into();
    // let m = cmd.get_matches_from(args[1..].to_vec());
    // for flag in &schema.cmd.flags {
    //     if flag.arg.is_some() {
    //         env::set_var(
    //             format!("usage_{}", &flag.name),
    //             m.get_one::<String>(&flag.name).cloned().unwrap_or_default(),
    //         )
    //     } else {
    //         env::set_var(
    //             format!("usage_{}", &flag.name),
    //             m.get_flag(&flag.name).to_string(),
    //         )
    //     }
    // }
    // for arg in &schema.cmd.args {
    //     env::set_var(
    //         format!("usage_{}", &arg.name),
    //         m.get_one::<String>(&arg.name).cloned().unwrap_or_default(),
    //     )
    // }
    let output_path = create_script(script, &body)?;
    let mut cmd = exec::Command::new(output_path);
    let err = cmd.args(&args[1..]).exec();
    Err(err).into_diagnostic()?
}

// fn get_schema(script: &Path) -> Result<String> {
//     let out = Command::new(script)
//         .arg("__USAGE__")
//         .stdin(Stdio::inherit())
//         .stderr(Stdio::inherit())
//         .output()
//         .into_diagnostic()?;
//     ensure!(
//         out.status.success(),
//         "Script exited with non-zero status code: {}",
//         out.status
//     );
//     String::from_utf8(out.stdout).into_diagnostic()
// }

fn create_script(script: &Path, body: &str) -> miette7::Result<PathBuf> {
    let tmp_filename = script.file_name().unwrap().to_str().unwrap();
    let tmp_filename = tmp_filename
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>();
    let tmp_filename = format!("{tmp_filename}-{}", hash::hash_to_str(&body));
    let output_path = env::CACHE_DIR.join(tmp_filename);
    if !output_path.exists() {
        fs::create_dir_all(&*env::CACHE_DIR).into_diagnostic()?;
        fs::write(&output_path, body).into_diagnostic()?;
        // make executable
        let mut perms = fs::metadata(&output_path).into_diagnostic()?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&output_path, perms).into_diagnostic()?;
    }
    Ok(output_path)
}
