// #![windows_subsystem = "windows"]
#![allow(dead_code)]
#![allow(unreachable_patterns)]

use eframe::egui;
use hsv_key_value::HsvKeyValue;

use crate::app::ZApp;

mod app;
mod color_picker;
mod curves;
mod fs;
mod gradient;
mod hsv_key_value;
mod math;
mod preset;
mod previewer;
mod ui_common;

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

type ControlPointType = HsvKeyValue;
