use arboard::ImageData;
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
    color_space::{BlendSpace, PickerSpace},
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::{
        ControlPoint, ControlPointStorage, ControlPointTangent, ControlPointValue,
    },
    logger::LogCollector,
    preset::{PresetHandler, PresetNameError},
    ui_egui::{
        clipboard::{
            clipboard_payload, copy_popup_seconds, copy_status_text, crop_screenshot,
            selected_color_payload, write_pixels_to_clipboard, write_string_to_clipboard,
            ClipboardCopyEvent, ClipboardPayload, ClipboardPopup, ScreenshotTarget,
            FAILED_COPY_POPUP_SECONDS,
        },
        color_picker::ZColorPickerWrapper,
        content_windows::{WindowPreset, WindowZColorPickerOptions},
        control_points::normalize_t,
        debug_windows::DebugWindowControlPoints,
        panes::{
            copy_format_bar_ui, preset_bar_ui, ColorPickerOptionsPane, ColorPickerPane, LogPane,
            Pane, PreviewerPane, TreeBehavior, ZAppPane,
        },
        previewer::{PreviewerUiResponses, ZPreviewer},
        ui_common::ContentWindow,
    },
};
use eframe::epaint::{Pos2, Vec2};
use egui_tiles::{Tile, TileId};

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
    #[serde(default)]
    pub picker_space: PickerSpace,
    #[serde(default)]
    pub blend_space: BlendSpace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZColorPickerOptions {
    pub is_curve_locked: bool,
    pub is_hue_middle_interpolated: bool,
    pub is_insert_right: bool,
    pub is_window_lock: bool,
    pub is_show_curvature_comb: bool,
    #[serde(default = "default_curvature_comb_scale")]
    pub curvature_comb_scale: f32,
    #[serde(default)]
    pub is_auto_comb: bool,
}

fn default_curvature_comb_scale() -> f32 {
    1.0
}

