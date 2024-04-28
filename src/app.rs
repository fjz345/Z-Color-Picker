use ecolor::Color32;
use eframe::{
    egui::{InnerResponse, Response, Stroke, Ui},
    glow::HasContext,
};
use env_logger::fmt::Color;
use std::{
    borrow::BorrowMut,
    time::{Instant, SystemTime},
};

#[allow(unused_imports)]
use crate::error::Result;
use eframe::{
    egui::{self, color_picker::show_color, Layout, PointerButton, Rect, Slider, Window},
    epaint::{Pos2, Vec2},
    glow, CreationContext,
};

use crate::{
    clipboard::write_color_to_clipboard,
    color_picker::{ColorStringCopy, ControlPoint, ZColorPicker},
    math::color_lerp_ex,
    previewer::ZPreviewer,
};

#[derive(Debug, Clone, Copy)]
enum AppState {
    Startup,
    Idle,
    Exit,
}

type RgbVec = Vec<(u8, u8, u8)>;
enum ClipboardPixelPayload {
    SinglePixel(Color32),
    MultiplePixels(RgbVec),
}
struct ClipboardCopyEvent {
    mouse_pos: Pos2,
    screen_color: ClipboardPixelPayload,
}

struct AddControlPointEvent {
    mouse_pos: Pos2,
}

struct ClipboardPopup {
    open: bool,
    position: Pos2,
    open_timestamp: Instant,
    open_duration: f32,
}

impl ClipboardPopup {
    pub fn new(open: bool, position: Pos2, open_timestamp: Instant, open_duration: f32) -> Self {
        Self {
            open,
            position,
            open_timestamp,
            open_duration,
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn open(&mut self, position: Pos2) {
        self.open = true;
        self.position = position;
        self.open_timestamp = Instant::now();
    }

    pub fn update(&mut self) {
        let time_since = Instant::now()
            .duration_since(self.open_timestamp)
            .as_secs_f32();
        if time_since > self.open_duration {
            self.close();
        }
    }

    pub fn draw_ui(&mut self, ui: &mut Ui) -> Option<InnerResponse<Option<()>>> {
        let time_since_open = Instant::now()
            .duration_since(self.open_timestamp)
            .as_secs_f32();
        let alpha = (1.0 - (time_since_open / self.open_duration)).clamp(0.0, 1.0);
        self.draw_ui_clipboard_copy(ui, alpha)
    }

    fn draw_ui_clipboard_copy(
        &mut self,
        ui: &mut Ui,
        opacity: f32,
    ) -> Option<InnerResponse<Option<()>>> {
        let prev_visuals = ui.visuals_mut().clone();

        let alpha_u8 = (opacity * 255.0) as u8;
        let mut color_bg = prev_visuals.window_fill;
        color_bg[3] = alpha_u8;
        let mut color_text = prev_visuals.text_color();
        color_text[3] = alpha_u8;
        ui.visuals_mut().window_fill = color_bg;
        ui.visuals_mut().window_stroke.color = color_bg;
        ui.visuals_mut().window_stroke.width = 0.0;
        ui.visuals_mut().widgets.active.fg_stroke.color = color_text;
        ui.visuals_mut().window_shadow.extrusion = 0.0;
        ui.ctx().set_visuals(ui.visuals().clone());

        let mut should_open: bool = self.open;
        let response = Window::new("")
            .fixed_pos(&[self.position.x, self.position.y])
            .resizable(false)
            .title_bar(false)
            .open(&mut should_open)
            .auto_sized()
            .show(ui.ctx(), |ui| {
                ui.label("Copied to clipboard");

                ui.ctx().request_repaint();
            });
        self.open = should_open;

        ui.ctx().set_visuals(prev_visuals);

        response
    }
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
    debug_c: f32,
    debug_alpha: f32,
    double_click_event: Option<AddControlPointEvent>,
    middle_click_event: Option<ClipboardCopyEvent>,
    clipboard_copy_window: ClipboardPopup,
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
            debug_c: 0.0,
            debug_alpha: 0.0,
            double_click_event: None,
            middle_click_event: None,
            z_color_picker: ZColorPicker::new(),
            clipboard_copy_window: ClipboardPopup::new(
                false,
                Pos2::new(0.0, 0.0),
                Instant::now(),
                0.7,
            ),
        }
    }

