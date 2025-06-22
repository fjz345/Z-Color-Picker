use arboard::ImageData;
use ecolor::Color32;
use eframe::egui::{self, Layout, PointerButton, Rect, Ui};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};
#[cfg(windows)]
#[allow(unused_imports)]
use winapi::shared::winerror::ERROR_INCOMPATIBLE_SERVICE_SID_TYPE;

use crate::{
    clipboard::{
        write_color_to_clipboard, write_pixels_to_clipboard, ClipboardCopyEvent, ClipboardPopup,
    },
    color_picker::ZColorPickerWrapper,
    common::{ColorStringCopy, SplineMode},
    content_windows::WindowZColorPickerOptions,
    debug_windows::{DebugWindowControlPoints, DebugWindowTestWindow},
    image_processing::{u8u8u8_to_u8u8u8u8, u8u8u8u8_to_u8, FramePixelRead, Rgb},
    logger::LogCollector,
    panes::{
        ColorPickerOptionsPane, ColorPickerPane, LogPane, Pane, PreviewerPane, TreeBehavior,
        ZAppPane,
    },
    preset::Preset,
    previewer::{PreviewerUiResponses, ZPreviewer},
    ui_common::ContentWindow,
};
use eframe::{
    epaint::{Pos2, Vec2},
    CreationContext,
};
use egui_tiles::Tile;

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
    pub auto_save_presets: bool,
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
            auto_save_presets: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ZColorPickerAppContext {
    pub z_color_picker: Rc<RefCell<ZColorPickerWrapper>>,
    pub previewer: ZPreviewer,
    pub color_copy_format: ColorStringCopy,
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
    pub stored_ui_responses: PreviewerUiResponses,
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

#[derive(Serialize, Deserialize)]
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

    fn startup(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Fix startup not having correct references
        {
            self.log_buffer = LogCollector::init().expect("Failed to init logger");

            for tile in &mut self.tree.tiles.iter_mut() {
                match tile.1 {
                    Tile::Pane(p) => match p {
                        Pane::Log(log_pane) => log_pane.log_buffer = self.log_buffer.clone(),
                        _ => p.update_ctx(self.app_ctx.clone()),
                    },
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

    fn draw_ui_post(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        self.update_and_draw_debug_windows(ui);
        let copy_window = &mut self.app_ctx.borrow_mut().clipboard_copy_window;
        copy_window.update();
        copy_window.draw(ctx);
    }

    fn create_tree(
        ctx: Rc<RefCell<ZColorPickerAppContext>>,
        log_buffer: Arc<Mutex<Vec<String>>>,
    ) -> egui_tiles::Tree<Pane> {
        let mut tiles = egui_tiles::Tiles::default();

        let mut tabs = vec![];

        let pane_color_picker = ColorPickerPane {
            title: None,
            ctx: ctx.clone(),
        };
        let pane_options = ColorPickerOptionsPane {
            title: None,
            ctx: ctx.clone(),
        };
        let pane_previewer = PreviewerPane {
            title: None,
            ctx: ctx.clone(),
        };
        let pane_log = LogPane {
            title: Some("Log".to_string()),
            log_buffer: log_buffer.clone(),
            scroll_to_bottom: true,
        };

        let tile_color_picker = tiles.insert_pane(Pane::ColorPicker(pane_color_picker));
        let tile_options = tiles.insert_pane(Pane::ColorPickerOptionsPane(pane_options));
        let tile_previewer = tiles.insert_pane(Pane::Previewer(pane_previewer));
        let tile_console = tiles.insert_pane(Pane::Log(pane_log));

        let vertical_tile = tiles.insert_vertical_tile(vec![tile_color_picker, tile_options]);
        let master_tile = tiles.insert_horizontal_tile(vec![vertical_tile, tile_previewer]);
        tabs.push(tiles.insert_vertical_tile(vec![master_tile, tile_console]));

        let root = tiles.insert_tab_tile(tabs);

        egui_tiles::Tree::new("my_tree", root, tiles)
    }

    fn draw_ui_tree(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::left_to_right(egui::Align::Min), |mut ui| {
                let mut behavior = TreeBehavior {};
                self.tree.ui(&mut behavior, ui);

                // Copy to clipboard
                let middle_mouse_clicked = ctx.input(|i| i.pointer.middle_down());
                if middle_mouse_clicked {
                    let interact_pos = ctx.input(|i| i.pointer.interact_pos());
                    if let Some(pos) = interact_pos {
                        self.handle_middleclick_event(pos, ui, ctx, frame);
                    }
                }

                self.draw_ui_post(ctx, &mut ui);
            });

            // auto-save-preset
            {
                let app_ctx = self.app_ctx.borrow_mut();
                let color_picker = &mut app_ctx.z_color_picker.borrow_mut();
                if color_picker.options.auto_save_presets {
                    color_picker
                        .save_selected_preset()
                        .unwrap_or_else(|e| println!("{e}"));
                }
            }
        });
    }

    fn handle_middleclick_event(
        &mut self,
        pointer_pos: Pos2,
        ui: &egui::Ui,
        ctx: &egui::Context,
        _frame: &eframe::Frame,
    ) {
        let app_ctx = &mut self.app_ctx.borrow_mut();
        let mut found_rect = None;
        for rect in app_ctx.stored_ui_responses.get_rects() {
            if rect.contains(pointer_pos) {
                found_rect = Some(rect.clone());
                log::debug!("Found Rect");
                break;
            }
        }
        // found_rect = None;
        // Fallback rect if none found: 1x1 rect at pointer_pos
        let rect = found_rect.unwrap_or(Rect::from_min_size(
            pointer_pos.clamp(
                Pos2 { x: 0.0, y: 0.0 },
                ctx.screen_rect().max - Vec2 { x: 1.0, y: 1.0 },
            ),
            Vec2::new(1.0, 1.0),
        ));

        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        let rect_image = ui.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let pixels_per_point = i.pixels_per_point();
                    let region = rect;
                    let rect_image = image.region(&region, Some(pixels_per_point));

                    return Some(rect_image);
                }
            }
            None
        });

        if let Some(img) = rect_image {
            let rgb_vec = img
                .pixels
                .iter()
                .map(|f| Rgb {
                    val: (f.r(), f.g(), f.b()),
                })
                .collect();

            let frame_pixels = FramePixelRead {
                width: img.width(),
                height: img.height(),
                data: rgb_vec,
            };
            app_ctx.clipboard_event = Some(ClipboardCopyEvent {
                frame_rect: rect,
                frame_pixels: Some(frame_pixels),
            });
        }
    }

    fn handle_clipboardcopy_event(&mut self) -> bool {
        let app_ctx = &mut self.app_ctx.borrow_mut();
        if let Some(event) = app_ctx.clipboard_event.take() {
            let mut copied_to_clipboard = false;

            // Copy to clipboard
            if let Some(frame_pixels) = event.frame_pixels {
                if frame_pixels.data.len() == 1 {
                    let color = Color32::from_rgb(
                        frame_pixels.data[0].val.0,
                        frame_pixels.data[0].val.1,
                        frame_pixels.data[0].val.2,
                    );
                    let _ = write_color_to_clipboard(color, app_ctx.color_copy_format);
                    app_ctx
                        .clipboard_copy_window
                        .set_text(&format!("{:?}", color).to_string());
                    copied_to_clipboard = true;
                    log::debug!("Wrote {:?} to clipboard", color);
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
                    log::debug!(
                        "Writing pixels ({},{}) to clipboard",
                        &data.width,
                        &data.height
                    );
                    app_ctx
                        .clipboard_copy_window
                        .set_text(&"Copied img to clipboard".to_string());
                    copied_to_clipboard = true;
                    let _ = write_pixels_to_clipboard(data);
                } else {
                    log::info!("clipboard event could not be processed, colors len was 0");
                }
            } else {
                log::info!("clipboard event could not be processed, did not have any colors set");
            }

            if copied_to_clipboard {
                app_ctx.clipboard_copy_window.open(event.frame_rect.min);
            }

            return copied_to_clipboard;
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
            log::debug!("SAVED with state: {:?}", self.state);
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
