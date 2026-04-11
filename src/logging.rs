use colored::Colorize;
use std::fs;
use std::io::Write;

use crate::AnyError;

pub static LOG_FILE: std::sync::RwLock<Option<fs::File>> = std::sync::RwLock::new(None);

pub fn log(name: &str, text: &str, color: Option<colored::Color>) {
    let timestamp = chrono::Utc::now().format("%H:%M:%S%.6f");
    let task_id = match tokio::task::try_id() {
        Some(id) => format!("[task-{id}]"),
        None => format!("[main]"),
    };

    let name = name.to_uppercase();
    let text = format!("{timestamp} UTC {name} {task_id} {text}");

    if let Some(color) = color {
        println!("{}", text.color(color));
    } else {
        println!("{}", text);
    }

    if let Ok(mut log_file_lock) = LOG_FILE.write() {
        if let Some(ref mut file_log) = *log_file_lock {
            writeln!(file_log, "{text}").ok();
            file_log.flush().ok();
        }
    }
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {{
        let text = format!($($arg)*);
        crate::logging::log("info", &text, None);
    }};
}

#[macro_export]
macro_rules! log_warning {
    ($($arg:tt)*) => {{
        let text = format!($($arg)*);
        crate::logging::log("warning", &text, Some(colored::Color::Yellow));
    }};
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {{
        let text = format!($($arg)*);
        crate::logging::log("error", &text, Some(colored::Color::Red));
    }};
}

pub use log_error as error;
pub use log_info as info;
pub use log_warning as warning;

// TODO: Removing old logs automatically
pub fn init_log_file() -> Result<(), AnyError> {
    log_info!("Initializing file logging");

    let mut log_file_guard = LOG_FILE.write().map_err(|e| e.to_string())?;

    let timestamp = chrono::Utc::now().format("%F_%H-%M-%S");

    std::fs::create_dir_all("logs")?;
    let log_path = format!("logs/{timestamp}.log");

    *log_file_guard = Some(std::fs::File::create(log_path)?);

    drop(log_file_guard); // Releasing the lock

    Ok(())
}