    fn startup(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.scale_factor);
    }

    fn handle_doubleclick_event(&mut self, z_color_picker_response: &Response) -> bool {
        match &self.double_click_event {
            Some(pos) => {
                if z_color_picker_response.rect.contains(pos.mouse_pos) {
                    let z_color_picker_response_xy =
                        pos.mouse_pos - z_color_picker_response.rect.min;
                    let normalized_xy =
                        z_color_picker_response_xy / z_color_picker_response.rect.size();

                    let closest = self
                        .z_color_picker
                        .get_control_points_sdf_2d(normalized_xy.to_pos2());
                    const MIN_DIST: f32 = 0.1;

                    let color_xy = Pos2::new(
                        normalized_xy.x.clamp(0.0, 1.0),
                        1.0 - normalized_xy.y.clamp(0.0, 1.0),
                    );

                    match closest {
                        Some((cp, dist)) => {
                            let should_spawn_control_point = dist > MIN_DIST;
                            if should_spawn_control_point {
                                let color_hue: f32 = cp.val.h();

                                let color: [f32; 3] = [color_xy[0], color_xy[1], color_hue];
                                self.z_color_picker
                                    .spawn_control_point(ControlPoint::new(color.into(), cp.t));
                            }
                        }
                        _ => {
                            let color: [f32; 3] = [color_xy[0], color_xy[1], 0.0];
                            self.z_color_picker
                                .spawn_control_point(ControlPoint::new(color.into(), 0.0));
                        }
                    };
                    self.z_color_picker.post_update_control_points();
                }
            }
            _ => {}
        }

        false
    }

    fn draw_ui_menu(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

                self.handle_doubleclick_event(&z_color_picker_response);

                self.previewer.draw_ui(ui, self.color_copy_format);

                self.handle_middleclick_event(ui);

                // TESTING
                if self.debug_window {
                    if self.z_color_picker.control_points.len() >= 2 {
                        let src_color = self
                            .z_color_picker
                            .control_points
                            .first()
                            .unwrap()
                            .val
                            .hsv();
                        let trg_color =
                            self.z_color_picker.control_points.last().unwrap().val.hsv();
                        let res_color = color_lerp_ex(
                            src_color.into(),
                            trg_color.into(),
                            self.debug_t,
                            self.debug_c,
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

            self.clipboard_copy_window.update();
            self.clipboard_copy_window.draw_ui(ui);
        });

        if self.debug_control_points {
            self.draw_debug_control_points(ctx);
        }

        if self.debug_window {
            self.draw_debug_window(ctx);
        }
    }

    fn handle_middleclick_event(&mut self, ui: &mut Ui) -> bool {
        if let Some(event) = &self.middle_click_event {
            self.clipboard_copy_window.open(event.mouse_pos);

            // Copy to clipboard
            match &event.screen_color {
                ClipboardPixelPayload::SinglePixel(color) => {
                    write_color_to_clipboard(*color, self.color_copy_format);
                }
                ClipboardPixelPayload::MultiplePixels(colors) => todo!(),
            }
        }

        false
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
                let point = &self.z_color_picker.control_points[i];
                ui.label(format!("[{i}]"));
                ui.label(format!("- x: {}", point.val[0]));
                ui.label(format!("- y: {}", point.val[1]));
                ui.label(format!("- h: {}", point.val[2]));
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
            ui.add(Slider::new(&mut self.debug_c, 0.0..=1.0).text("debug_C"));
            ui.add(Slider::new(&mut self.debug_alpha, 0.0..=1.0).text("debug_alpha"));
        });
    }

    fn process_ctx_inputs(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // DoubleLeftClick
        self.double_click_event = None;
        ctx.input(|reader| {
            if reader.pointer.button_double_clicked(PointerButton::Primary) {
                let mouse_pos = reader.pointer.interact_pos().unwrap();
                self.double_click_event = Some(AddControlPointEvent { mouse_pos });
                println!("double click @({},{})", mouse_pos.x, mouse_pos.y);
            }
        });

        // MiddleMouseClick
        self.middle_click_event = None;
        ctx.input(|reader| {
            if reader.pointer.button_clicked(PointerButton::Middle) {
                let mouse_pos = reader.pointer.interact_pos().unwrap();
                self.middle_click_event = Some(ClipboardCopyEvent {
                    mouse_pos,
                    screen_color: ClipboardPixelPayload::SinglePixel(Color32::BLACK),
                });
                println!("middle click @({},{})", mouse_pos.x, mouse_pos.y);
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

impl eframe::App for ZApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self.state {
            AppState::Startup => {
                self.startup(ctx, frame);
                self.state = AppState::Idle;
            }
            AppState::Idle => {
                self.draw_ui_menu(ctx, frame);
                self.process_ctx_inputs(ctx, frame);
            }
            AppState::Exit => {
                frame.close();
            }
            _ => {
                panic!("Not a valid state {:?}", self.state);
            }
        }
    }

    fn post_rendering(&mut self, screen_size_px: [u32; 2], frame: &eframe::Frame) {
        if let Some(event) = &mut self.middle_click_event {
            let (r, g, b) = read_pixels_from_frame_one_pixel(
                frame,
                screen_size_px,
                self.scale_factor,
                event.mouse_pos.x,
                event.mouse_pos.y,
            );

            let color = Color32::from_rgb(r, g, b);
            dbg!(color);

            event.screen_color = ClipboardPixelPayload::SinglePixel(color);
        }
    }
}

fn read_pixels_from_frame_one_pixel(
    frame: &eframe::Frame,
    screen_size_px: [u32; 2],
    scale_factor: f32,
    x: f32,
    y: f32,
) -> (u8, u8, u8) {
    read_pixels_from_frame(frame, screen_size_px, scale_factor, x, y, 1, 1)
}

fn read_pixels_from_frame(
    frame: &eframe::Frame,
    screen_size_px: [u32; 2],
    scale_factor: f32,
    x_start: f32,
    y_start: f32,
    x_size: i32,
    y_size: i32,
) -> (u8, u8, u8) {
    let (r, g, b) = unsafe {
        let screen_scale_factor =
            scale_factor * frame.info().native_pixels_per_point.unwrap_or(1.0);
        let x_ = (x_start * screen_scale_factor).round();
        let y_ = screen_size_px[1] as i32 - (y_start * screen_scale_factor).round() as i32;

        let mut buf = [0u8; 3];
        let pixels = glow::PixelPackData::Slice(&mut buf[..]);
        frame.gl().unwrap().read_pixels(
            x_ as i32,
            y_ as i32,
            x_size,
            y_size,
            glow::RGB,
            glow::UNSIGNED_BYTE,
            pixels,
        );
        (buf[0], buf[1], buf[2])
    };

    (r, g, b)
}
