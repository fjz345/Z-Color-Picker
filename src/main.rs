// #![windows_subsystem = "windows"]
// #![allow(dead_code)]
// #![allow(unreachable_patterns)]

use eframe::egui::{self};

use crate::{logger::LogCollector, ui_egui::app::ZApp};

mod color_space;
mod common;
mod datatypes;
mod error;
mod fs;
mod logger;
mod math;
mod preset;
mod ui_egui;

fn main() -> eframe::Result {
    let log_buffer = LogCollector::init().expect("Failed to init logger");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([2560.0, 1440.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Z-Color-Picker",
        native_options,
        Box::new(move |cc: &eframe::CreationContext<'_>| {
            if let Some(app) = cc
                .storage
                .and_then(|storage| storage.get_string(eframe::APP_KEY))
                .and_then(|json| restore_app(&json))
            {
                return Ok(Box::new(app));
            }

            let app = ZApp::new(log_buffer.clone());
            Ok(Box::<ZApp>::new(app))
        }),
    )
}

fn restore_app(json: &str) -> Option<ZApp> {
    match serde_json::from_str::<ZApp>(json) {
        Ok(mut app) => {
            log::info!("Found previous app storage");
            app.request_init();
            Some(app)
        }
        Err(e) => {
            log::warn!("Ignoring stored app state: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_app_ignores_unparseable_state() {
        assert!(restore_app("{ garbage").is_none());
    }
}
