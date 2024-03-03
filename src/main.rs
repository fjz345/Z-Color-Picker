// #![windows_subsystem = "windows"]

use eframe::egui;
use hsv_key_value::HsvKeyValue;

use crate::app::ZApp;

mod app;
mod color_picker;
mod curves;
mod gradient;
mod hsv_key_value;
mod math;
mod previewer;
mod ui_common;

/*
=============================================================
TODO:

BUGS
* Bezier mode hue interpolation
* HermiteBezier, can not modify t's in previewer
* Polynomial

FEATURES'
* Quantizize gradient previewer
* Lerp hue at 0.5 dist to get smoother result
* Preset save/load
* standard Color pick for control points
* Preset hue with stepping for points
* Visualization curves
- Brightness
- Saturation
- Hue
- Value
* User feedback when copying color value
* Different color spaces
* Lerp color instead of lerpx,lerpy,lerp_hue
* Import curves to photoshop????
* Better ui widget dynamic rezizing
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

type CONTROL_POINT_TYPE = HsvKeyValue;
