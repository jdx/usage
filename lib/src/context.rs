use std::path::PathBuf;
use std::sync::Mutex;

static LOAD_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn get_load_root() -> PathBuf {
    LOAD_ROOT.lock().unwrap().clone().unwrap_or_default()
}
pub fn set_load_root(root: PathBuf) {
    *LOAD_ROOT.lock().unwrap() = Some(root);
}
