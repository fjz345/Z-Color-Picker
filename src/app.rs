use arboard::ImageData;
use ecolor::Color32;
use eframe::egui::{
    self,
    color_picker::{color_picker_color32, Alpha},
    InnerResponse, Layout, PointerButton, Rect, Response, Slider, TopBottomPanel, Ui, WidgetText,
};
use std::{
    borrow::Cow,
    collections::HashSet,
    sync::{Arc, Mutex},
    time::Instant,
};
use winapi::shared::winerror::ERROR_INCOMPATIBLE_SERVICE_SID_TYPE;

use crate::{
    clipboard::{
        write_color_to_clipboard, write_pixels_to_clipboard, ClipboardCopyEvent, ClipboardPopup,
    },
    color_picker::{ZColorPicker, ZColorPickerWrapper},
    common::{ColorStringCopy, SplineMode},
    content_windows::WindowZColorPickerOptions,
    control_point::ControlPoint,
    debug_windows::{DebugWindowControlPoints, DebugWindowTestWindow},
    image_processing::{u8u8u8_to_u8u8u8u8, u8u8u8u8_to_u8},
    preset::Preset,
    previewer::{PreviewerUiResponses, ZPreviewer},
    ui_common::{read_pixels_from_frame, ContentWindow, FramePixelRead},
};
use eframe::{
    epaint::{Pos2, Vec2},
    CreationContext,
};
use egui_tiles::{Tile, TileId, Tiles};

#[derive(Debug, Clone, Copy)]
enum AppState {
    Startup,
    Idle,
    Exit,
}

struct MouseClickEvent {
    mouse_pos: Pos2,
}

#[derive(Debug, Clone)]
pub struct ZColorPickerOptions {
    pub is_curve_locked: bool,
    pub is_hue_middle_interpolated: bool,
    pub is_insert_right: bool,
    pub is_window_lock: bool,
    pub spline_mode: SplineMode,
    pub presets: Vec<Preset>,
    pub preset_selected_index: Option<usize>,
}

impl Default for ZColorPickerOptions {
    fn default() -> Self {
        Self {
            is_curve_locked: false,
            is_hue_middle_interpolated: true,
            is_insert_right: true,
            is_window_lock: true,
            spline_mode: SplineMode::HermiteBezier,
            presets: Vec::new(),
            preset_selected_index: None,
        }
    }
}

struct ZColorPickerAppContext {
    z_color_picker: Arc<Mutex<ZColorPickerWrapper>>,
    previewer: ZPreviewer,
    color_copy_format: ColorStringCopy,
    debug_window_control_points: DebugWindowControlPoints,
    debug_window_test: DebugWindowTestWindow,
    double_click_event: Option<MouseClickEvent>,
    middle_click_event: Option<MouseClickEvent>,
    clipboard_event: Option<ClipboardCopyEvent>,
    clipboard_copy_window: ClipboardPopup,
    stored_ui_responses: PreviewerUiResponses,
    open_tabs: HashSet<String>,

    pub options: ZColorPickerOptions,
    pub options_window: WindowZColorPickerOptions,
}

impl ZColorPickerAppContext {
    pub fn default() -> Self {
        Self {
            z_color_picker: Arc::new(Mutex::new(ZColorPickerWrapper::default())),
            previewer: ZPreviewer::default(),
            color_copy_format: ColorStringCopy::default(),
            debug_window_control_points: DebugWindowControlPoints::new(Pos2 { x: 200.0, y: 200.0 }),
            debug_window_test: DebugWindowTestWindow::new(Pos2 { x: 200.0, y: 200.0 }),
            double_click_event: None,
            middle_click_event: None,
            clipboard_event: None,
            clipboard_copy_window: ClipboardPopup::new(
                false,
                Pos2::new(0.0, 0.0),
                Instant::now(),
                0.7,
            ),
            stored_ui_responses: PreviewerUiResponses::default(),
            open_tabs: HashSet::default(),
            options: ZColorPickerOptions::default(),
            options_window: WindowZColorPickerOptions::new(Pos2::new(200.0, 200.0)),
        }
    }
    pub fn new() -> Self {
        Self::default()
    }
}

const PANE_COLOR_PICKER: usize = 1;
const PANE_COLOR_PICKER_OPTIONS: usize = 2;
const PANE_COLOR_PICKER_PREVIEWER: usize = 3;
struct Pane {
    nr: usize,
    color_picker: Arc<Mutex<ZColorPickerWrapper>>,
}