impl Default for ZColorPickerOptions {
    fn default() -> Self {
        Self {
            is_curve_locked: false,
            is_hue_middle_interpolated: true,
            is_insert_right: true,
            is_window_lock: true,
            is_show_curvature_comb: true,
            curvature_comb_scale: default_curvature_comb_scale(),
            is_auto_comb: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ZColorPickerAppContext {
    pub control_points: Vec<ControlPoint>,
    pub spline_mode: SplineMode,
    #[serde(default)]
    pub picker_space: PickerSpace,
    #[serde(default)]
    pub blend_space: BlendSpace,

    pub preset_handler: PresetHandler,

    pub z_color_picker: Rc<RefCell<ZColorPickerWrapper>>,
    pub previewer: ZPreviewer,
    pub color_copy_format: ColorStringCopy,
    #[serde(skip)]
    debug_window_control_points: DebugWindowControlPoints,
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
            picker_space: PickerSpace::default(),
            blend_space: BlendSpace::default(),

            preset_handler,

            z_color_picker: Rc::new(RefCell::new(z_color_picker_wrapper)),
            previewer: ZPreviewer::default(),
            color_copy_format: ColorStringCopy::default(),
            debug_window_control_points: DebugWindowControlPoints::new(),
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
    state: AppState,
    app_ctx: Rc<RefCell<ZColorPickerAppContext>>,
    tree: egui_tiles::Tree<Pane>,
    #[serde(skip)]
    log_buffer: Arc<Mutex<Vec<String>>>,
    #[serde(skip)]
    had_keyboard_focus: bool,
}

const HARDCODED_MONITOR_SIZE: Vec2 = Vec2::new(2560.0, 1440.0);
const UI_SCALE_REFERENCE_HEIGHT: f32 = 1080.0;

fn usable_monitor_size(monitor_size_px: Option<Vec2>) -> Option<Vec2> {
    monitor_size_px.filter(|size| size.is_finite() && size.min_elem() > 0.0)
}

fn ui_scale(monitor_size_px: Option<Vec2>) -> f32 {
    let size = usable_monitor_size(monitor_size_px).unwrap_or(HARDCODED_MONITOR_SIZE);
    size.min_elem() / UI_SCALE_REFERENCE_HEIGHT
}

impl ZApp {
    // stupid work around since persistance storage does not work??
    pub fn request_init(&mut self) {
        self.state = AppState::Startup;
    }

    pub fn new(log_buffer: Arc<Mutex<Vec<String>>>) -> Self {
        let app_ctx = ZColorPickerAppContext::default();
        let app_ctx = Rc::new(RefCell::new(app_ctx));

        Self {
            state: AppState::Startup,
            tree: Self::create_tree(app_ctx.clone(), log_buffer.clone()),
            app_ctx: app_ctx,
            log_buffer: log_buffer,
            had_keyboard_focus: false,
        }
    }

    fn init(&mut self) {
        let ctx = &mut *self.app_ctx.borrow_mut();
        ctx.preset_handler.init_presets();
        if normalize_t(&mut ctx.control_points) {
            log::warn!(
                "Restored control point T was not increasing from 0 to 1; spaced the points evenly"
            );
        }
        ctx.preset_handler.hold_auto_save_if_dirty(
            &ctx.control_points,
            ctx.spline_mode,
            ctx.picker_space,
            ctx.blend_space,
        );
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
        let monitor_size_px = ctx.input(|i| {
            i.viewport()
                .monitor_size
                .map(|size| size * i.pixels_per_point)
        });
        ctx.set_pixels_per_point(ui_scale(monitor_size_px));
        let monitor = match usable_monitor_size(monitor_size_px) {
            Some(size) => format!("monitor {:.0}x{:.0} px", size.x, size.y),
            None => format!(
                "monitor size {monitor_size_px:?} unusable, fallback {}x{} px",
                HARDCODED_MONITOR_SIZE.x, HARDCODED_MONITOR_SIZE.y
            ),
        };
        let native = ctx
            .native_pixels_per_point()
            .map_or_else(|| "unknown".to_owned(), |ppp| format!("{ppp:.2}"));
        log::info!(
            "UI scale {:.2} from {monitor} (native pixels_per_point {native})",
            ctx.pixels_per_point()
        );

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
        tiles.set_visible(tile_console, false);

        const PICKER_FRACTION: f32 = 0.85;
        const MASTER_FRACTION: f32 = 0.8;
        const PICKER_COLUMN_FRACTION: f32 = 0.3;
        let vertical_tile = tiles.insert_container(egui_tiles::Linear::new_binary(
            egui_tiles::LinearDir::Vertical,
            [tile_color_picker, tile_options],
            PICKER_FRACTION,
        ));
        let master_tile = tiles.insert_container(egui_tiles::Linear::new_binary(
            egui_tiles::LinearDir::Horizontal,
            [vertical_tile, tile_previewer],
            PICKER_COLUMN_FRACTION,
        ));
        tabs.push(tiles.insert_container(egui_tiles::Linear::new_binary(
            egui_tiles::LinearDir::Vertical,
            [master_tile, tile_console],
            MASTER_FRACTION,
        )));

        let root = tiles.insert_tab_tile(tabs);

        egui_tiles::Tree::new("my_tree", root, tiles)
    }

    fn show_create_preset_window(&mut self, ctx: &egui::Context) {
        let shown_id = egui::Id::new("create_preset_window_shown");
        if !self.app_ctx.borrow().presets_window.new_preset_is_open {
            ctx.data_mut(|d| d.remove::<bool>(shown_id));
            return;
        }

        let (
            mut text,
            spline_mode,
            picker_space,
            blend_space,
            control_points,
            preset_handler,
            create_error,
        ) = {
            let ctx_ref = self.app_ctx.borrow();
            (
                ctx_ref.presets_window.new_preset_window_text.clone(),
                ctx_ref.spline_mode.clone(),
                ctx_ref.picker_space,
                ctx_ref.blend_space,
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
            match ctx_write.preset_handler.create_preset(
                &name,
                &control_points,
                &spline_mode,
                picker_space,
                blend_space,
            ) {
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

    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        let log_id = log_tile_id(&self.tree);
        let mut is_log_visible = log_id.map(|id| self.tree.is_visible(id));
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                preset_bar_ui(&self.app_ctx, ui);
                ui.separator();
                copy_format_bar_ui(&self.app_ctx, ui);
                if let Some(is_visible) = &mut is_log_visible {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        let hint = if *is_visible {
                            "Hide the log"
                        } else {
                            "Show the log"
                        };
                        ui.toggle_value(is_visible, "Log").on_hover_text(hint);
                    });
                }
            });
        });
        if let (Some(id), Some(is_visible)) = (log_id, is_log_visible) {
            self.tree.set_visible(id, is_visible);
        }
    }

