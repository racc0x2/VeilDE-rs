mod gui;

use native_dialog::{DialogBuilder, MessageLevel};

fn main() {
    match gui::init() {
        Ok(_) => {
            DialogBuilder::message()
                .set_title("VeilDE-rs - Success")
                .set_text("Nothing failed!")
                .set_level(MessageLevel::Info)
                .alert()
                .show()
                .unwrap();
        },

        Err(e) => {
            eprintln!("{:?}", e);

            DialogBuilder::message()
                .set_title("VeilDE-rs - Failure")
                .set_text(format!("{:#}", e))
                .set_level(MessageLevel::Error)
                .alert()
                .show()
                .unwrap();
        },
    }
}
