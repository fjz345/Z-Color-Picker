use ecolor::Color32;
use eframe::{
    egui::{
        self, color_picker::show_color, ComboBox, Layout, PointerButton, Rect, Slider, Ui, Window,
    },
    epaint::{Pos2, Vec2},
    CreationContext,
};

use crate::{
    color_picker::{main_color_picker, ColorStringCopy, SplineMode, ZColorPicker},
    math::{color_lerp, color_lerp_ex, hue_lerp},
    preset::{
        delete_preset_from_disk, get_preset_save_path, load_presets, save_preset_to_disk, Preset,
        PresetData,
    },
    previewer::{ui_previewer, PreviewerData, ZPreviewer},
    CONTROL_POINT_TYPE,
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
    z_color_picker: ZColorPicker,
    previewer: ZPreviewer,
    color_copy_format: ColorStringCopy,
    debug_control_points: bool,
    debug_window: bool,
    debug_t: f32,
    debug_C: f32,
    debug_alpha: f32,
    double_click_event: Option<Pos2>,
}

impl ZApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let monitor_size = cc.integration_info.window_info.monitor_size.unwrap();
        const RESOLUTION_REF: f32 = 1080.0;
        let scale_factor: f32 = monitor_size.x.min(monitor_size.y) / RESOLUTION_REF;
        Self {
            scale_factor: scale_factor,
            state: AppState::Startup,
            previewer: ZPreviewer::new(),
            color_copy_format: ColorStringCopy::HEX,
            debug_control_points: false,
            debug_window: false,
            debug_t: 0.0,
            debug_C: 0.0,
            debug_alpha: 0.0,
            double_click_event: None,
            z_color_picker: ZColorPicker::new(),
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

            ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                ui.spacing_mut().slider_width =
                    color_picker_desired_size.x.min(color_picker_desired_size.y);

                let left_side_reponse = ui.vertical(|ui| {
                    let z_color_picker_response =
                        self.z_color_picker.draw_ui(ui, &mut self.color_copy_format);

                    z_color_picker_response
                });

                let z_color_picker_response = left_side_reponse.inner;

                self.previewer.update(
                    &self.z_color_picker.control_points,
                    self.z_color_picker.spline_mode,
                );

                match self.double_click_event {
                    Some(pos) => {
                        if z_color_picker_response.rect.contains(pos) {
                            let z_color_picker_response_xy = pos - z_color_picker_response.rect.min;
                            let normalized_xy =
                                z_color_picker_response_xy / z_color_picker_response.rect.size();

                            let closest_distance_to_control_point = self
                                .z_color_picker
                                .get_control_points_sdf_2d(normalized_xy.to_pos2());
                            const MIN_DIST: f32 = 0.1;

                            let should_spawn_control_point = match closest_distance_to_control_point
                            {
                                Some(dist) => {
                                    let dist = closest_distance_to_control_point.unwrap();
                                    dist > MIN_DIST
                                }
                                _ => true,
                            };
                            if should_spawn_control_point {
                                let color_hue = 0.5;
                                let color_xy = Pos2::new(
                                    normalized_xy.x.clamp(0.0, 1.0),
                                    1.0 - normalized_xy.y.clamp(0.0, 1.0),
                                );
                                let color = [color_xy[0], color_xy[1], color_hue];
                                self.z_color_picker.spawn_control_point(color.into());
                            }
                        }
                    }
                    _ => {}
                }

                self.previewer.draw_ui(ui, self.color_copy_format);

                // TESTING
                if self.debug_window {
                    if self.z_color_picker.control_points.len() >= 2 {
                        let src_color = self.z_color_picker.control_points.first().unwrap().hsv();
                        let trg_color = self.z_color_picker.control_points.last().unwrap().hsv();
                        let res_color = color_lerp_ex(
                            src_color.into(),
                            trg_color.into(),
                            self.debug_t,
                            self.debug_C,
                            self.debug_alpha,
                        );

                        ui.allocate_ui_at_rect(
                            Rect::from_center_size(
                                Pos2::new(500.0, 500.0),
                                Vec2::new(500.0, 500.0),
                            ),
                            |ui| {
                                let show_size = 100.0;
                                show_color(ui, src_color, Vec2::new(show_size, show_size));
                                show_color(ui, trg_color, Vec2::new(show_size, show_size));
                                show_color(ui, res_color, Vec2::new(show_size, show_size));
                            },
                        );
                    }
                }
            });
        });

        if self.debug_control_points {
            self.draw_debug_control_points(ctx);
        }

        if self.debug_window {
            self.draw_debug_window(ctx);
        }
    }

    fn draw_debug_control_points(&mut self, ctx: &egui::Context) {
        let window = Window::new("=== Debug Control Points ===")
            .resizable(true)
            .constrain(true)
            .collapsible(true)
            .title_bar(true)
            .enabled(true);

        window.show(ctx, |ui| {
            for i in 0..self.z_color_picker.control_points.len() {
                let point = self.z_color_picker.control_points[i];
                ui.label(format!("[{i}]"));
                ui.label(format!("- x: {}", point[0]));
                ui.label(format!("- y: {}", point[1]));
                ui.label(format!("- h: {}", point[2]));
                ui.label(format!(""));
            }
        });
    }
    fn draw_debug_window(&mut self, ctx: &egui::Context) {
        let window = Window::new("=== Debug Window ===")
            .resizable(true)
            .constrain(true)
            .collapsible(true)
            .title_bar(true)
            .enabled(true);

        window.show(ctx, |ui| {
            ui.add(Slider::new(&mut self.debug_t, 0.0..=1.0).text("debug_t"));
            ui.add(Slider::new(&mut self.debug_C, 0.0..=1.0).text("debug_C"));
            ui.add(Slider::new(&mut self.debug_alpha, 0.0..=1.0).text("debug_alpha"));
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

        // Register add control point
        self.double_click_event = None;
        ctx.input(|reader| {
            if reader.pointer.button_double_clicked(PointerButton::Primary) {
                self.double_click_event = Some(reader.pointer.interact_pos().unwrap());
                println!(
                    "double click @({},{})",
                    self.double_click_event.unwrap().x,
                    self.double_click_event.unwrap().y
                );
            }
        });

        // Debug toggles
        ctx.input(|reader| {
            if reader.key_pressed(egui::Key::F12) {
                self.debug_control_points = !self.debug_control_points;
                println!("debug_control_points {}", self.debug_control_points);
            }
        });

        // Debug toggles
        ctx.input(|reader| {
            if reader.key_pressed(egui::Key::F11) {
                self.debug_window = !self.debug_window;
                println!("debug_window {}", self.debug_window);
            }
        });
    }
}