    fn draw_ui_tree(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Windows needs to be drawn before the tree
        self.show_create_preset_window(ctx);
        self.draw_toolbar(ctx);
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
                match app_ctx.preset_handler.auto_save(
                    &app_ctx.control_points,
                    app_ctx.spline_mode,
                    app_ctx.picker_space,
                    app_ctx.blend_space,
                ) {
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

    fn handle_clipboardcopy_event(&mut self) {
        let app_ctx = &mut self.app_ctx.borrow_mut();
        let Some(event) = app_ctx.clipboard_event.take() else {
            return;
        };

        let (status_text, popup_seconds) =
            write_payload_to_clipboard(clipboard_payload(&event.image, app_ctx.color_copy_format));

        app_ctx.clipboard_copy_window.set_text(&status_text);
        app_ctx
            .clipboard_copy_window
            .open(event.frame_rect.min, popup_seconds);
    }

    fn handle_selected_color_copy(&mut self, ctx: &egui::Context, input_blocked: bool) {
        let clicked_anchor = self
            .app_ctx
            .borrow()
            .z_color_picker
            .borrow_mut()
            .copy_request
            .take();
        let shortcut_blocked =
            input_blocked || egui::text_selection::LabelSelectionState::load(ctx).has_selection();
        let shortcut_anchor = ctx
            .input(|i| is_copy_shortcut(&i.events, shortcut_blocked))
            .then(|| {
                ctx.input(|i| i.pointer.hover_pos())
                    .unwrap_or(ctx.screen_rect().center())
            });
        let Some(anchor) = clicked_anchor.or(shortcut_anchor) else {
            return;
        };

        let payload = {
            let app_ctx = self.app_ctx.borrow();
            let picker = app_ctx.z_color_picker.borrow();
            selected_color_payload(
                &app_ctx.control_points,
                picker.dragging_index.or(picker.last_modifying_point_index),
                app_ctx.picker_space,
                app_ctx.color_copy_format,
            )
        };
        let Some(payload) = payload else {
            return;
        };

        let (status_text, popup_seconds) = write_payload_to_clipboard(Some(payload));
        let mut app_ctx = self.app_ctx.borrow_mut();
        app_ctx.clipboard_copy_window.set_text(&status_text);
        app_ctx.clipboard_copy_window.open(anchor, popup_seconds);
        ctx.request_repaint();
    }

    fn request_shutdown(&mut self) {
        self.state = AppState::Exit;
    }

    fn update_and_draw_debug_windows(&mut self, ui: &mut Ui) {
        let ctx = &mut *self.app_ctx.borrow_mut();
        if ctx.debug_window_control_points.is_open() {
            ctx.debug_window_control_points.update(&ctx.control_points);
        }
        ctx.debug_window_control_points.draw_ui(ui);
    }

    fn is_input_blocked(&self, ctx: &egui::Context) -> bool {
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
        input_blocked: bool,
    ) {
        let mut user_quit: bool = false;
        {
            let app_ctx = &mut self.app_ctx.borrow_mut();
            ctx.input(|r| {
                user_quit = should_quit_on_escape(&r.events, input_blocked);

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
            });
        }

        if user_quit {
            self.request_shutdown();
        }
    }
}

fn log_tile_id(tree: &egui_tiles::Tree<Pane>) -> Option<TileId> {
    tree.tiles
        .iter()
        .find(|(_, tile)| matches!(tile, Tile::Pane(Pane::Log(_))))
        .map(|(id, _)| *id)
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

fn is_copy_shortcut(events: &[egui::Event], blocked: bool) -> bool {
    !blocked
        && events
            .iter()
            .any(|event| matches!(event, egui::Event::Copy))
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

    if let Some(image) = rect_image {
        app_ctx.pending_screenshot_target = None;
        let frame_rect = match target {
            ScreenshotTarget::Pixel(p) => Rect::from_min_size(p, Vec2::splat(1.0)),
            ScreenshotTarget::Region(r) => r,
        };
        app_ctx.clipboard_event = Some(ClipboardCopyEvent { frame_rect, image });
    }
}

fn write_payload_to_clipboard(payload: Option<ClipboardPayload>) -> (String, f32) {
    let Some(payload) = payload else {
        let text = "Nothing to copy";
        log::warn!("{text}");
        return (text.to_string(), FAILED_COPY_POPUP_SECONDS);
    };
    let (result, success_text) = match payload {
        ClipboardPayload::Text(text) => {
            let success_text = format!("Copied {text}");
            (write_string_to_clipboard(text), success_text)
        }
        ClipboardPayload::Image {
            width,
            height,
            rgba,
        } => (
            write_pixels_to_clipboard(ImageData {
                width,
                height,
                bytes: Cow::Owned(rgba),
            }),
            "Copied img to clipboard".to_string(),
        ),
    };
    if let Err(e) = &result {
        log::error!("{e}");
    }
    (
        copy_status_text(&result, &success_text),
        copy_popup_seconds(&result),
    )
}

impl eframe::App for ZApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        match serde_json::to_string(self) {
            Ok(json) => {
                storage.set_string(eframe::APP_KEY, json);
                log::debug!("Saved app state");
            }
            Err(e) => log::error!("Could not save app state: {e}"),
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self.state {
            AppState::Startup => {
                self.startup(ctx, frame);
                self.state = AppState::Idle;
            }
            AppState::Idle => {
                // egui drops focus and closes popups during the Esc frame, so read them first.
                let input_blocked = self.is_input_blocked(ctx);
                self.handle_clipboardcopy_event();
                self.draw_ui_tree(ctx, frame);
                self.handle_selected_color_copy(ctx, input_blocked);
                self.process_ctx_inputs(ctx, frame, input_blocked);
                self.had_keyboard_focus = ctx.wants_keyboard_input();
            }
            AppState::Exit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ecolor::Color32;

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
    fn copy_shortcut_needs_a_copy_event_and_nothing_blocking_it() {
        assert!(is_copy_shortcut(&[egui::Event::Copy], false));
        assert!(!is_copy_shortcut(&[egui::Event::Copy], true));
        assert!(!is_copy_shortcut(
            &[key_event(egui::Key::C, true, false)],
            false
        ));
        assert!(!is_copy_shortcut(&[], false));
    }

    #[test]
    fn options_json_with_removed_field_still_loads() {
        let json = r#"{
            "is_curve_locked": true,
            "is_hue_middle_interpolated": false,
            "is_insert_right": false,
            "is_window_lock": false,
            "is_show_curvature_comb": false,
            "is_curvature_friendly": true
        }"#;

        let options: ZColorPickerOptions = serde_json::from_str(json).unwrap();

        assert!(options.is_curve_locked);
        assert!(!options.is_hue_middle_interpolated);
        assert!(!options.is_insert_right);
        assert!(!options.is_window_lock);
        assert!(!options.is_show_curvature_comb);
    }

    #[test]
    fn options_json_without_comb_scale_or_auto_comb_loads_their_defaults() {
        let json = r#"{
            "is_curve_locked": false,
            "is_hue_middle_interpolated": true,
            "is_insert_right": true,
            "is_window_lock": true,
            "is_show_curvature_comb": true
        }"#;

        let options: ZColorPickerOptions = serde_json::from_str(json).unwrap();

        assert_eq!(options.curvature_comb_scale, 1.0);
        assert!(!options.is_auto_comb);
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

    const DEFAULT_SCREEN: Rect = Rect {
        min: Pos2::ZERO,
        max: Pos2::new(1920.0, 1040.0),
    };

    fn test_app() -> ZApp {
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        let app_ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        ZApp {
            state: AppState::Idle,
            tree: ZApp::create_tree(app_ctx.clone(), log_buffer.clone()),
            app_ctx,
            log_buffer,
            had_keyboard_focus: false,
        }
    }

    fn draw_toolbar_and_tree(
        app: &mut ZApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(DEFAULT_SCREEN),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            app.draw_toolbar(ctx);
            egui::CentralPanel::default().show(ctx, |ui| app.tree.ui(&mut TreeBehavior {}, ui));
        })
    }

    fn lay_out(app: &mut ZApp) {
        let ctx = egui::Context::default();
        for _ in 0..3 {
            draw_toolbar_and_tree(app, &ctx, Vec::new());
        }
    }

    fn pane_rect(tree: &egui_tiles::Tree<Pane>, is_pane: fn(&Pane) -> bool) -> Option<Rect> {
        let (id, _) = tree
            .tiles
            .iter()
            .find(|(_, tile)| matches!(tile, Tile::Pane(pane) if is_pane(pane)))
            .unwrap();
        tree.tiles.rect(*id)
    }

    fn is_picker_pane(pane: &Pane) -> bool {
        matches!(pane, Pane::ColorPicker(_))
    }

    fn is_log_pane(pane: &Pane) -> bool {
        matches!(pane, Pane::Log(_))
    }

    #[test]
    fn default_layout_fits_the_picker_and_widens_the_previewer() {
        const MIN_PICKER_PANE_HEIGHT: f32 = 323.0;
        const MIN_PICKER_PANE_WIDTH: f32 = 383.0;
        const MIN_OPTIONS_PANE_HEIGHT: f32 = 70.0;
        let mut app = test_app();
        lay_out(&mut app);

        let picker = pane_rect(&app.tree, is_picker_pane).unwrap();
        let options = pane_rect(&app.tree, |pane| {
            matches!(pane, Pane::ColorPickerOptionsPane(_))
        })
        .unwrap()
        .height();
        let previewer = pane_rect(&app.tree, |pane| matches!(pane, Pane::Previewer(_)))
            .unwrap()
            .width();
        assert!(
            picker.height() >= MIN_PICKER_PANE_HEIGHT,
            "picker pane {picker:?}"
        );
        assert!(
            picker.width() >= MIN_PICKER_PANE_WIDTH,
            "picker pane {picker:?}"
        );
        assert!(
            previewer >= 0.6 * DEFAULT_SCREEN.width(),
            "previewer pane {previewer}"
        );
        assert!(options >= MIN_OPTIONS_PANE_HEIGHT, "options pane {options}");
    }

    #[test]
    fn default_tree_hides_the_log_and_gives_its_height_to_the_panes() {
        let mut app = test_app();
        let log_id = log_tile_id(&app.tree).unwrap();
        assert!(!app.tree.is_visible(log_id));
        lay_out(&mut app);
        let hidden_log_picker = pane_rect(&app.tree, is_picker_pane).unwrap().height();
        assert_eq!(pane_rect(&app.tree, is_log_pane), None);

        app.tree.set_visible(log_id, true);
        lay_out(&mut app);

        let picker = pane_rect(&app.tree, is_picker_pane).unwrap().height();
        let log = pane_rect(&app.tree, is_log_pane).unwrap().height();
        assert!(
            hidden_log_picker > picker,
            "{hidden_log_picker} vs {picker}"
        );
        assert!(log < 0.25 * DEFAULT_SCREEN.height(), "log pane {log}");
    }

    fn round_trip(app: &ZApp) -> ZApp {
        serde_json::from_str(&serde_json::to_string(app).unwrap()).unwrap()
    }

    fn is_log_visible(app: &ZApp) -> bool {
        app.tree.is_visible(log_tile_id(&app.tree).unwrap())
    }

    #[test]
    fn log_toggle_shows_the_log_and_survives_a_save_round_trip() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        assert!(!is_log_visible(&round_trip(&app)));

        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        let log_toggle = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == "Log" => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                }
                _ => None,
            })
            .unwrap();
        let button = |pressed| egui::Event::PointerButton {
            pos: log_toggle,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let press = vec![egui::Event::PointerMoved(log_toggle), button(true)];
        draw_toolbar_and_tree(&mut app, &ctx, press);
        draw_toolbar_and_tree(&mut app, &ctx, vec![button(false)]);

        assert!(is_log_visible(&app));
        assert!(is_log_visible(&round_trip(&app)));
    }

    #[test]
    fn zapp_json_with_a_visible_log_still_loads() {
        let app = test_app();
        let mut json = serde_json::to_value(&app).unwrap();
        let invisible = &mut json["tree"]["tiles"]["invisible"];
        assert_eq!(invisible.as_array().map(Vec::len), Some(1), "{invisible}");
        *invisible = serde_json::json!([]);

        let restored: ZApp = serde_json::from_value(json).unwrap();

        assert!(is_log_visible(&restored));
    }

    #[test]
    fn ui_scale_is_the_monitor_short_side_over_1080() {
        assert!((ui_scale(Some(Vec2::new(2560.0, 1440.0))) - 1.3333).abs() < 1e-4);
        assert!((ui_scale(Some(Vec2::new(1440.0, 2560.0))) - 1.3333).abs() < 1e-4);
        assert_eq!(ui_scale(Some(Vec2::new(3840.0, 2160.0))), 2.0);
        assert_eq!(ui_scale(Some(Vec2::new(1920.0, 1080.0))), 1.0);
    }

    #[test]
    fn ui_scale_without_a_usable_monitor_size_uses_the_hardcoded_monitor() {
        for size in [None, Some(Vec2::ZERO), Some(Vec2::new(f32::NAN, 1080.0))] {
            assert!((ui_scale(size) - 1.3333).abs() < 1e-4, "{size:?}");
        }
    }

    #[test]
    fn zapp_json_with_removed_scale_fields_still_loads() {
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        let app_ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        let app = ZApp {
            state: AppState::Idle,
            tree: ZApp::create_tree(app_ctx.clone(), log_buffer.clone()),
            app_ctx,
            log_buffer,
            had_keyboard_focus: false,
        };
        let mut json = serde_json::to_value(&app).unwrap();
        let fields = json.as_object_mut().unwrap();
        fields.insert(
            "monitor_size".to_owned(),
            serde_json::json!([2560.0, 1440.0]),
        );
        fields.insert("scale_factor".to_owned(), serde_json::json!(1.333));
        fields.insert("native_pixel_per_point".to_owned(), serde_json::json!(1.0));

        assert!(serde_json::from_value::<ZApp>(json).is_ok());
    }

    #[test]
    fn init_spaces_restored_old_t_evenly_without_dirtying_the_selected_preset() {
        let tmp = tempfile::TempDir::new().unwrap();
        let old_points: Vec<ControlPoint> = [0.1, 0.5, 0.9]
            .map(|sv| ControlPoint::new_simple([sv, sv, 0.2].into(), 0.0))
            .to_vec();
        let preset = crate::preset::PresetEntity::new(
            "Old",
            (
                old_points.clone(),
                SplineMode::Linear,
                PickerSpace::Hsv,
                BlendSpace::Picker,
            )
                .into(),
        );
        crate::preset::save_preset_to_disk(tmp.path(), &preset).unwrap();
        let preset_file = tmp.path().join("Old.json");
        let file_before = std::fs::read(&preset_file).unwrap();
        let mut preset_handler = PresetHandler::new(tmp.path().into());
        preset_handler.auto_save_presets = true;
        preset_handler.preset_selected_index = Some(0);
        let app_ctx = Rc::new(RefCell::new(ZColorPickerAppContext {
            control_points: old_points,
            spline_mode: SplineMode::Linear,
            preset_handler,
            ..<ZColorPickerAppContext as Default>::default()
        }));
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut app = ZApp {
            state: AppState::Startup,
            tree: ZApp::create_tree(app_ctx.clone(), log_buffer.clone()),
            app_ctx,
            log_buffer,
            had_keyboard_focus: false,
        };

        app.init();

        let ctx = &mut *app.app_ctx.borrow_mut();
        let ts: Vec<f32> = ctx.control_points.iter().map(|cp| *cp.t()).collect();
        assert_eq!(ts, [0.0, 0.5, 1.0]);
        assert!(!ctx.preset_handler.is_selected_dirty(
            &ctx.control_points,
            ctx.spline_mode,
            ctx.picker_space,
            ctx.blend_space
        ));
        assert!(!ctx.preset_handler.has_failed_auto_save());
        assert!(!ctx
            .preset_handler
            .auto_save(
                &ctx.control_points,
                ctx.spline_mode,
                ctx.picker_space,
                ctx.blend_space
            )
            .unwrap());
        assert_eq!(std::fs::read(&preset_file).unwrap(), file_before);
    }

    #[test]
    fn debug_control_points_window_tracks_live_points_only_while_open() {
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        let app_ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        let mut app = ZApp {
            state: AppState::Idle,
            tree: ZApp::create_tree(app_ctx.clone(), log_buffer.clone()),
            app_ctx,
            log_buffer,
            had_keyboard_focus: false,
        };
        let ctx = egui::Context::default();
        let draw_frame = |app: &mut ZApp| {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.update_and_draw_debug_windows(ui));
            });
        };
        app.app_ctx
            .borrow_mut()
            .control_points
            .push(ControlPoint::new_simple([0.5, 0.5, 0.2].into(), 0.0));

        draw_frame(&mut app);
        assert!(app
            .app_ctx
            .borrow()
            .debug_window_control_points
            .control_points
            .is_empty());

        app.app_ctx.borrow_mut().debug_window_control_points.open();
        draw_frame(&mut app);
        let app_ctx = app.app_ctx.borrow();
        assert_eq!(
            app_ctx.debug_window_control_points.control_points,
            app_ctx.control_points
        );
    }
}
