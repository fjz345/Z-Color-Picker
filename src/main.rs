// #![windows_subsystem = "windows"]
#![allow(dead_code)]
#![allow(unreachable_patterns)]

use eframe::egui;

use crate::app::ZApp;

mod app;
mod clipboard;
mod color_picker;
mod common;
mod content_windows;
mod control_point;
mod curves;
mod debug_windows;
mod error;
mod fs;
mod gradient;
mod hsv_key_value;
mod image_processing;
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

    eframe::run_native(
        "Z Color Picker",
        options,
        Box::new(|cc| Box::<ZApp>::new(ZApp::new(cc))),
    );
}
