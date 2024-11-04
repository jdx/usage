use clap::Command;
use std::io::Write;

pub fn generate<S: Into<String>>(cmd: &mut Command, bin_name: S, buf: &mut dyn Write) {
    let mut spec: usage::Spec = cmd.clone().into();
    spec.bin = bin_name.into();

    writeln!(buf, "{spec}").expect("write usage spec");
}
