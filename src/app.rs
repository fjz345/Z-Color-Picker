use arboard::ImageData;
use ecolor::Color32;
use eframe::egui::{InnerResponse, Response, Ui};
use std::{borrow::Cow, time::Instant};

use eframe::{
    egui::{self, Layout, PointerButton, Rect, Window},
    epaint::{Pos2, Vec2},
    CreationContext,
};

use crate::{
    clipboard::{write_color_to_clipboard, write_pixels_to_clipboard},
    color_picker::{ColorStringCopy, ZColorPicker},
    control_point::ControlPoint,
    debug_windows::{DebugWindowControlPoints, DebugWindowTestWindow},
    image_processing::{u8u8u8_to_u8u8u8u8, u8u8u8u8_to_u8},
    previewer::{PreviewerUiResponses, ZPreviewer},
    ui_common::{read_pixels_from_frame, ContentWindow, FramePixelRead},
};

#[derive(Debug, Clone, Copy)]
enum AppState {
    Startup,
    Idle,
    Exit,
}

struct MouseClickEvent {
    mouse_pos: Pos2,
}

struct ClipboardCopyEvent {
    frame_rect: Rect,
    frame_pixels: Option<FramePixelRead>,
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
    monitor_size: Vec2,
    scale_factor: f32,
    state: AppState,
    z_color_picker: ZColorPicker,
    previewer: ZPreviewer,
    color_copy_format: ColorStringCopy,
    debug_window_control_points: DebugWindowControlPoints,
    debug_window_test: DebugWindowTestWindow,
    double_click_event: Option<MouseClickEvent>,
    middle_click_event: Option<MouseClickEvent>,
    clipboard_event: Option<ClipboardCopyEvent>,
    clipboard_copy_window: ClipboardPopup,
    stored_ui_responses: PreviewerUiResponses,
}

