use std::default;

use eframe::{
    egui::{self, Frame, Id, LayerId, Layout, Painter, Response, Ui, Widget},
    epaint::{Color32, Hsva, HsvaGamma, Pos2, Rect, Rounding, Vec2},
    CreationContext,
};
use env_logger::fmt::Color;

use crate::{
    bezier::{Bezier, PaintBezier},
    color_picker::{
        self, main_color_picker, main_color_picker_color_at, MainColorPickerData, PreviewerData,
    },
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
    pub num_control_points: usize,
    pub bezier: Bezier<3, 4>,
    main_color_picker_data: MainColorPickerData,
    previewer_data: PreviewerData<4>,
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
            previewer_data: PreviewerData::default(),
            num_control_points: 4,
            bezier: Bezier::new(),
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.scale_factor);
    }

    fn draw_ui_menu(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let color_picker_desired_size = Vec2 {
                x: ui.available_width() * 0.5,
                y: ui.available_height(),
            };

            let mut bezier_draw_size = Vec2::default();

            ui.horizontal(|ui| {
                ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().slider_width =
                        color_picker_desired_size.x.min(color_picker_desired_size.y);
                    bezier_draw_size = main_color_picker(ui, &mut self.main_color_picker_data);
                });

                self.draw_ui_previewer(ui, bezier_draw_size);
            });
        });
    }

    fn draw_ui_previewer(&mut self, ui: &mut Ui, bezier_draw_size: Vec2) {
        ui.spacing_mut().item_spacing = Vec2::ZERO;

        let bezier = &self.main_color_picker_data.paint_bezier;
        let num_colors: usize = bezier.degree();
        let total_size: Vec2 = ui.available_size();

        let size_per_color_x = total_size.x / (num_colors as f32);
        let size_per_color_y = total_size.y;
        let previewer_sizes_sum: f32 = self.previewer_data.points_preview_sizes.iter().sum();

        let points = bezier.control_points(bezier_draw_size);
        for i in 0..num_colors {
            let mut color_at_point: HsvaGamma =
                main_color_picker_color_at(self.main_color_picker_data.hsva, &points[i]).into();
            color_at_point.h = bezier.get_hue(i);

            let size_weight: f32 = self.previewer_data.points_preview_sizes[i] * num_colors as f32
                / previewer_sizes_sum;
            let response: Response = color_button(
                ui,
                Vec2 {
                    x: size_weight * size_per_color_x,
                    y: size_per_color_y,
                },
                color_at_point.into(),
                true,
            );
            if response.dragged() {
                const SENSITIVITY: f32 = 0.02;
                self.previewer_data.points_preview_sizes[i] +=
                    response.drag_delta().x * SENSITIVITY;
                self.previewer_data.points_preview_sizes[i] =
                    self.previewer_data.points_preview_sizes[i].max(1.0);

                // self.num_control_points
                let min_percentage_x = 1.0 / num_colors as f32;
                let min_preview_size =
                    min_percentage_x * (size_weight * size_per_color_x) / previewer_sizes_sum;
                self.previewer_data.points_preview_sizes[i] =
                    self.previewer_data.points_preview_sizes[i].min(min_preview_size);
            }
        }

        let reset_button = egui::Button::new("❌").small().wrap(false).frame(false);
        const RESET_BUTTON_PERCENT_SIZE: f32 = 0.09;
        let mut reset_button_rect = ui.max_rect();
        reset_button_rect.set_width(reset_button_rect.width() * RESET_BUTTON_PERCENT_SIZE);
        reset_button_rect.set_height(reset_button_rect.height() * RESET_BUTTON_PERCENT_SIZE);

        if ui.put(reset_button_rect, reset_button).clicked() {
            self.previewer_data.reset_preview_sizes();
        }
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
