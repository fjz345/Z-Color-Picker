use std::default;

use eframe::{
    egui::{self, Frame, Id, LayerId, Painter, Response, Widget},
    epaint::{Color32, Hsva, HsvaGamma, Pos2, Rect, Rounding, Vec2},
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

            let mut bezier_draw_size = Vec2::default();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    Frame::canvas(ui.style()).show(ui, |ui| {
                        bezier_draw_size = main_color_picker(ui, &mut self.main_color_picker_data);
                    });
                });

                ui.vertical(|ui| {
                    // let size = ui.spacing().interact_size;

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
                            let color_at_point: Color32 = main_color_picker_color_at(
                                self.main_color_picker_data.hsva,
                                &points[i],
                            );
                            color_button(ui, size_per_color, color_at_point, true);
                        }
                    });
                });
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
