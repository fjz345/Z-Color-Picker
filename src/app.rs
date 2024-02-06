use std::default;

use eframe::{
    egui::{self, Frame, Id, LayerId, Layout, Painter, Response, Sense, Ui, Widget},
    epaint::{Color32, Hsva, HsvaGamma, Pos2, Rect, Rounding, Vec2},
    CreationContext,
};
use env_logger::fmt::Color;

use crate::{
    bezier::{Bezier, PaintBezier},
    color_picker::{
        format_color_as, main_color_picker, xyz_to_hsva, ColorStringCopy, MainColorPickerData,
        PreviewerData,
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
                bezier_right_clicked: None,
                last_modifying_bezier_index: 0,
                is_curve_locked: false,
                is_hue_middle_interpolated: true,
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
                y: ui.available_height().min(ui.available_width()),
            };

            let mut bezier_draw_size = Vec2::default();

            ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                ui.spacing_mut().slider_width =
                    color_picker_desired_size.x.min(color_picker_desired_size.y);
                let (main_response, bezier_draw_size) =
                    main_color_picker(ui, &mut self.main_color_picker_data);

                if main_response.double_clicked() {
                    self.num_control_points = self.num_control_points + 1;
                    println!("num_points_inc, new num_points {}", self.num_control_points);
                }
                match self.main_color_picker_data.bezier_right_clicked {
                    Some(a) => {
                        self.num_control_points = self.num_control_points.max(1) - 1;
                        println!(
                            "point_removed {}, new num_points {}",
                            a, self.num_control_points
                        );
                    }
                    _ => {}
                }
                self.draw_ui_previewer(ui, bezier_draw_size);

                println!(
                    "{}",
                    format_color_as(Color32::DARK_RED, ColorStringCopy::HEX)
                );
            });
        });
    }

    fn draw_ui_previewer(&mut self, ui: &mut Ui, bezier_draw_size: Vec2) {
        let previewer_rect = ui.available_rect_before_wrap();
        let mut previewer_ui = ui.child_ui(previewer_rect, Layout::left_to_right(egui::Align::Min));
        previewer_ui.spacing_mut().item_spacing = Vec2::ZERO;

        let bezier = &self.main_color_picker_data.paint_bezier;
        let total_size: Vec2 = previewer_ui.available_size();

        let size_per_color_x = total_size.x / (self.num_control_points as f32);
        let size_per_color_y = total_size.y;
        let previewer_sizes_sum: f32 = self.previewer_data.points_preview_sizes.iter().sum();

        let points: Vec<Vec2> = bezier.control_points(bezier_draw_size);
        for i in 0..self.num_control_points {
            if points.len() <= i {
                break;
            }
            let color_data = &points[i];
            let color_data_hue = bezier.get_hue(i);
            let mut color_at_point: HsvaGamma =
                xyz_to_hsva(color_data_hue, color_data.x, color_data.y).into();

            let size_weight: f32 = self.previewer_data.points_preview_sizes[i]
                * self.num_control_points as f32
                / previewer_sizes_sum;
            let response: Response = color_button(
                &mut previewer_ui,
                Vec2 {
                    x: size_weight * size_per_color_x,
                    y: size_per_color_y,
                },
                color_at_point.into(),
                true,
            );
            if response.dragged() {
                const PREVIEWER_DRAG_SENSITIVITY: f32 = 0.6;
                self.previewer_data.points_preview_sizes[i] +=
                    response.drag_delta().x * PREVIEWER_DRAG_SENSITIVITY;
                self.previewer_data.points_preview_sizes[i] =
                    self.previewer_data.points_preview_sizes[i].max(0.0);

                let min_percentage_x = 0.5 * (1.0 / self.num_control_points as f32);
                let min_preview_size: f32 = min_percentage_x * previewer_sizes_sum;

                // TODO: loop over all and set min_preview_size
                self.previewer_data.enforce_min_size(min_preview_size);
            }

            let color_response_rect = response.ctx.screen_rect();
        }

        let reset_button = egui::Button::new("❌").small().wrap(true).frame(true);
        let reset_button_size: Vec2 = Vec2::new(25.0, 25.0);
        let mut reset_button_rect: Rect = Rect {
            min: previewer_rect.min,
            max: previewer_rect.min + reset_button_size,
        };

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