impl Pane {
    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        if self.nr == PANE_COLOR_PICKER {
            // ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::WHITE);
            ui.allocate_ui(ui.max_rect().size(), |ui| {
                let color_picker_response = self
                    .color_picker
                    .lock()
                    .unwrap()
                    .draw_ui(ui, &mut ColorStringCopy::HEXNOA);

                color_picker_response
            });

            return egui_tiles::UiResponse::None;
        } else if self.nr == PANE_COLOR_PICKER_OPTIONS {
        }
        // else if self.nr == PANE_COLOR_PICKER_PREVIEWER
        // {

        // }

        let color = egui::epaint::Hsva::new(0.103 * self.nr as f32, 0.5, 0.5, 1.0);
        ui.painter().rect_filled(ui.max_rect(), 0.0, color);
        let dragged = ui
            .allocate_rect(ui.max_rect(), egui::Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab)
            .dragged();
        if dragged {
            egui_tiles::UiResponse::DragStarted
        } else {
            egui_tiles::UiResponse::None
        }
    }
}

struct TreeBehavior {}

impl egui_tiles::Behavior<Pane> for TreeBehavior {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        format!("Pane {}", pane.nr).into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        pane.ui(ui)
    }
}

pub struct ZApp {
    monitor_size: Vec2,
    scale_factor: f32,
    native_pixel_per_point: f32,
    state: AppState,
    z_color_picker_ctx: ZColorPickerAppContext,
    tree: egui_tiles::Tree<Pane>,
}