impl ZApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let monitor_size = cc.integration_info.window_info.monitor_size.unwrap();
        const RESOLUTION_REF: f32 = 1080.0;
        let scale_factor: f32 = monitor_size.x.min(monitor_size.y) / RESOLUTION_REF;

        Self {
            monitor_size: monitor_size,
            scale_factor: scale_factor,
            state: AppState::Startup,
            previewer: ZPreviewer::new(),
            color_copy_format: ColorStringCopy::HEX,
            double_click_event: None,
            middle_click_event: None,
            z_color_picker: ZColorPicker::new(),
            clipboard_copy_window: ClipboardPopup::new(
                false,
                Pos2::new(0.0, 0.0),
                Instant::now(),
                0.7,
            ),
            stored_ui_responses: PreviewerUiResponses::default(),
            clipboard_event: None,
            debug_window_control_points: DebugWindowControlPoints::new(Pos2 { x: 200.0, y: 0.0 }),
            debug_window_test: DebugWindowTestWindow::new(Pos2 { x: 200.0, y: 0.0 }),
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.scale_factor);
        frame.set_window_size(self.monitor_size);
        frame.set_maximized(true);
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

                let z_color_picker_response_option = left_side_reponse.inner;

                self.previewer.update(
                    &self.z_color_picker.control_points,
                    self.z_color_picker.options.spline_mode,
                );
                self.stored_ui_responses = self.previewer.draw_ui(ui, self.color_copy_format);

                if let Some(z_color_picker_response) = z_color_picker_response_option {
                    self.handle_doubleclick_event(&z_color_picker_response);
                } else {
                    println!("Something went very wrong with z_color_picker, response missing");
                }

                self.handle_middleclick_event(ui);

                self.update_and_draw_debug_windows(ui);
            });

            self.clipboard_copy_window.update();
            self.clipboard_copy_window.draw_ui(ui);
        });
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
                                let color_hue: f32 = cp.val().h();

                                let color: [f32; 3] = [color_xy[0], color_xy[1], color_hue];
                                self.z_color_picker.spawn_control_point(cp.clone());
                            }
                        }
                        _ => {
                            let color: [f32; 3] = [color_xy[0], color_xy[1], 0.0];
                            let new_cp = ControlPoint::new_simple(color.into(), 0.0);
                            self.z_color_picker.spawn_control_point(new_cp);
                        }
                    };
                    self.z_color_picker.post_update_control_points();
                }
            }
            _ => {}
        }

        false
    }

    fn handle_middleclick_event(&mut self, _ui: &mut Ui) -> bool {
        if let Some(event) = &self.middle_click_event {
            let mut found_rect = None;
            for rect in self.stored_ui_responses.get_rects() {
                if rect.contains(event.mouse_pos) {
                    found_rect = Some(rect.clone());
                    break;
                }
            }

            let rect =
                found_rect.unwrap_or(Rect::from_min_size(event.mouse_pos, Vec2::new(1.0, 1.0)));
            self.clipboard_event = Some(ClipboardCopyEvent {
                frame_rect: rect,
                frame_pixels: None,
            });
        }

        false
    }

    fn handle_clipboardcopy_event(&mut self) -> bool {
        if let Some(event) = self.clipboard_event.take() {
            self.clipboard_copy_window.open(event.frame_rect.min);

            // Copy to clipboard
            if let Some(frame_pixels) = event.frame_pixels {
                if frame_pixels.data.len() == 1 {
                    let color = Color32::from_rgb(
                        frame_pixels.data[0].val.0,
                        frame_pixels.data[0].val.1,
                        frame_pixels.data[0].val.2,
                    );
                    let _ = write_color_to_clipboard(color, self.color_copy_format);
                } else if frame_pixels.data.len() > 1 {
                    let a_padded = u8u8u8_to_u8u8u8u8(&frame_pixels.data[..]);
                    let u8_stream = u8u8u8u8_to_u8(&a_padded[..]);
                    let cow = Cow::Owned(u8_stream);
                    let data = ImageData {
                        width: frame_pixels.width,
                        height: frame_pixels.height,
                        bytes: cow,
                    };
                    // let _ = write_pixels_to_test_ppm(&data, copy);
                    let _ = write_pixels_to_clipboard(data);
                } else {
                    println!("clipboard event could not be processed, colors len was 0");
                }
            } else {
                println!("clipboard event could not be processed, did not have any colors set");
            }

            return true;
        }

        false
    }

    fn update_and_draw_debug_windows(&mut self, ui: &mut Ui) {
        self.debug_window_control_points
            .update(&self.z_color_picker.control_points);
        self.debug_window_control_points.draw_ui(ui);

        if self.z_color_picker.control_points.len() >= 2 {
            let src_color = self
                .z_color_picker
                .control_points
                .first()
                .unwrap()
                .val()
                .hsv();
            let trg_color = self
                .z_color_picker
                .control_points
                .last()
                .unwrap()
                .val()
                .hsv();

            self.debug_window_test.update(src_color, trg_color);
        }

        self.debug_window_test.draw_ui(ui);
    }

    fn process_ctx_inputs(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // DoubleLeftClick
        self.double_click_event = None;
        ctx.input(|reader| {
            if reader.pointer.button_double_clicked(PointerButton::Primary) {
                let mouse_pos = reader.pointer.interact_pos().unwrap();
                self.double_click_event = Some(MouseClickEvent { mouse_pos });
                println!("double click @({},{})", mouse_pos.x, mouse_pos.y);
            }
        });

        // MiddleMouseClick
        self.middle_click_event = None;
        ctx.input(|reader| {
            if reader.pointer.button_clicked(PointerButton::Middle) {
                let mouse_pos: Pos2 = reader.pointer.interact_pos().unwrap();
                self.middle_click_event = Some(MouseClickEvent { mouse_pos });

                println!("middle click @({},{})", mouse_pos.x, mouse_pos.y);
            }
        });

        // Debug toggles
        ctx.input(|reader| {
            if reader.key_pressed(egui::Key::F12) {
                if self.debug_window_control_points.is_open() {
                    self.debug_window_control_points.close();
                } else {
                    self.debug_window_control_points.open();
                }

                println!(
                    "debug_control_points {}",
                    self.debug_window_control_points.is_open()
                );
            }
        });

        // Debug toggles
        ctx.input(|reader| {
            if reader.key_pressed(egui::Key::F11) {
                if self.debug_window_test.is_open() {
                    self.debug_window_test.close();
                } else {
                    self.debug_window_test.open();
                }
                println!("debug_window {}", self.debug_window_test.is_open());
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
                self.handle_clipboardcopy_event();
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
        if let Some(event) = &mut self.clipboard_event {
            let pixel_read = read_pixels_from_frame(
                frame,
                screen_size_px,
                self.scale_factor,
                event.frame_rect.min.x,
                event.frame_rect.max.y,
                event.frame_rect.size().x,
                event.frame_rect.size().y,
            );
            if pixel_read.data.len() > 0 {
                event.frame_pixels = Some(pixel_read);
            } else {
                event.frame_pixels = None;
            }
        }
    }
}
