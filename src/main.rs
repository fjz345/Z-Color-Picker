use eframe::egui;

use crate::app::ZApp;

mod app;
mod bezier;
mod color_picker;
mod previewer;
mod ui_common;

/*
=============================================================
TODO:

ISSUES
* remove console from release binary
* fix preview cell spacing

FEATURES
* Hex Copy Selected color
* Hex Copy Gradient color
* Sample along bezier
* Different curves
* Number of points in curve (double click add, right click remove)
* translate all curve points
* change hue all curve points
* Preview colors, change preview cell sizes by sliding
* Preset save/load
* beginning/end of curve visuals
* Preset hue with stepping for points
=============================================================
*/
fn main() {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };

    let eframe_result = eframe::run_native(
        "Z Color Picker",
        options,
        Box::new(|_cc| Box::<ZApp>::default()),
    );

    match eframe_result {
        Ok(_) => {}
        Err(error) => {
            println!("{:?}", error);
        }
    }
}
