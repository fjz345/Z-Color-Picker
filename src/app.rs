use std::default;

use bspline::Interpolate;
use eframe::{
    egui::{
        self, color_picker::Alpha, lerp, Frame, Id, InnerResponse, LayerId, Layout, Painter,
        PointerButton, Response, Sense, Ui, Widget, Window,
    },
    emath,
    epaint::{Color32, Hsva, HsvaGamma, Pos2, Rect, Rounding, Vec2},
    CreationContext,
};
use env_logger::fmt::Color;
use palette::white_point::A;
use splines::{interpolate::Interpolator, Interpolation, Key, Spline};

use crate::{
    color_picker::{
        color_button_copy, format_color_as, main_color_picker, response_copy_color_on_click,
        ColorStringCopy, MainColorPickerData, PreviewerData,
    },
    curves::{control_points_to_spline, Bezier, PaintCurve},
    gradient::{color_function_gradient, mesh_gradient, vertex_gradient, Gradient},
    hsv_key_value::HsvKeyValue,
    ui_common::color_button,
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
    control_points: Vec<CONTROL_POINT_TYPE>,
    main_color_picker_data: MainColorPickerData,
    previewer_data: PreviewerData,
    color_copy_format: ColorStringCopy,
    debug_control_points: bool,
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
            main_color_picker_data: MainColorPickerData {
                hsva: HsvaGamma::default(),
                alpha: egui::color_picker::Alpha::Opaque,
                paint_curve: PaintCurve::default(),
                dragging_bezier_index: None,
                control_point_right_clicked: None,
                last_modifying_point_index: None,
                is_curve_locked: false,
                is_hue_middle_interpolated: false,
                is_insert_right: true,
                is_window_lock: true,
            },
            previewer_data: PreviewerData::new(0),
            color_copy_format: ColorStringCopy::HEX,
            debug_control_points: false,
            double_click_event: None,
            control_points: Vec::with_capacity(4),
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let mut visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.scale_factor);

        const DEFAULT_STARTUP_CONTROL_POINTS: [CONTROL_POINT_TYPE; 4] = [
            CONTROL_POINT_TYPE {
                val: [0.25, 0.33, 0.0],
            },
            CONTROL_POINT_TYPE {
                val: [0.44, 0.38, 0.1],
            },
            CONTROL_POINT_TYPE {
                val: [0.8, 0.6, 0.1],
            },
            CONTROL_POINT_TYPE {
                val: [0.9, 0.8, 0.2],
            },
        ];

        for control_point in DEFAULT_STARTUP_CONTROL_POINTS {
            self.spawn_control_point(control_point);
        }
    }

    fn spawn_control_point(&mut self, color: CONTROL_POINT_TYPE) {
        let control_point_pivot = self.main_color_picker_data.last_modifying_point_index;

        let new_index = match control_point_pivot {
            Some(index) => {
                if self.main_color_picker_data.is_insert_right {
                    index + 1
                } else {
                    index
                }
            }
            None => {
                if self.control_points.len() <= 0 {
                    0
                } else {
                    if self.main_color_picker_data.is_insert_right {
                        self.control_points.len()
                    } else {
                        0
                    }
                }
            }
        };

        self.control_points.insert(new_index, color);
        // Adding keys messes with the indicies
        self.main_color_picker_data.last_modifying_point_index = Some(new_index);
        self.main_color_picker_data.dragging_bezier_index = None;

        self.previewer_data.points_preview_sizes.push(0.0);
        self.previewer_data.reset_preview_sizes();

        println!(
            "ControlPoint#{} spawned @{},{},{}",
            self.control_points.len(),
            color[0],
            color[1],
            color[2],
        );
    }

    fn get_control_points_sdf_2d(&self, xy: Pos2) -> Option<f32> {
        let mut closest_distance_to_control_point: Option<f32> = None;
        for cp in self.control_points.iter() {
            let pos_2d = Pos2::new(cp[0].clamp(0.0, 1.0), 1.0 - cp[1].clamp(0.0, 1.0));
            let distance_2d = pos_2d.distance(xy);

            closest_distance_to_control_point = match closest_distance_to_control_point {
                Some(closest_dist_2d) => Some(closest_dist_2d.min(distance_2d)),
                None => Some(distance_2d),
            };
        }

        match closest_distance_to_control_point {
            Some(closest_dist_2d) => {
                let dist = closest_dist_2d;
                println!("Closest Dist: {}", dist);
                Some(dist)
            }
            None => {
                println!("Did not find closest dist");
                None
            }
        }
    }

    fn post_update_control_points(&mut self) {
        if self.main_color_picker_data.is_hue_middle_interpolated {
            let num_points = self.control_points.len();
            if num_points >= 2 {
                let points = &mut self.control_points[..];

                let first_index = 0;
                let last_index = points.len() - 1;
                let first_point = points[0][2];
                let last_point = points[last_index][2];
                let first_hue = points[first_index][2];
                let last_hue: f32 = points[last_index][2];
                for i in 1..last_index {
                    let t = i as f32 / points.len() as f32;
                    let hue = lerp(first_hue..=last_hue, t);
                    points[i][2] = hue;
                }
            }
        }

        if self.main_color_picker_data.is_window_lock {
            for i in 0..self.control_points.len() {
                let cp = &mut self.control_points[i];
                cp[0] = cp[0].clamp(0.0, 1.0);
                cp[1] = cp[1].clamp(0.0, 1.0);
                cp[2] = cp[2].clamp(0.0, 1.0);
            }
        }
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
                    let main_response = main_color_picker(
                        ui,
                        &mut self.control_points[..],
                        &mut self.main_color_picker_data,
                        &mut self.color_copy_format,
                    );
                    self.post_update_control_points();

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.main_color_picker_data.is_curve_locked, "🔒")
                            .on_hover_text("Apply changes to all control points");
                        ui.checkbox(
                            &mut self.main_color_picker_data.is_hue_middle_interpolated,
                            "🎨",
                        )
                        .on_hover_text("Only modify first/last control points");
                        const INSERT_RIGHT_UNICODE: &str = "👉";
                        const INSERT_LEFT_UNICODE: &str = "👈";
                        let insert_mode_unicode = if self.main_color_picker_data.is_insert_right {
                            INSERT_RIGHT_UNICODE
                        } else {
                            INSERT_LEFT_UNICODE
                        };
                        ui.checkbox(
                            &mut self.main_color_picker_data.is_insert_right,
                            insert_mode_unicode,
                        )
                        .on_hover_text(format!(
                            "Insert new points in {} direction",
                            insert_mode_unicode
                        ));
                        ui.checkbox(&mut self.main_color_picker_data.is_window_lock, "🆘")
                            .on_hover_text("Clamps the control points so they are contained");

                        egui::ComboBox::from_label("")
                            .selected_text(format!("{:?}", self.color_copy_format))
                            .show_ui(ui, |ui| {
                                ui.style_mut().wrap = Some(false);
                                ui.set_min_width(60.0);
                                ui.selectable_value(
                                    &mut self.color_copy_format,
                                    ColorStringCopy::HEX,
                                    "Hex",
                                );
                                ui.selectable_value(
                                    &mut self.color_copy_format,
                                    ColorStringCopy::HEXNOA,
                                    "Hex(no A)",
                                );
                            })
                            .response
                            .on_hover_text("Color Copy Format");
                    });

                    main_response
                });

                let main_response = left_side_reponse.inner;

                match self.double_click_event {
                    Some(pos) => {
                        if main_response.rect.contains(pos) {
                            let main_response_xy = pos - main_response.rect.min;
                            let normalized_xy = main_response_xy / main_response.rect.size();

                            let closest_distance_to_control_point =
                                self.get_control_points_sdf_2d(normalized_xy.to_pos2());
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
                                self.spawn_control_point(color.into());
                            }
                        }
                    }
                    _ => {}
                }
                match self.main_color_picker_data.control_point_right_clicked {
                    Some(a) => {
                        self.control_points.remove(a);
                        self.previewer_data.points_preview_sizes.remove(a);
                        self.previewer_data.reset_preview_sizes();
                        println!("CP {} removed, new len {}", a, self.control_points.len());
                    }
                    _ => {}
                }
                self.draw_ui_previewer(ui);
            });
        });

        if self.debug_control_points {
            self.draw_debug_control_points(ctx);
        }
    }

    fn draw_ui_previewer_control_points(&mut self, ui: &mut Ui, size: Vec2) {
        let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
        ui.allocate_rect(rect, Sense::click_and_drag());
        let mut previewer_ui_control_points =
            ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));

        previewer_ui_control_points.spacing_mut().item_spacing = Vec2::ZERO;

        let ui_size: Vec2 = previewer_ui_control_points.available_size();

        let spline = control_points_to_spline(&self.control_points[..]);
        let num_spline_points = spline.len();
        let size_per_color_x = ui_size.x / (num_spline_points as f32);
        let size_per_color_y = ui_size.y;
        let previewer_sizes_sum: f32 = self.previewer_data.points_preview_sizes.iter().sum();

        let mut points: Vec<Vec2> = Vec::with_capacity(num_spline_points);
        for cp in &self.control_points[..] {
            points.push(Vec2::new(cp[0], cp[1]));
        }

        for i in 0..num_spline_points {
            if points.len() <= i {
                break;
            }
            let color_data = &points[i];
            let color_data_hue = spline.get(i).unwrap().value[2];
            let mut color_at_point: HsvaGamma = HsvaGamma {
                h: color_data_hue,
                s: color_data.x,
                v: color_data.y,
                a: 1.0,
            };

            let size_weight: f32 = self.previewer_data.points_preview_sizes[i]
                * num_spline_points as f32
                / previewer_sizes_sum;
            let response: Response = color_button(
                &mut previewer_ui_control_points,
                Vec2 {
                    x: size_weight * size_per_color_x,
                    y: size_per_color_y,
                },
                color_at_point.into(),
                true,
            );

            response_copy_color_on_click(
                ui,
                &response,
                color_at_point,
                self.color_copy_format,
                PointerButton::Middle,
            );

            if response.dragged_by(PointerButton::Primary) {
                const PREVIEWER_DRAG_SENSITIVITY: f32 = 0.6;
                self.previewer_data.points_preview_sizes[i] +=
                    response.drag_delta().x * PREVIEWER_DRAG_SENSITIVITY;
                self.previewer_data.points_preview_sizes[i] =
                    self.previewer_data.points_preview_sizes[i].max(0.0);

                let min_percentage_x = 0.5 * (1.0 / num_spline_points as f32);
                let min_preview_size: f32 = min_percentage_x * previewer_sizes_sum;

                // TODO: loop over all and set min_preview_size
                self.previewer_data.enforce_min_size(min_preview_size);
            }

            let color_response_rect = response.ctx.screen_rect();
        }
    }

    fn draw_ui_previewer_curve(&mut self, ui: &mut Ui, size: Vec2) {
        let rect = Rect::from_min_size(ui.available_rect_before_wrap().min, size);
        ui.allocate_rect(rect, Sense::click_and_drag());
        let mut previewer_ui_curve = ui.child_ui(rect, Layout::left_to_right(egui::Align::Min));
        previewer_ui_curve.spacing_mut().item_spacing = Vec2::ZERO;

        let spline = control_points_to_spline(&self.control_points[..]);
        let colors: Vec<Color32> = spline
            .keys()
            .iter()
            .map(|a| {
                HsvaGamma {
                    h: a.value[2],
                    s: a.value[0],
                    v: a.value[1],
                    a: 1.0,
                }
                .into()
            })
            .collect();

        color_function_gradient(&mut previewer_ui_curve, rect.size(), |x| {
            if spline.len() <= 0 {
                return HsvaGamma {
                    h: 0.0,
                    s: 0.0,
                    v: 0.0,
                    a: 0.0,
                }
                .into();
            }

            let sample = spline
                .clamped_sample(x * spline.get(spline.len() - 1).unwrap().t as f32)
                .unwrap_or_default();

            HsvaGamma {
                h: sample[2],
                s: sample[0],
                v: sample[1],
                a: 1.0,
            }
            .into()
        });
    }

    fn draw_ui_previewer(&mut self, ui: &mut Ui) {
        let previewer_rect = ui.available_rect_before_wrap();

        ui.vertical(|ui| {
            self.draw_ui_previewer_control_points(ui, previewer_rect.size() * Vec2::new(1.0, 0.5));
            self.draw_ui_previewer_curve(ui, previewer_rect.size() * Vec2::new(1.0, 0.5));

            let reset_button = egui::Button::new("❌").small().wrap(true).frame(true);
            let reset_button_size: Vec2 = Vec2::new(25.0, 25.0);
            let mut reset_button_rect: Rect = Rect {
                min: previewer_rect.min,
                max: previewer_rect.min + reset_button_size,
            };

            if ui.put(reset_button_rect, reset_button).clicked() {
                self.previewer_data.reset_preview_sizes();
            }
        });
    }

    fn draw_debug_control_points(&mut self, ctx: &egui::Context) {
        let window = Window::new("=== Debug Control Points ===")
            .resizable(true)
            .constrain(true)
            .collapsible(true)
            .title_bar(true)
            .enabled(true);

        window.show(ctx, |ui| {
            for i in 0..self.control_points.len() {
                let point = self.control_points[i];
                ui.label(format!("[{i}]"));
                ui.label(format!("- x: {}", point[0]));
                ui.label(format!("- y: {}", point[1]));
                ui.label(format!("- h: {}", point[2]));
                ui.label(format!(""));
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
    }
}
