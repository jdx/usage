#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;
#[cfg(not(target_arch = "wasm32"))]
use xx::process::check_status;
#[cfg(not(target_arch = "wasm32"))]
use xx::{XXError, XXResult};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn sh(script: &str) -> XXResult<String> {
    #[cfg(unix)]
    let (shell, flag) = ("sh", "-c");

    #[cfg(windows)]
    let (shell, flag) = ("cmd", "/c");

    let output = Command::new(shell)
        .arg(flag)
        .arg(script)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .env("__USAGE", env!("CARGO_PKG_VERSION"))
        .output()
        .map_err(|err| XXError::ProcessError(err, format!("{shell} {flag} {script}")))?;

    check_status(output.status)
        .map_err(|err| XXError::ProcessError(err, format!("{shell} {flag} {script}")))?;
    let stdout = String::from_utf8(output.stdout).expect("stdout is not utf-8");
    Ok(stdout)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn sh(_script: &str) -> std::result::Result<String, crate::error::UsageErr> {
    Err(crate::error::UsageErr::IO(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "shell execution is not supported on wasm",
    )))
}
