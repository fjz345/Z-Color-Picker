// #![windows_subsystem = "windows"]

use eframe::egui;

use crate::app::ZApp;

mod app;
mod color_picker;
mod curves;
mod math;
mod previewer;
mod ui_common;

/*
=============================================================
TODO:

FEATURES
* Fix clamp to picker window mode, (fix bug where only [0,1] is saved for V (y) of control_points)
* Gradient Curve
* Hex Copy Gradient color
* Flip Control Points button
* Beginning/end of curve visuals (identifiers)
* Hue multiple control points
* Different curve types
    - Bezier
    - Polynomial
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
