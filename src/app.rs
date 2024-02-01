use std::default;

use eframe::{
    egui::{self, Frame, Id, LayerId, Painter},
    epaint::{Color32, Hsva, HsvaGamma, Pos2, Rect, Vec2},
};
use env_logger::fmt::Color;

use crate::{
    bezier::PaintBezier,
    color_picker::{self, main_color_picker, MainColorPickerData},
};

#[derive(Debug, Clone, Copy)]
enum AppState {
    Startup,
    Idle,
    Exit,
}

pub struct ZApp {
    state: AppState,
    main_color_picker_data: MainColorPickerData,
}

impl Default for ZApp {
    fn default() -> Self {
        Self {
            state: AppState::Startup,
            main_color_picker_data: MainColorPickerData {
                hsva: HsvaGamma::default(),
                alpha: egui::color_picker::Alpha::Opaque,
                paint_bezier: PaintBezier::default(),
            },
        }
    }
}

impl ZApp {
    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut visuals: egui::Visuals = egui::Visuals::dark();
        // visuals.panel_fill = Color32::from_rgba_unmultiplied(24, 36, 41, 255);
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(3.0);
    }

    fn draw_ui_menu(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // ui.color_edit_button_srgba(&mut color_picker_val);

            // self.paint_bezier.ui_content(ui);

            Frame::canvas(ui.style()).show(ui, |ui| {
                main_color_picker(ui, &mut self.main_color_picker_data);

                // self.paint_bezier.ui_content(ui);
            });
        });
    }
}

impl eframe::App for ZApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self.state {
            AppState::Startup => {
                self.startup(ctx, frame);
                self.state = AppState::Idle;
            }
            AppState::Idle => {
                self.draw_ui_menu(ctx, frame);
            }
            AppState::Exit => {
                frame.close();
            }
            _ => {
                panic!("Not a valid state {:?}", self.state);
            }
        }
    }
}