impl ZApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        // Can not get window screen size from CreationContext
        let monitor_size = Vec2::new(2560.0, 1440.0);
        const RESOLUTION_REF: f32 = 1080.0;
        let scale_factor: f32 = monitor_size.x.min(monitor_size.y) / RESOLUTION_REF;
        let native_pixel_per_point = cc.egui_ctx.native_pixels_per_point();

        let z_color_picker_ctx = ZColorPickerAppContext::default();

        let tree = Self::create_tree(z_color_picker_ctx.z_color_picker.clone());

        Self {
            monitor_size: monitor_size,
            native_pixel_per_point: native_pixel_per_point.unwrap_or(1.0),
            scale_factor: scale_factor,
            state: AppState::Startup,
            z_color_picker_ctx,
            tree: tree,
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        println!("pixels_per_point{:?}", ctx.pixels_per_point());
        println!("native_pixels_per_point{:?}", ctx.native_pixels_per_point());
        ctx.set_pixels_per_point(self.scale_factor); // Maybe mult native_pixels_per_point?
                                                     // ctx.set_debug_on_hover(true);

        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
    }

    fn draw_ui_post(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        self.update_and_draw_debug_windows(ui);
        self.z_color_picker_ctx.clipboard_copy_window.update();
        self.z_color_picker_ctx.clipboard_copy_window.draw_ui(ui);
    }

    fn create_tree(color_picker: Arc<Mutex<ZColorPickerWrapper>>) -> egui_tiles::Tree<Pane> {
        let mut tiles = egui_tiles::Tiles::default();

        let mut tabs = vec![];

        let pane_color_picker = Pane {
            nr: PANE_COLOR_PICKER,
            color_picker: color_picker.clone(),
        };
        let pane_options = Pane {
            nr: PANE_COLOR_PICKER_OPTIONS,
            color_picker: color_picker.clone(),
        };
        let pane_previewer = Pane {
            nr: PANE_COLOR_PICKER_OPTIONS,
            color_picker: color_picker.clone(),
        };

        let tile_color_picker = tiles.insert_pane(pane_color_picker);
        let tile_options = tiles.insert_pane(pane_options);
        let tile_previewer = tiles.insert_pane(pane_previewer);

        let vertical_tile = tiles.insert_vertical_tile(vec![tile_options, tile_previewer]);
        tabs.push(tiles.insert_horizontal_tile(vec![tile_color_picker, vertical_tile]));

        let root = tiles.insert_tab_tile(tabs);

        egui_tiles::Tree::new("my_tree", root, tiles)
    }

    fn draw_ui_tree(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let response = ui.with_layout(Layout::left_to_right(egui::Align::Min), |mut ui| {
                let mut behavior = TreeBehavior {};
                self.tree.ui(&mut behavior, ui);

                self.draw_ui_post(ctx, &mut ui);
            });

            self.handle_middleclick_event(&response.response);
        });
    }

    fn handle_middleclick_event(&mut self, response: &Response) -> bool {
        if response.clicked_by(PointerButton::Middle) {
            match response.interact_pointer_pos() {
                Some(pos) => {
                    let mut found_rect = None;
                    for rect in self.z_color_picker_ctx.stored_ui_responses.get_rects() {
                        if rect.contains(pos) {
                            found_rect = Some(rect.clone());
                            break;
                        }
                    }

                    let rect = found_rect.unwrap_or(Rect::from_min_size(pos, Vec2::new(1.0, 1.0)));
                    self.z_color_picker_ctx.clipboard_event = Some(ClipboardCopyEvent {
                        frame_rect: rect,
                        frame_pixels: None,
                    });
                }
                None => {}
            }
        }

        if let Some(event) = &self.z_color_picker_ctx.middle_click_event {}

        false
    }

    fn handle_clipboardcopy_event(&mut self) -> bool {
        if let Some(event) = self.z_color_picker_ctx.clipboard_event.take() {
            self.z_color_picker_ctx
                .clipboard_copy_window
                .open(event.frame_rect.min);

            // Copy to clipboard
            if let Some(frame_pixels) = event.frame_pixels {
                if frame_pixels.data.len() == 1 {
                    let color = Color32::from_rgb(
                        frame_pixels.data[0].val.0,
                        frame_pixels.data[0].val.1,
                        frame_pixels.data[0].val.2,
                    );
                    let _ =
                        write_color_to_clipboard(color, self.z_color_picker_ctx.color_copy_format);
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
        let color_picker = self.z_color_picker_ctx.z_color_picker.lock().unwrap();

        self.z_color_picker_ctx
            .debug_window_control_points
            .update(&color_picker.control_points);
        self.z_color_picker_ctx
            .debug_window_control_points
            .draw_ui(ui);

        if color_picker.control_points.len() >= 2 {
            let src_color = color_picker.control_points.first().unwrap().val().hsv();
            let trg_color = color_picker.control_points.last().unwrap().val().hsv();

            self.z_color_picker_ctx
                .debug_window_test
                .update(src_color, trg_color);
        }

        self.z_color_picker_ctx.debug_window_test.draw_ui(ui);
    }

    fn process_ctx_inputs(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut user_quit: bool = false;
        let _input_ctx = ctx.input(|r| {
            // Esc
            if r.key_down(egui::Key::Escape) {
                user_quit = true;
            }

            // DoubleLeftClick
            self.z_color_picker_ctx.double_click_event = None;
            if r.pointer.button_double_clicked(PointerButton::Primary) {
                let mouse_pos = r.pointer.interact_pos().unwrap();
                self.z_color_picker_ctx.double_click_event = Some(MouseClickEvent { mouse_pos });
                println!("double click @({},{})", mouse_pos.x, mouse_pos.y);
            }

            self.z_color_picker_ctx.middle_click_event = None;
            if r.pointer.button_clicked(PointerButton::Middle) {
                let mouse_pos: Pos2 = r.pointer.interact_pos().unwrap();
                self.z_color_picker_ctx.middle_click_event = Some(MouseClickEvent { mouse_pos });

                println!("middle click @({},{})", mouse_pos.x, mouse_pos.y);
            }

            // Debug toggles
            self.z_color_picker_ctx.double_click_event = None;
            if r.key_pressed(egui::Key::F12) {
                if self
                    .z_color_picker_ctx
                    .debug_window_control_points
                    .is_open()
                {
                    self.z_color_picker_ctx.debug_window_control_points.close();
                } else {
                    self.z_color_picker_ctx.debug_window_control_points.open();
                }

                println!(
                    "debug_control_points {}",
                    self.z_color_picker_ctx
                        .debug_window_control_points
                        .is_open()
                );
            }
            self.z_color_picker_ctx.double_click_event = None;
            if r.key_pressed(egui::Key::F11) {
                if self.z_color_picker_ctx.debug_window_test.is_open() {
                    self.z_color_picker_ctx.debug_window_test.close();
                } else {
                    self.z_color_picker_ctx.debug_window_test.open();
                }
                println!(
                    "debug_window {}",
                    self.z_color_picker_ctx.debug_window_test.is_open()
                );
            }
        });

        if user_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
                self.handle_clipboardcopy_event();
                self.draw_ui_tree(ctx, frame);
                self.process_ctx_inputs(ctx, frame);
            }
            AppState::Exit => {
                // ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {
                panic!("Not a valid state {:?}", self.state);
            }
        }

        // let screen_size_px = [ctx.used_size().x as u32, ctx.used_size().y as u32];
        // if let Some(event) = &mut self.z_color_picker_ctx.clipboard_event {
        //     let pixel_read = read_pixels_from_frame(
        //         frame,
        //         screen_size_px,
        //         self.native_pixel_per_point,
        //         self.scale_factor,
        //         event.frame_rect.min.x,
        //         event.frame_rect.max.y,
        //         event.frame_rect.size().x,
        //         event.frame_rect.size().y,
        //     );
        //     if pixel_read.data.len() > 0 {
        //         event.frame_pixels = Some(pixel_read);
        //     } else {
        //         event.frame_pixels = None;
        //     }
        // }
    }
}
