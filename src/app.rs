use std::default;

use eframe::{
    egui::{self, Frame, Id, LayerId, Painter, Response, Ui, Widget},
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
        ui.horizontal_centered(|mut ui| {
            ui.visuals_mut().widgets.open.rounding = Rounding::default();
            ui.visuals_mut().widgets.open.expansion = 0.0;
            ui.visuals_mut().widgets.noninteractive.rounding = Rounding::default();
            ui.visuals_mut().widgets.noninteractive.expansion = 0.0;
            ui.spacing_mut().button_padding = Vec2::ZERO;
            ui.spacing_mut().combo_width = 0.0;
            ui.spacing_mut().icon_width = 0.0;
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.spacing_mut().icon_spacing = 0.0;

            let bezier = &self.main_color_picker_data.paint_bezier;
            let num_colors: usize = bezier.degree();
            let size: Vec2 = ui.available_size();
            let size_per_color_x = size.x / (num_colors as f32);
            let size_per_color = Vec2::new(size_per_color_x, size.y);
            let previewer_sizes_sum: f32 = self.previewer_data.points_preview_sizes.iter().sum();

            ui.allocate_ui(size, |ui| {
                let points = bezier.control_points(bezier_draw_size);
                for i in 0..num_colors {
                    let mut color_at_point: HsvaGamma =
                        main_color_picker_color_at(self.main_color_picker_data.hsva, &points[i])
                            .into();
                    color_at_point.h = bezier.get_hue(i);

                    let size_weight: f32 =
                        self.previewer_data.points_preview_sizes[i] / previewer_sizes_sum;
                    let response: Response = color_button(
                        ui,
                        Vec2 {
                            x: size_weight * size_per_color_x,
                            y: size_per_color.y,
                        },
                        color_at_point.into(),
                        true,
                    );
                    if response.dragged() {
                        const SENSITIVITY: f32 = 0.04;
                        self.previewer_data.points_preview_sizes[i] +=
                            response.drag_delta().x * SENSITIVITY;
                        self.previewer_data.points_preview_sizes[i] =
                            self.previewer_data.points_preview_sizes[i].max(1.0);

                        // self.num_control_points
                        let min_percentage_x = (1.0 / 4 as f32);
                        let min_preview_size = min_percentage_x * (size_weight * size_per_color_x)
                            / previewer_sizes_sum;
                        self.previewer_data.points_preview_sizes[i] =
                            self.previewer_data.points_preview_sizes[i].min(min_preview_size);
                    }
                }
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
