use std::default;

use eframe::{
    egui::{self, Frame, Id, LayerId, Painter, Response, Ui, Widget},
    epaint::{Color32, Hsva, HsvaGamma, Pos2, Rect, Rounding, Vec2},
    CreationContext,
};
use env_logger::fmt::Color;

use crate::{
    bezier::PaintBezier,
    color_picker::{self, main_color_picker, main_color_picker_color_at, MainColorPickerData},
    ui_common::color_button,
};

#[derive(Debug, Clone, Copy)]
enum AppState {
    Startup,
    Idle,
    Exit,
}

pub struct ZApp {
    scale_factor: f32,
    state: AppState,
    main_color_picker_data: MainColorPickerData,
}

impl ZApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let monitor_size = cc.integration_info.window_info.monitor_size.unwrap();
        const RESOLUTION_REF: f32 = 1080.0;
        let scale_factor = monitor_size.x.min(monitor_size.y) / RESOLUTION_REF;
        Self {
            scale_factor: scale_factor,
            state: AppState::Startup,
            main_color_picker_data: MainColorPickerData {
                hsva: HsvaGamma::default(),
                alpha: egui::color_picker::Alpha::Opaque,
                paint_bezier: PaintBezier::default(),
                dragging_bezier_index: None,
                last_modifying_bezier_index: 0,
            },
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut visuals: egui::Visuals = egui::Visuals::dark();
        // visuals.panel_fill = Color32::from_rgba_unmultiplied(24, 36, 41, 255);
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.scale_factor);
    }

    fn draw_ui_menu(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut bezier_draw_size = Vec2::default();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    Frame::canvas(ui.style()).show(ui, |ui| {
                        bezier_draw_size = main_color_picker(ui, &mut self.main_color_picker_data);
                    });
                });

                ui.vertical(|ui| self.draw_ui_previewer(ui, bezier_draw_size));
            });
        });
    }

    fn draw_ui_previewer(&mut self, ui: &mut Ui, bezier_draw_size: Vec2) {
        ui.horizontal_centered(|ui| {
            ui.visuals_mut().widgets.open.rounding = Rounding::default();
            ui.visuals_mut().widgets.open.expansion = 0.0;
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.spacing_mut().combo_width = 0.0;
            ui.spacing_mut().icon_width = 0.0;
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.spacing_mut().icon_spacing = 0.0;

            let bezier = &self.main_color_picker_data.paint_bezier;
            let num_colors: usize = bezier.degree();
            let size = ui.available_size();
            let size_per_color_x = size.x / (num_colors as f32);
            let size_per_color = Vec2::new(size_per_color_x, size.y);

            let points = bezier.control_points(bezier_draw_size);
            for i in 0..num_colors {
                let mut color_at_point: HsvaGamma =
                    main_color_picker_color_at(self.main_color_picker_data.hsva, &points[i]).into();
                color_at_point.h = bezier.get_hue(i);
                color_button(ui, size_per_color, color_at_point.into(), true);
            }
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
