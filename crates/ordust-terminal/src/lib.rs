pub mod tui;

pub use tui ::*;


use std::fs::OpenOptions;
use std::io::Write;

/// Zero-dependency file logger accessible across lib and main targets
pub fn log_debug(msg: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug_engine.log")
    {
        let _ = writeln!(
            file,
            "[{}] {}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            msg
        );
    }
}