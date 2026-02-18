mod app;
mod consts;
mod renderer;

use chrono::{Datelike, Local, Timelike};
use native_dialog::{DialogBuilder, MessageLevel};
use anyhow::{Context, Result};

fn save_log(log: &str) -> Result<()> {
    let now = Local::now();

    let name = format!(
        "{}{:02}{:02}{:02}{:02}{:02}{:03}.log",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        now.timestamp_subsec_millis()
    );

    std::fs::create_dir("crash")?;
    std::fs::write(format!("crash/{name}"), log.as_bytes()).context("Failed to write log file")?;

    Ok(())
}

fn main() {
    match app::init() {
        Ok(_) => {
            DialogBuilder::message()
                .set_title("VeilDE-rs - Success")
                .set_text("Nothing failed!")
                .set_level(MessageLevel::Info)
                .alert()
                .show()
                .expect("Failed to show dialog");
        },

        Err(e) => {
            let display = format!("{:?}", e);
            eprintln!("{display}");

            DialogBuilder::message()
                .set_title("VeilDE-rs - Failure")
                .set_text(display.clone())
                .set_level(MessageLevel::Error)
                .alert()
                .show()
                .expect("Failed to show dialog");

            save_log(display.as_str()).expect("Failed to save log file");
        },
    }
}
