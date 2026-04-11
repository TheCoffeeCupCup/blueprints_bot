use colored::Colorize;
use std::io::Write;
use std::{ffi::OsStr, fs};

use crate::{AnyError, logging};

pub static LOG_FILE: std::sync::RwLock<Option<fs::File>> = std::sync::RwLock::new(None);
pub static LOG_FILES_AMOUNT_LIMIT: usize = 20;

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

fn purge_old_logs() {
    logging::info!("Purging old logs");

    match std::fs::read_dir("logs") {
        Ok(files) => {
            let mut files: Vec<_> = files
                .filter_map(|f| f.ok())
                .map(|f| f.path())
                .filter(|path| path.is_file() && path.extension() == Some(OsStr::new("log")))
                .collect();

            files.sort();

            if files.len() > LOG_FILES_AMOUNT_LIMIT {
                let remove_amount = files.len() - LOG_FILES_AMOUNT_LIMIT;

                logging::info!("Removing {remove_amount} old logs");

                for path in files.iter().take(remove_amount) {
                    fs::remove_file(path)
                        .map_err(|err| logging::error!("Couldn't remove {path:?}: {err}"))
                        .ok();
                }
            } else {
                logging::info!("Limit not exceeded, no removing needed");
            }
        }
        Err(err) => {
            logging::error!("Error reading logs directory: {err}");
        }
    }
}

pub fn init_log_file() -> Result<(), AnyError> {
    log_info!("Initializing file logging");

    let mut log_file_guard = LOG_FILE.write().map_err(|e| e.to_string())?;

    let timestamp = chrono::Utc::now().format("%F_%H-%M-%S");

    std::fs::create_dir_all("logs")?;
    let log_path = format!("logs/{timestamp}.log");

    *log_file_guard = Some(std::fs::File::create(log_path)?);

    drop(log_file_guard); // Releasing the lock

    purge_old_logs();

    Ok(())
}
