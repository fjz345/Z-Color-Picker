// #![windows_subsystem = "windows"]
// #![allow(dead_code)]
// #![allow(unreachable_patterns)]

use std::env;

use eframe::egui::{self};

use crate::{logger::LogCollector, ui_egui::app::ZApp};

mod common;
mod datatypes;
mod error;
mod fs;
mod image_processing;
mod logger;
mod math;
mod preset;
mod ui_egui;

fn main() -> eframe::Result {
    unsafe { env::set_var("RUST_LOG", "debug") }; // or "info" or "debug"

    let log_buffer = LogCollector::init().expect("Failed to init logger");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([2560.0, 1440.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Z-Color-Picker",
        native_options,
        Box::new(move |cc: &eframe::CreationContext<'_>| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);

            #[cfg(feature = "serde")]
            {
                // Try to load saved state from storage
                if let Some(storage) = cc.storage {
                    if let Some(json) = storage.get_string(eframe::APP_KEY) {
                        if let Ok(mut app) = serde_json::from_str::<ZApp>(&json) {
                            log::info!("Found previous app storage");
                            app.request_init();
                            return Ok(Box::new(app));
                        }
                    }
                }
            }

            let app = ZApp::new(cc, log_buffer.clone());
            Ok(Box::<ZApp>::new(app))
        }),
    )
}
