use arboard::ImageData;
use ecolor::Color32;
use eframe::{
    egui::{
        self,
        color_picker::{color_picker_color32, Alpha},
        InnerResponse, Layout, PointerButton, Rect, Response, Slider, TopBottomPanel, Ui,
        WidgetText,
    },
    epaint::tessellator::Path,
};
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashSet,
    ops::DerefMut,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};
#[cfg(windows)]
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
    logger::{ui_log_window, LogCollector},
    preset::{Preset, SAVED_FOLDER_NAME},
    previewer::{self, PreviewerUiResponses, ZPreviewer},
    ui_common::{read_pixels_from_frame, ContentWindow, FramePixelRead},
};
use eframe::{
    epaint::{Pos2, Vec2},
    CreationContext,
};
use egui_tiles::{Tile, TileId, Tiles, Tree};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
enum AppState {
    #[default]
    Startup,
    Idle,
    Exit,
}

#[derive(Debug)]
struct MouseClickEvent {
    mouse_pos: Pos2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Serialize, Deserialize, Debug, Default)]
struct ZColorPickerAppContext {
    z_color_picker: Rc<RefCell<ZColorPickerWrapper>>,
    previewer: ZPreviewer,
    color_copy_format: ColorStringCopy,
    #[serde(skip)]
    debug_window_control_points: DebugWindowControlPoints,
    #[serde(skip)]
    debug_window_test: DebugWindowTestWindow,
    #[serde(skip)]
    double_click_event: Option<MouseClickEvent>,
    #[serde(skip)]
    middle_click_event: Option<MouseClickEvent>,
    #[serde(skip)]
    clipboard_event: Option<ClipboardCopyEvent>,
    #[serde(skip)]
    clipboard_copy_window: ClipboardPopup,
    #[serde(skip)]
    stored_ui_responses: PreviewerUiResponses,
    open_tabs: HashSet<String>,

    #[serde(skip)]
    pub options_window: WindowZColorPickerOptions,
}

impl ZColorPickerAppContext {
    pub fn default() -> Self {
        Self {
            z_color_picker: Rc::new(RefCell::new(ZColorPickerWrapper::default())),
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
            options_window: WindowZColorPickerOptions::new(Pos2::new(200.0, 200.0)),
        }
    }
}

const PANE_COLOR_PICKER: usize = 1;
const PANE_COLOR_PICKER_OPTIONS: usize = 2;
const PANE_COLOR_PICKER_PREVIEWER: usize = 3;
const PANE_CONSOLE: usize = 4;

#[derive(Serialize, Deserialize, Debug)]
pub struct Pane {
    id: usize,
    title: Option<String>,
    ctx: Rc<RefCell<ZColorPickerAppContext>>,
    log_buffer: Arc<Mutex<Vec<String>>>,
    scroll_to_bottom: bool, // to remove, LogPane variable
}

impl Pane {
    pub fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx.clone();
    }

    pub fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane {}", self.id))
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut mut_ctx = self.ctx.borrow_mut();
        let color_copy_format = mut_ctx.color_copy_format;

        if self.id == PANE_COLOR_PICKER {
            // ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::WHITE);
            ui.allocate_ui(ui.max_rect().size(), |ui| {
                let color_picker_response = color_picker.draw_ui(ui, &color_copy_format);
                *mut_ctx.z_color_picker.borrow_mut() = color_picker;
                color_picker_response
            });

            return egui_tiles::UiResponse::None;
        } else if self.id == PANE_COLOR_PICKER_OPTIONS {
            let mut options = color_picker.options.clone();
            let mut options_window = mut_ctx.options_window.clone();
            options_window.update();
            let mut color_copy_format = color_copy_format;

            let options_draw_results = options_window.draw_content(
                ui,
                &mut options,
                &mut color_picker.control_points,
                &mut color_copy_format,
            );
            if let Some(preset_to_apply) = options_draw_results.preset_result.should_apply {
                color_picker
                    .apply_preset(&preset_to_apply)
                    .unwrap_or_else(|e| log::info!("{e}"))
            }
            mut_ctx.color_copy_format = color_copy_format;
            mut_ctx.options_window = options_window;
            color_picker.options = options;

            *mut_ctx.z_color_picker.borrow_mut() = color_picker;

            return egui_tiles::UiResponse::None;
        } else if self.id == PANE_COLOR_PICKER_PREVIEWER {
            let mut previewer = mut_ctx.previewer.clone();

            previewer.update(
                &color_picker.control_points,
                color_picker.options.spline_mode,
            );
            previewer.draw_ui(ui, ColorStringCopy::HEXNOA);

            mut_ctx.previewer = previewer;

            return egui_tiles::UiResponse::None;
        } else if self.id == PANE_CONSOLE {
            ui_log_window(ui, self.log_buffer.clone(), &mut self.scroll_to_bottom);

            return egui_tiles::UiResponse::None;
        }

        let color = egui::epaint::Hsva::new(0.103 * self.id as f32, 0.5, 0.5, 1.0);
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
        pane.title().into()
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

