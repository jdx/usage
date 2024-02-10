use crate::env;

#[ctor::ctor]
fn init() {
    env::set_var("USAGE_BIN", "usage");
}
