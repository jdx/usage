use clap::CommandFactory;
use usage_cli::Cli;

#[test]
fn verify_cli_sorted() {
    clap_sort::assert_sorted(&Cli::command());
}
