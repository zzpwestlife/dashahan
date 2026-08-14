// INPUT: dsh web child process handle
// OUTPUT: shared killable state for the app
// POS: src-tauri/src/state.rs
use std::process::Child;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub child: Mutex<Option<Child>>,
}

impl AppState {
    pub fn kill_child(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut c) = guard.take() {
                let _ = c.kill();
                let _ = c.wait();
            }
        }
    }
}
