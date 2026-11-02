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

use crate::{
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::{
        ControlPoint, ControlPointStorage, ControlPointTangent, ControlPointValue,
    },
    image_processing::{u8u8u8_to_u8u8u8u8, u8u8u8u8_to_u8, FramePixelRead, Rgb},
    logger::LogCollector,
    preset::{PresetHandler, PresetNameError},
    ui_egui::{
        clipboard::{
            copy_popup_seconds, copy_status_text, crop_screenshot, write_pixels_to_clipboard,
            write_string_to_clipboard, ClipboardCopyEvent, ClipboardPopup, ScreenshotTarget,
        },
        color_picker::{format_color_as, ZColorPickerWrapper},
        content_windows::{WindowPreset, WindowZColorPickerOptions},
        debug_windows::{DebugWindowControlPoints, DebugWindowTestWindow},
        panes::{
            ColorPickerOptionsPane, ColorPickerPane, LogPane, Pane, PreviewerPane, TreeBehavior,
            ZAppPane,
        },
        previewer::{PreviewerUiResponses, ZPreviewer},
        ui_common::ContentWindow,
    },
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppDataCtx {
    pub spline_mode: SplineMode,
    pub control_points: Vec<ControlPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZColorPickerOptions {
    pub is_curve_locked: bool,
    pub is_hue_middle_interpolated: bool,
    pub is_insert_right: bool,
    pub is_window_lock: bool,
    pub is_show_curvature_comb: bool,
    pub is_curvature_friendly: bool,
}

impl Default for ZColorPickerOptions {
    fn default() -> Self {
        Self {
            is_curve_locked: false,
            is_hue_middle_interpolated: true,
            is_insert_right: true,
            is_window_lock: true,
            is_show_curvature_comb: true,
            is_curvature_friendly: true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ZColorPickerAppContext {
    pub control_points: Vec<ControlPoint>,
    pub spline_mode: SplineMode,

    pub preset_handler: PresetHandler,

    pub z_color_picker: Rc<RefCell<ZColorPickerWrapper>>,
    pub previewer: ZPreviewer,
    pub color_copy_format: ColorStringCopy,
    #[serde(skip)]
    debug_window_control_points: DebugWindowControlPoints,
    #[serde(skip)]
    debug_window_test: DebugWindowTestWindow,
    #[serde(skip)]
    pending_screenshot_target: Option<ScreenshotTarget>,
    #[serde(skip)]
    clipboard_event: Option<ClipboardCopyEvent>,
    #[serde(skip)]
    clipboard_copy_window: ClipboardPopup,
    #[serde(skip)]
    pub stored_ui_responses: PreviewerUiResponses,
    open_tabs: HashSet<String>,

    #[serde(skip)]
    pub options_window: WindowZColorPickerOptions,
    #[serde(skip)]
    pub presets_window: WindowPreset,
}

const LAZY_TANGENT_DELTA: f32 = 0.01;
impl ZColorPickerAppContext {
    const DEFAULT_STARTUP_CONTROL_POINTS: [ControlPoint; 4] = [
        ControlPoint::ControlPointSimple(ControlPointStorage {
            val: ControlPointValue {
                val: [0.25, 0.33, 0.0],
            },
            t: 0.0,
            tangents: [
                Some(ControlPointTangent {
                    val: [-LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
                Some(ControlPointTangent {
                    val: [LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
            ],
        }),
        ControlPoint::ControlPointSimple(ControlPointStorage {
            val: ControlPointValue {
                val: [0.44, 0.38, 0.1],
            },
            t: 0.33,
            tangents: [
                Some(ControlPointTangent {
                    val: [-LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
                Some(ControlPointTangent {
                    val: [LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
            ],
        }),
        ControlPoint::ControlPointSimple(ControlPointStorage {
            val: ControlPointValue {
                val: [0.8, 0.6, 0.1],
            },
            t: 0.66,
            tangents: [
                Some(ControlPointTangent {
                    val: [-LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
                Some(ControlPointTangent {
                    val: [LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
            ],
        }),
        ControlPoint::ControlPointSimple(ControlPointStorage {
            val: ControlPointValue {
                val: [0.9, 0.8, 0.2],
            },
            t: 1.0,
            tangents: [
                Some(ControlPointTangent {
                    val: [-LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
                Some(ControlPointTangent {
                    val: [LAZY_TANGENT_DELTA, 0.0, 0.0],
                }),
            ],
        }),
    ];

    pub fn default() -> Self {
        let z_color_picker_wrapper = ZColorPickerWrapper::default();
        let mut preset_handler = PresetHandler::default();
        preset_handler.init_presets();
        Self {
            control_points: Self::DEFAULT_STARTUP_CONTROL_POINTS.to_vec(),
            spline_mode: SplineMode::HermiteBezier,

            preset_handler,

            z_color_picker: Rc::new(RefCell::new(z_color_picker_wrapper)),
            previewer: ZPreviewer::default(),
            color_copy_format: ColorStringCopy::default(),
            debug_window_control_points: DebugWindowControlPoints::new(),
            debug_window_test: DebugWindowTestWindow::new(),
            pending_screenshot_target: None,
            clipboard_event: None,
            clipboard_copy_window: ClipboardPopup::new(
                false,
                Pos2::new(0.0, 0.0),
                Instant::now(),
                0.7,
            ),
            stored_ui_responses: PreviewerUiResponses::default(),
            open_tabs: HashSet::default(),
            options_window: WindowZColorPickerOptions::new(),
            presets_window: WindowPreset::new(),
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
    #[serde(skip)]
    had_keyboard_focus: bool,
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
            had_keyboard_focus: false,
        }
    }

    fn init(&mut self) {
        self.app_ctx.borrow_mut().preset_handler.init_presets();
    }

    fn startup(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.init();
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

    fn show_create_preset_window(&mut self, ctx: &egui::Context) {
        let shown_id = egui::Id::new("create_preset_window_shown");
        if !self.app_ctx.borrow().presets_window.new_preset_is_open {
            ctx.data_mut(|d| d.remove::<bool>(shown_id));
            return;
        }

        let (mut text, spline_mode, control_points, preset_handler, create_error) = {
            let ctx_ref = self.app_ctx.borrow();
            (
                ctx_ref.presets_window.new_preset_window_text.clone(),
                ctx_ref.spline_mode.clone(),
                ctx_ref.control_points.clone(),
                ctx_ref.preset_handler.clone(),
                ctx_ref.presets_window.create_error.clone(),
            )
        };
        let is_first_open_frame = !ctx.data(|d| d.get_temp::<bool>(shown_id)).unwrap_or(false);

        let mut is_open = true;
        let mut should_create = false;
        let mut text_changed = false;
        let mut close_requested = false;
        let mut window = egui::Window::new("Create Preset")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER);
        if is_first_open_frame {
            window = window.current_pos(ctx.screen_rect().center());
        }
        window.show(ctx, |ui| {
            close_requested = ui.input(|i| i.key_pressed(egui::Key::Escape));
            let text_response =
                ui.add(egui::TextEdit::singleline(&mut text).hint_text("Preset name"));
            if is_first_open_frame {
                text_response.request_focus();
            }
            text_changed = text_response.changed();

            let was_enter =
                text_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            let check = preset_handler.check_name(&text, None);
            if let Some(reason) = visible_name_error(&text, check) {
                ui.colored_label(ui.visuals().error_fg_color, reason);
            }
            if let Some(error) = &create_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            let create_clicked = ui
                .add_enabled(check.is_ok(), egui::Button::new("Create"))
                .clicked();
            should_create = check.is_ok() && (was_enter || create_clicked);
        });
        if is_first_open_frame {
            ctx.data_mut(|d| d.insert_temp(shown_id, true));
        }
        if close_requested {
            is_open = false;
        }

        // 3. Re-borrow mutably only at the very end to apply changes
        let mut ctx_write = self.app_ctx.borrow_mut();
        ctx_write.presets_window.new_preset_is_open = is_open;
        ctx_write.presets_window.new_preset_window_text = text;
        if text_changed {
            ctx_write.presets_window.create_error = None;
        }

        if should_create {
            let name = ctx_write.presets_window.new_preset_window_text.clone();
            match ctx_write
                .preset_handler
                .create_preset(&name, &control_points, &spline_mode)
            {
                Ok(()) => {
                    // Reset state
                    ctx_write.presets_window.new_preset_is_open = false;
                    ctx_write.presets_window.new_preset_window_text.clear();
                    ctx_write.presets_window.create_error = None;
                }
                Err(e) => {
                    log::error!("{e}");
                    ctx_write.presets_window.create_error = Some(e.to_string());
                }
            }
        }
    }

    fn draw_ui_tree(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Windows needs to be drawn before the tree
        self.show_create_preset_window(ctx);
        self.app_ctx.borrow_mut().stored_ui_responses = PreviewerUiResponses::default();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::left_to_right(egui::Align::Min), |mut ui| {
                let mut behavior = TreeBehavior {};
                self.tree.ui(&mut behavior, ui);

                // Copy to clipboard
                let middle_mouse_pressed =
                    ctx.input(|i| i.pointer.button_pressed(PointerButton::Middle));
                if middle_mouse_pressed {
                    let interact_pos = ctx.input(|i| i.pointer.interact_pos());
                    if let Some(pos) = interact_pos {
                        self.handle_middleclick_event(pos, ctx, frame);
                    }
                }
                // The screenshot arrives in a later frame, possibly after the button is released.
                self.handle_screenshot_event(ui);

                self.draw_ui_post(ctx, &mut ui);
            });

            // auto-save-preset
            if !ctx.input(|i| i.pointer.any_down()) {
                let mut app_ctx = self.app_ctx.borrow_mut();
                let app_ctx = &mut *app_ctx;
                match app_ctx
                    .preset_handler
                    .auto_save(&app_ctx.control_points, app_ctx.spline_mode)
                {
                    Ok(true) => app_ctx.presets_window.save_error = None,
                    Ok(false) => {}
                    Err(e) => {
                        let message = format!("Auto-save failed: {e}");
                        log::error!("{message}");
                        app_ctx.presets_window.save_error = Some(message);
                        ctx.request_repaint();
                    }
                }
            }
        });
    }

    fn handle_middleclick_event(
        &mut self,
        pointer_pos: Pos2,
        ctx: &egui::Context,
        _frame: &eframe::Frame,
    ) {
        begin_screenshot(&mut self.app_ctx.borrow_mut(), pointer_pos);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
    }

    fn handle_screenshot_event(&mut self, ui: &egui::Ui) {
        receive_screenshot(&mut self.app_ctx.borrow_mut(), ui);
    }

    fn handle_clipboardcopy_event(&mut self) -> bool {
        let app_ctx = &mut self.app_ctx.borrow_mut();
        if let Some(event) = app_ctx.clipboard_event.take() {
            let mut popup_seconds = None;

            // Copy to clipboard
            if let Some(frame_pixels) = event.frame_pixels {
                if frame_pixels.data.len() == 1 {
                    let color = Color32::from_rgb(
                        frame_pixels.data[0].val.0,
                        frame_pixels.data[0].val.1,
                        frame_pixels.data[0].val.2,
                    );
                    let text = format_color_as(color, app_ctx.color_copy_format, None);
                    let result = write_string_to_clipboard(text.clone());
                    if let Err(e) = &result {
                        log::error!("{e}");
                    }
                    app_ctx
                        .clipboard_copy_window
                        .set_text(&copy_status_text(&result, &format!("Copied {text}")));
                    popup_seconds = Some(copy_popup_seconds(&result));
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
                    let result = write_pixels_to_clipboard(data);
                    if let Err(e) = &result {
                        log::error!("{e}");
                    }
                    app_ctx
                        .clipboard_copy_window
                        .set_text(&copy_status_text(&result, "Copied img to clipboard"));
                    popup_seconds = Some(copy_popup_seconds(&result));
                } else {
                    log::info!("clipboard event could not be processed, colors len was 0");
                }
            } else {
                log::info!("clipboard event could not be processed, did not have any colors set");
            }

            if let Some(seconds) = popup_seconds {
                app_ctx
                    .clipboard_copy_window
                    .open(event.frame_rect.min, seconds);
            }

            return popup_seconds.is_some();
        }

        false
    }

    fn request_shutdown(&mut self) {
        self.state = AppState::Exit;
    }

    fn update_and_draw_debug_windows(&mut self, ui: &mut Ui) {
        // TODO: Should clean up this logic for borrowing
        let mut app_ctx = self.app_ctx.borrow_mut();
        let control_points = app_ctx.control_points.clone();

        app_ctx.debug_window_control_points.update(&control_points);
        app_ctx.debug_window_control_points.draw_ui(ui);

        if control_points.len() >= 2 {
            let src_color = control_points.first().unwrap().val().hsv();
            let trg_color = control_points.last().unwrap().val().hsv();

            app_ctx.debug_window_test.update(src_color, trg_color);
        }

        app_ctx.debug_window_test.draw_ui(ui);
    }

    fn is_escape_blocked(&self, ctx: &egui::Context) -> bool {
        let app_ctx = self.app_ctx.borrow();
        self.had_keyboard_focus
            || ctx.memory(|m| m.any_popup_open())
            || app_ctx.presets_window.new_preset_is_open
            || app_ctx.presets_window.is_modal_open()
    }

    fn process_ctx_inputs(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
        esc_blocked: bool,
    ) {
        let mut user_quit: bool = false;
        {
            let app_ctx = &mut self.app_ctx.borrow_mut();
            ctx.input(|r| {
                user_quit = should_quit_on_escape(&r.events, esc_blocked);

                // Debug toggles
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

fn should_quit_on_escape(events: &[egui::Event], esc_blocked: bool) -> bool {
    let esc_first_press = events.iter().any(|event| {
        matches!(
            event,
            egui::Event::Key {
                key: egui::Key::Escape,
                pressed: true,
                repeat: false,
                ..
            }
        )
    });
    esc_first_press && !esc_blocked
}

fn visible_name_error(text: &str, check: Result<(), PresetNameError>) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    check.err().map(|err| err.to_string())
}

fn screenshot_target(pointer_pos: Pos2, previewer_rects: &[Rect]) -> ScreenshotTarget {
    match previewer_rects
        .iter()
        .find(|rect| rect.contains(pointer_pos))
    {
        Some(&rect) => ScreenshotTarget::Region(rect),
        None => ScreenshotTarget::Pixel(pointer_pos),
    }
}

fn begin_screenshot(app_ctx: &mut ZColorPickerAppContext, pointer_pos: Pos2) {
    let target = screenshot_target(pointer_pos, &app_ctx.stored_ui_responses.get_rects());

    app_ctx.pending_screenshot_target = Some(target);
    app_ctx.clipboard_copy_window.close();
}

fn receive_screenshot(app_ctx: &mut ZColorPickerAppContext, ui: &egui::Ui) {
    let Some(target) = app_ctx.pending_screenshot_target else {
        return;
    };

    let rect_image = ui.input(|i| {
        for event in &i.raw.events {
            if let egui::Event::Screenshot { image, .. } = event {
                return Some(crop_screenshot(image, target, i.pixels_per_point()));
            }
        }
        None
    });

    if let Some(img) = rect_image {
        app_ctx.pending_screenshot_target = None;
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
        let frame_rect = match target {
            ScreenshotTarget::Pixel(p) => Rect::from_min_size(p, Vec2::splat(1.0)),
            ScreenshotTarget::Region(r) => r,
        };
        app_ctx.clipboard_event = Some(ClipboardCopyEvent {
            frame_rect,
            frame_pixels: Some(frame_pixels),
        });
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
                // egui drops focus and closes popups during the Esc frame, so read them first.
                let esc_blocked = self.is_escape_blocked(ctx);
                self.handle_clipboardcopy_event();
                self.draw_ui_tree(ctx, frame);
                self.process_ctx_inputs(ctx, frame, esc_blocked);
                self.had_keyboard_focus = ctx.wants_keyboard_input();
            }
            AppState::Exit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key_event(key: egui::Key, pressed: bool, repeat: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn escape_quits_only_on_first_press_when_not_blocked() {
        let first_press = [key_event(egui::Key::Escape, true, false)];
        let repeat = [key_event(egui::Key::Escape, true, true)];
        let release = [key_event(egui::Key::Escape, false, false)];
        let other_key = [key_event(egui::Key::Enter, true, false)];

        assert!(should_quit_on_escape(&first_press, false));
        assert!(!should_quit_on_escape(&first_press, true));
        assert!(!should_quit_on_escape(&repeat, false));
        assert!(!should_quit_on_escape(&release, false));
        assert!(!should_quit_on_escape(&other_key, false));
        assert!(!should_quit_on_escape(&[], false));
    }

    #[test]
    fn begin_screenshot_closes_popup_and_targets_pixel_outside_previewer() {
        let mut app_ctx = <ZColorPickerAppContext as Default>::default();
        app_ctx.clipboard_copy_window.open(Pos2::ZERO, 0.7);
        let pos = Pos2::new(10.0, 20.0);

        begin_screenshot(&mut app_ctx, pos);

        assert!(!app_ctx.clipboard_copy_window.open);
        assert!(matches!(
            app_ctx.pending_screenshot_target,
            Some(ScreenshotTarget::Pixel(p)) if p == pos
        ));
    }

    #[test]
    fn screenshot_target_is_the_previewer_rect_under_the_pointer() {
        let left = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0));
        let right = Rect::from_min_max(Pos2::new(20.0, 0.0), Pos2::new(30.0, 10.0));

        assert!(matches!(
            screenshot_target(Pos2::new(25.0, 5.0), &[left, right]),
            ScreenshotTarget::Region(r) if r == right
        ));
    }

    #[test]
    fn screenshot_target_outside_every_rect_is_the_pointer_pixel() {
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0));
        let pos = Pos2::new(15.0, 5.0);

        assert!(matches!(
            screenshot_target(pos, &[rect]),
            ScreenshotTarget::Pixel(p) if p == pos
        ));
        assert!(matches!(
            screenshot_target(pos, &[]),
            ScreenshotTarget::Pixel(p) if p == pos
        ));
    }

    #[test]
    fn receive_screenshot_stores_copy_in_a_pass_that_repaints() {
        let mut app_ctx = <ZColorPickerAppContext as Default>::default();
        app_ctx.pending_screenshot_target = Some(ScreenshotTarget::Pixel(Pos2::new(1.0, 1.0)));
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_| {});
            });
        }
        let input = egui::RawInput {
            events: vec![egui::Event::Screenshot {
                viewport_id: egui::ViewportId::ROOT,
                user_data: Default::default(),
                image: Arc::new(egui::ColorImage::new([4, 4], Color32::RED)),
            }],
            ..Default::default()
        };

        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| receive_screenshot(&mut app_ctx, ui));
        });

        assert!(app_ctx.clipboard_event.is_some());
        assert!(app_ctx.pending_screenshot_target.is_none());
        assert_eq!(
            output.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            std::time::Duration::ZERO
        );
    }

    #[test]
    fn visible_name_error_hides_only_the_empty_text_error() {
        use crate::preset::validate_preset_name;

        assert_eq!(visible_name_error("", Err(PresetNameError::Empty)), None);
        assert_eq!(
            visible_name_error("a/b", validate_preset_name("a/b", [])),
            Some(PresetNameError::InvalidChar('/').to_string())
        );
        assert_eq!(
            visible_name_error("Warm", validate_preset_name("Warm", ["warm"])),
            Some(PresetNameError::Duplicate.to_string())
        );
        assert_eq!(
            visible_name_error("Warm", validate_preset_name("Warm", ["cold"])),
            None
        );
    }
}
