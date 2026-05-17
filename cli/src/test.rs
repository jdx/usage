use crate::env;

#[ctor::ctor(unsafe)]
fn init() {
    env::set_var("USAGE_BIN", "usage");
}
