mod gui;
mod utils;

use native_dialog::DialogBuilder;

fn main() {
    match gui::init() {
        Ok(_) => {
            DialogBuilder::message()
                .set_title("VeilDE-rs")
                .set_text("Nothing failed!")
                .confirm()
                .show()
                .unwrap();
        },

        Err(e) => {
            eprintln!("{:?}", e);

            DialogBuilder::message()
                .set_title("VeilDE-rs")
                .set_text(format!("Error: {:#}", e))
                .alert()
                .show()
                .unwrap();
        },
    }
}