#[derive(Serialize, Deserialize, Debug)]
pub struct ZApp {
    monitor_size: Vec2,
    scale_factor: f32,
    native_pixel_per_point: f32,
    state: AppState,
    app_ctx: Rc<RefCell<ZColorPickerAppContext>>,
    tree: egui_tiles::Tree<Pane>,
    #[serde(skip)]
    log_buffer: Arc<Mutex<Vec<String>>>,
}

const HARDCODED_MONITOR_SIZE: Vec2 = Vec2::new(2560.0, 1440.0);
impl ZApp {
    // stupid work around since persistance storage does not work??
    pub fn request_init(&mut self) {
        self.state = AppState::Startup;
    }

    pub fn new(cc: &CreationContext<'_>, log_buffer: Arc<Mutex<Vec<String>>>) -> Self {
        // Can not get window screen size from CreationContext
        let monitor_size = HARDCODED_MONITOR_SIZE;
        const RESOLUTION_REF: f32 = 1080.0;
        let scale_factor: f32 = monitor_size.x.min(monitor_size.y) / RESOLUTION_REF;

        let app_ctx = ZColorPickerAppContext::default();
        let app_ctx = Rc::new(RefCell::new(app_ctx));

        let native_pixel_per_point = cc.egui_ctx.native_pixels_per_point().unwrap_or(1.0);

        Self {
            monitor_size: monitor_size,
            scale_factor: scale_factor,
            native_pixel_per_point: native_pixel_per_point,
            state: AppState::Startup,
            tree: Self::create_tree(app_ctx.clone(), log_buffer.clone()),
            app_ctx: app_ctx,
            log_buffer: log_buffer,
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Fix startup not having correct references
        {
            self.log_buffer = LogCollector::init().expect("Failed to init logger");

            for tile in &mut self.tree.tiles.iter_mut() {
                match tile.1 {
                    Tile::Pane(p) => {
                        p.ctx = self.app_ctx.clone();
                        p.log_buffer = self.log_buffer.clone();
                    }
                    _ => {}
                }
            }
        }

        let visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        log::info!("pixels_per_point{:?}", ctx.pixels_per_point());
        log::info!("native_pixels_per_point{:?}", ctx.native_pixels_per_point());
        ctx.set_pixels_per_point(self.scale_factor); // Maybe mult native_pixels_per_point?
                                                     // ctx.set_debug_on_hover(true);

        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
    }

    fn draw_ui_post(&mut self, _ctx: &egui::Context, ui: &mut Ui) {
        self.update_and_draw_debug_windows(ui);
        let copy_window = &mut self.app_ctx.borrow_mut().clipboard_copy_window;
        copy_window.update();
        copy_window.draw_ui(ui);
    }

    fn create_tree(
        ctx: Rc<RefCell<ZColorPickerAppContext>>,
        log_buffer: Arc<Mutex<Vec<String>>>,
    ) -> egui_tiles::Tree<Pane> {
        let mut tiles = egui_tiles::Tiles::default();

        let mut tabs = vec![];

        let pane_color_picker = Pane {
            id: PANE_COLOR_PICKER,
            title: None,
            ctx: ctx.clone(),
            log_buffer: log_buffer.clone(),
            scroll_to_bottom: true,
        };
        let pane_options = Pane {
            id: PANE_COLOR_PICKER_OPTIONS,
            title: None,
            ctx: ctx.clone(),
            log_buffer: log_buffer.clone(),
            scroll_to_bottom: true,
        };
        let pane_previewer = Pane {
            id: PANE_COLOR_PICKER_PREVIEWER,
            title: None,
            ctx: ctx.clone(),
            log_buffer: log_buffer.clone(),
            scroll_to_bottom: true,
        };
        let pane_console = Pane {
            id: PANE_CONSOLE,
            title: None,
            ctx: ctx.clone(),
            log_buffer: log_buffer.clone(),
            scroll_to_bottom: true,
        };

        let tile_color_picker = tiles.insert_pane(pane_color_picker);
        let tile_options = tiles.insert_pane(pane_options);
        let tile_previewer = tiles.insert_pane(pane_previewer);
        let tile_console = tiles.insert_pane(pane_console);

        let vertical_tile = tiles.insert_vertical_tile(vec![tile_color_picker, tile_options]);
        let master_tile = tiles.insert_horizontal_tile(vec![vertical_tile, tile_previewer]);
        tabs.push(tiles.insert_vertical_tile(vec![master_tile, tile_console]));

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
        let app_ctx = &mut self.app_ctx.borrow_mut();
        if response.clicked_by(PointerButton::Middle) {
            match response.interact_pointer_pos() {
                Some(pos) => {
                    let mut found_rect = None;
                    for rect in app_ctx.stored_ui_responses.get_rects() {
                        if rect.contains(pos) {
                            found_rect = Some(rect.clone());
                            break;
                        }
                    }

                    let rect = found_rect.unwrap_or(Rect::from_min_size(pos, Vec2::new(1.0, 1.0)));
                    app_ctx.clipboard_event = Some(ClipboardCopyEvent {
                        frame_rect: rect,
                        frame_pixels: None,
                    });
                }
                None => {}
            }
        }

        false
    }

    fn handle_clipboardcopy_event(&mut self) -> bool {
        let app_ctx = &mut self.app_ctx.borrow_mut();
        if let Some(event) = app_ctx.clipboard_event.take() {
            app_ctx.clipboard_copy_window.open(event.frame_rect.min);

            // Copy to clipboard
            if let Some(frame_pixels) = event.frame_pixels {
                if frame_pixels.data.len() == 1 {
                    let color = Color32::from_rgb(
                        frame_pixels.data[0].val.0,
                        frame_pixels.data[0].val.1,
                        frame_pixels.data[0].val.2,
                    );
                    let _ = write_color_to_clipboard(color, app_ctx.color_copy_format);
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
                    log::info!("clipboard event could not be processed, colors len was 0");
                }
            } else {
                log::info!("clipboard event could not be processed, did not have any colors set");
            }

            return true;
        }

        false
    }

    fn request_shutdown(&mut self) {
        self.state = AppState::Exit;
    }

    fn update_and_draw_debug_windows(&mut self, ui: &mut Ui) {
        let mut app_ctx = self.app_ctx.borrow_mut();
        let color_picker_clone = if let Ok(a) = app_ctx.z_color_picker.try_borrow_mut() {
            Some(a.clone())
        } else {
            None
        };

        if let Some(color_picker) = color_picker_clone {
            app_ctx
                .debug_window_control_points
                .update(&color_picker.control_points);
            app_ctx.debug_window_control_points.draw_ui(ui);

            if color_picker.control_points.len() >= 2 {
                let src_color = color_picker.control_points.first().unwrap().val().hsv();
                let trg_color = color_picker.control_points.last().unwrap().val().hsv();

                app_ctx.debug_window_test.update(src_color, trg_color);
            }
        }

        app_ctx.debug_window_test.draw_ui(ui);
    }

    fn process_ctx_inputs(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut user_quit: bool = false;
        {
            let app_ctx = &mut self.app_ctx.borrow_mut();
            let _input_ctx = ctx.input(|r| {
                // Esc
                if r.key_down(egui::Key::Escape) {
                    user_quit = true;
                }

                // DoubleLeftClick
                app_ctx.double_click_event = None;
                if r.pointer.button_double_clicked(PointerButton::Primary) {
                    let mouse_pos = r.pointer.interact_pos().unwrap();
                    app_ctx.double_click_event = Some(MouseClickEvent { mouse_pos });
                    log::info!("double click @({},{})", mouse_pos.x, mouse_pos.y);
                }

                app_ctx.middle_click_event = None;
                if r.pointer.button_clicked(PointerButton::Middle) {
                    let mouse_pos: Pos2 = r.pointer.interact_pos().unwrap();
                    app_ctx.middle_click_event = Some(MouseClickEvent { mouse_pos });

                    log::info!("middle click @({},{})", mouse_pos.x, mouse_pos.y);
                }

                // Debug toggles
                app_ctx.double_click_event = None;
                if r.key_pressed(egui::Key::F12) {
                    if app_ctx.debug_window_control_points.is_open() {
                        app_ctx.debug_window_control_points.close();
                    } else {
                        app_ctx.debug_window_control_points.open();
                    }

                    log::info!(
                        "debug_control_points {}",
                        app_ctx.debug_window_control_points.is_open()
                    );
                }
                app_ctx.double_click_event = None;
                if r.key_pressed(egui::Key::F11) {
                    if app_ctx.debug_window_test.is_open() {
                        app_ctx.debug_window_test.close();
                    } else {
                        app_ctx.debug_window_test.open();
                    }
                    log::info!("debug_window {}", app_ctx.debug_window_test.is_open());
                }
            });

            if let Some(mouse_event) = &app_ctx.double_click_event {
                // let color_picker = app_ctx.z_color_picker.lock().unwrap();
            }
        }

        if user_quit {
            self.request_shutdown();
        }
    }
}

impl eframe::App for ZApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        log::info!("SAVING...");

        #[cfg(feature = "serde")]
        if let Ok(json) = serde_json::to_string(self) {
            log::info!("SAVED with state: {:?}", self.state);
            storage.set_string(eframe::APP_KEY, json);
        }
        log::info!("SAVED!");
    }

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
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            _ => {
                panic!("Not a valid state {:?}", self.state);
            }
        }

        // let screen_size_px = [ctx.used_size().x as u32, ctx.used_size().y as u32];
        // if let Some(event) = &mut self.app_ctx.clipboard_event {
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
