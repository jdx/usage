use clap::{arg, Command, ValueHint};
use clap_usage::generate;
use std::io::BufWriter;

fn build_example_cli() -> Command {
    Command::new("example")
        .arg(arg!(--file <FILE> "some input file").value_hint(ValueHint::AnyPath))
        .arg(arg!(--usage))
}

#[test]
fn test_simple() {
    let mut cli = build_example_cli();
    let mut buf = BufWriter::new(Vec::new());
    generate(&mut cli, "example", &mut buf);

    let output = String::from_utf8(buf.into_inner().unwrap()).unwrap();
    insta::assert_snapshot!(output);
}
