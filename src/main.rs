// #![windows_subsystem = "windows"]

use eframe::egui;

use crate::app::ZApp;

mod app;
mod bezier;
mod color_picker;
mod curve;
mod math;
mod previewer;
mod ui_common;

/*
=============================================================
TODO:

FEATURES
* Write colored circles on spline points identifiers
* Number of points in curve (double click add, right click remove)
* Hue multiple points
* Flip Gradient button
* Hex Copy Selected color
* Hex Copy Gradient color
* Beginning/end of curve visuals (identifiers)
* Different curves
* Preset save/load
* Preset hue with stepping for points
* Visualization curves
- Brightness
- Saturation
- Hue
- Value
* Different color spaces
* Import curves to photoshop????
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
        Box::new(|cc| Box::<ZApp>::new(ZApp::new(cc))),
    );

    match eframe_result {
        Ok(_) => {}
        Err(error) => {
            println!("{:?}", error);
        }
    }
}
