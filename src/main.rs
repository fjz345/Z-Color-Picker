// #![windows_subsystem = "windows"]
#![allow(dead_code)]
#![allow(unreachable_patterns)]

use eframe::{egui, WindowBuilderHook};

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

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([2560.0, 1440.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Z-Color-Picker",
        native_options,
        Box::new(|cc: &eframe::CreationContext<'_>| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            let app = ZApp::new(cc);
            Ok(Box::<ZApp>::new(app))
        }),
    )
}
