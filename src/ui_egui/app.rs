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
    export::{ExportSettings, UnrealLiveSettings},
    logger::{ui_log_header, ui_log_window, LogCollector},
    preset::{PresetHandler, PresetNameError},
    ui_egui::{
        clipboard::{
            clipboard_payload, copy_popup_seconds, copy_status_text, crop_screenshot,
            selected_color_payload, write_pixels_to_clipboard, write_string_to_clipboard,
            ClipboardCopyEvent, ClipboardPayload, ClipboardPopup, ScreenshotTarget,
            FAILED_COPY_POPUP_SECONDS,
        },
        color_picker::ZColorPickerWrapper,
        content_windows::WindowPreset,
        control_points::normalize_t,
        debug_windows::DebugWindowControlPoints,
        panes::{
            copy_format_bar_ui, export_bar_ui, preset_bar_ui, ColorPickerPane, Pane,
            PreviewerPane, TileUi, View3dPane, ZAppPane,
        },
        previewer::{PreviewerUiResponses, ZPreviewer},
        theme,
        ui_common::ContentWindow,
        view_3d::View3d,
    },
};
#[cfg(feature = "unreal-live")]
use crate::{
    export::export_name,
    ui_egui::panes::unreal_live_bar_ui,
    unreal_live::{asset_path, UnrealLiveLink},
};
use eframe::epaint::{Pos2, Vec2};
use egui_tiles::{Behavior, Container, Linear, LinearDir, Shares, Tile, TileId};

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
    #[serde(default)]
    pub is_lightness_middle_interpolated: bool,
    #[serde(default = "default_wants_clip_to_srgb")]
    pub wants_clip_to_srgb: bool,
}

fn default_curvature_comb_scale() -> f32 {
    1.0
}

fn default_wants_clip_to_srgb() -> bool {
    true
}

impl ZColorPickerOptions {
    pub fn is_middle_z_interpolated(&self, picker_space: PickerSpace) -> bool {
        if picker_space.is_z_hue() {
            self.is_hue_middle_interpolated
        } else {
            self.is_lightness_middle_interpolated
        }
    }
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
            is_lightness_middle_interpolated: false,
            wants_clip_to_srgb: default_wants_clip_to_srgb(),
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
    #[serde(default)]
    pub export: ExportSettings,
    #[serde(default)]
    pub unreal_live: UnrealLiveSettings,
    #[cfg(feature = "unreal-live")]
    #[serde(skip)]
    pub unreal_live_link: UnrealLiveLink,
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
            export: ExportSettings::default(),
            unreal_live: UnrealLiveSettings::default(),
            #[cfg(feature = "unreal-live")]
            unreal_live_link: UnrealLiveLink::default(),
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
            presets_window: WindowPreset::new(),
        }
    }

    fn export_selected_preset_on_save(&mut self) {
        let selected = self
            .preset_handler
            .preset_selected_index
            .and_then(|i| self.preset_handler.presets.get(i));
        if let Some(preset) = selected {
            self.export.export_saved_preset(preset);
        }
    }

    pub fn copy_to_clipboard(&mut self, payload: Option<ClipboardPayload>, anchor: Pos2) {
        let (status_text, popup_seconds) = write_payload_to_clipboard(payload);
        self.clipboard_copy_window.set_text(&status_text);
        self.clipboard_copy_window.open(anchor, popup_seconds);
    }

    #[cfg(feature = "unreal-live")]
    pub fn unreal_asset_path(&self) -> String {
        let selected_name = self
            .preset_handler
            .preset_selected_index
            .and_then(|i| self.preset_handler.presets.get(i))
            .map(|preset| preset.name.as_str());
        asset_path(&self.unreal_live.asset_folder, export_name(selected_name))
    }

    #[cfg(feature = "unreal-live")]
    fn sync_unreal_live(&mut self, egui_ctx: &egui::Context) {
        if !self.unreal_live.enabled {
            self.unreal_live_link.stop();
            return;
        }
        let data: AppDataCtx = (
            self.control_points.clone(),
            self.spline_mode,
            self.picker_space,
            self.blend_space,
        )
            .into();
        let asset_path = self.unreal_asset_path();
        let egui_ctx = egui_ctx.clone();
        self.unreal_live_link
            .sync(&self.unreal_live, asset_path, &data, move || {
                egui_ctx.request_repaint()
            });
    }
}

#[derive(Serialize, Deserialize)]
pub struct ZApp {
    state: AppState,
    app_ctx: Rc<RefCell<ZColorPickerAppContext>>,
    tree: egui_tiles::Tree<Pane>,
    #[serde(default)]
    is_log_expanded: bool,
    #[serde(default = "default_log_drawer_height")]
    log_drawer_height: f32,
    #[serde(default)]
    layout_version: u32,
    #[serde(skip)]
    log_buffer: Arc<Mutex<Vec<String>>>,
    #[serde(skip)]
    had_keyboard_focus: bool,
}

const LAYOUT_VERSION: u32 = 1;
const HARDCODED_MONITOR_SIZE: Vec2 = Vec2::new(2560.0, 1440.0);
const UI_SCALE_REFERENCE_HEIGHT: f32 = 1080.0;
const LOG_DRAWER_DEFAULT_HEIGHT: f32 = 200.0;
const LOG_DRAWER_MIN_HEIGHT: f32 = 80.0;

fn default_log_drawer_height() -> f32 {
    LOG_DRAWER_DEFAULT_HEIGHT
}

fn usable_monitor_size(monitor_size_px: Option<Vec2>) -> Option<Vec2> {
    monitor_size_px.filter(|size| size.is_finite() && size.min_elem() > 0.0)
}

fn ui_scale(monitor_size_px: Option<Vec2>) -> f32 {
    let size = usable_monitor_size(monitor_size_px).unwrap_or(HARDCODED_MONITOR_SIZE);
    size.min_elem() / UI_SCALE_REFERENCE_HEIGHT
}

fn ui_scale_log_line(scale: f32, monitor_size_px: Option<Vec2>, native_ppp: Option<f32>) -> String {
    let monitor = match usable_monitor_size(monitor_size_px) {
        Some(size) => format!("monitor {:.0}x{:.0} px", size.x, size.y),
        None => format!(
            "monitor size {monitor_size_px:?} unusable, fallback {}x{} px",
            HARDCODED_MONITOR_SIZE.x, HARDCODED_MONITOR_SIZE.y
        ),
    };
    let native = native_ppp.map_or_else(|| "unknown".to_owned(), |ppp| format!("{ppp:.2}"));
    format!("UI scale {scale:.2} from {monitor} (native pixels_per_point {native})")
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
            tree: Self::create_tree(app_ctx.clone()),
            app_ctx: app_ctx,
            is_log_expanded: false,
            log_drawer_height: LOG_DRAWER_DEFAULT_HEIGHT,
            layout_version: LAYOUT_VERSION,
            log_buffer: log_buffer,
            had_keyboard_focus: false,
        }
    }

    fn upgrade_layout(&mut self) {
        if self.layout_version >= LAYOUT_VERSION {
            return;
        }
        log::info!(
            "Replacing the pane layout of version {} with the default of version {LAYOUT_VERSION}",
            self.layout_version
        );
        self.tree = Self::create_tree(self.app_ctx.clone());
        self.layout_version = LAYOUT_VERSION;
    }

    fn init(&mut self) {
        let ctx = &mut *self.app_ctx.borrow_mut();
        ctx.preset_handler.init_presets();
        if normalize_t(&mut ctx.control_points) {
            log::warn!(
                "Restored control point T was not increasing from 0 to 1; spaced the points evenly"
            );
        }
        // The first frame's pass (tangents, auto comb) is no edit, so the hold has to see its result.
        ctx.z_color_picker.borrow_mut().pre_draw_update(
            &mut ctx.control_points,
            ctx.spline_mode,
            ctx.picker_space,
            ctx.blend_space,
        );
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
            self.upgrade_layout();

            for tile in &mut self.tree.tiles.iter_mut() {
                if let Tile::Pane(p) = tile.1 {
                    p.update_ctx(self.app_ctx.clone());
                }
            }
        }

        theme::apply(ctx);
        let monitor_size_px = ctx.input(|i| {
            i.viewport()
                .monitor_size
                .map(|size| size * i.pixels_per_point)
        });
        let scale = ui_scale(monitor_size_px);
        ctx.set_pixels_per_point(scale);
        log::info!(
            "{}",
            ui_scale_log_line(scale, monitor_size_px, ctx.native_pixels_per_point())
        );

        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
    }

    fn draw_ui_post(&mut self, ctx: &egui::Context, ui: &mut Ui) {
        self.update_and_draw_debug_windows(ui);
        let copy_window = &mut self.app_ctx.borrow_mut().clipboard_copy_window;
        copy_window.update();
        copy_window.draw(ctx);
    }

    fn create_tree(ctx: Rc<RefCell<ZColorPickerAppContext>>) -> egui_tiles::Tree<Pane> {
        let mut tiles = egui_tiles::Tiles::default();

        let pane_color_picker = ColorPickerPane {
            title: None,
            ctx: ctx.clone(),
        };
        let pane_previewer = PreviewerPane {
            title: None,
            ctx: ctx.clone(),
        };
        let pane_view_3d = View3dPane {
            title: Some("3D view".to_string()),
            ctx: ctx.clone(),
            view: View3d::default(),
        };

        let tile_color_picker = tiles.insert_pane(Pane::ColorPicker(pane_color_picker));
        let tile_view_3d = tiles.insert_pane(Pane::View3d(pane_view_3d));
        let tile_previewer = tiles.insert_pane(Pane::Previewer(pane_previewer));

        const PICKER_COLUMN_FRACTION: f32 = 0.3;
        const VIEW_3D_FRACTION: f32 = 0.55;
        let right_column_tile = tiles.insert_container(egui_tiles::Linear::new_binary(
            egui_tiles::LinearDir::Vertical,
            [tile_view_3d, tile_previewer],
            VIEW_3D_FRACTION,
        ));
        let root = tiles.insert_container(egui_tiles::Linear::new_binary(
            egui_tiles::LinearDir::Horizontal,
            [tile_color_picker, right_column_tile],
            PICKER_COLUMN_FRACTION,
        ));
        egui_tiles::Tree::new("z_color_picker_panes", root, tiles)
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
                    ctx_write.export_selected_preset_on_save();
                }
                Err(e) => {
                    log::error!("{e}");
                    ctx_write.presets_window.create_error = Some(e.to_string());
                }
            }
        }
    }

    fn draw_toolbar(&mut self, ctx: &egui::Context) {
        let toolbar = egui::TopBottomPanel::top("toolbar").frame(theme::toolbar_frame());
        toolbar.show(ctx, |ui| {
            ui.horizontal(|ui| {
                preset_bar_ui(&self.app_ctx, ui);
                theme::group_separator(ui);
                copy_format_bar_ui(&self.app_ctx, ui);
                theme::group_separator(ui);
                export_bar_ui(&self.app_ctx, ui);
                #[cfg(feature = "unreal-live")]
                {
                    theme::group_separator(ui);
                    unreal_live_bar_ui(&self.app_ctx, ui);
                }
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    let hint = if self.is_log_expanded {
                        "Collapse the log"
                    } else {
                        "Expand the log"
                    };
                    let log_button = egui::Button::new("Log").selected(self.is_log_expanded);
                    if ui.add(log_button).on_hover_text(hint).clicked() {
                        self.is_log_expanded = !self.is_log_expanded;
                    }
                });
            });
        });
    }

    fn draw_log_drawer(&mut self, ctx: &egui::Context) {
        let is_expanded = self.is_log_expanded;
        let grab_id = egui::Id::new("log_drawer_grab");
        let picker_min_height = picker_tile_min_size(&self.app_ctx.borrow()).y;
        let max_height = log_drawer_max_height(ctx, picker_min_height);
        let shown_height = |stored: f32| stored.max(LOG_DRAWER_MIN_HEIGHT).min(max_height);
        let bottom = ctx.available_rect().bottom();
        // Read before the panel is laid out, so the drag has no frame of lag.
        let grab = ctx
            .read_response(grab_id)
            .filter(|grab| is_expanded && grab.dragged());
        let grab_offset_id = grab_id.with("grab_offset");
        let pressed = grab.as_ref().filter(|grab| grab.drag_started());
        if let Some(press) = pressed.and_then(|_| ctx.input(|i| i.pointer.press_origin())) {
            let top = bottom - shown_height(self.log_drawer_height);
            ctx.data_mut(|d| d.insert_temp(grab_offset_id, press.y - top));
        }
        if let Some(pointer) = grab.and_then(|grab| grab.interact_pointer_pos()) {
            let grab_offset = ctx.data(|d| d.get_temp::<f32>(grab_offset_id));
            let top = pointer.y - grab_offset.unwrap_or_default();
            self.log_drawer_height = (bottom - top).min(max_height).max(LOG_DRAWER_MIN_HEIGHT);
        }
        let panel = if is_expanded {
            let height = shown_height(self.log_drawer_height);
            egui::TopBottomPanel::bottom("log_drawer")
                .resizable(false)
                .exact_height(height)
        } else {
            egui::TopBottomPanel::bottom("log_drawer_collapsed")
        };
        let shown = panel.frame(theme::drawer_frame()).show(ctx, |ui| {
            if ui_log_header(ui, is_expanded, &self.log_buffer).clicked() {
                self.is_log_expanded = !is_expanded;
            }
            if is_expanded {
                ui_log_window(ui, self.log_buffer.clone());
            }
        });
        if is_expanded {
            log_drawer_grab(ctx, grab_id, shown.response.rect);
        }
    }

    fn draw_ui_tree(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // Windows needs to be drawn before the tree
        self.show_create_preset_window(ctx);
        self.draw_toolbar(ctx);
        self.draw_log_drawer(ctx);
        self.app_ctx.borrow_mut().stored_ui_responses = PreviewerUiResponses::default();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::left_to_right(egui::Align::Min), |mut ui| {
                self.draw_tree(ui);

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
                    Ok(true) => {
                        app_ctx.presets_window.save_error = None;
                        app_ctx.export_selected_preset_on_save();
                    }
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
        #[cfg(feature = "unreal-live")]
        self.app_ctx.borrow_mut().sync_unreal_live(ctx);
    }

    fn draw_tree(&mut self, ui: &mut Ui) {
        let min_size = picker_tile_min_size(&self.app_ctx.borrow());
        let floor = ShareFloor::of_picker(&self.tree, ui.available_rect_before_wrap(), min_size);
        let shares = floor.as_ref().and_then(|floor| {
            let linear = floor.linear_mut(&mut self.tree)?;
            let base = linear.shares.clone();
            floor.raise(&mut linear.shares);
            Some((base, linear.shares.clone()))
        });
        self.tree.ui(&mut TileUi, ui);
        // Unchanged shares go back to the user's split, so a small window doesn't widen the picker for good.
        if let (Some(floor), Some((base, raised))) = (floor, shares) {
            if let Some(linear) = floor.linear_mut(&mut self.tree) {
                if linear.shares == raised {
                    linear.shares = base;
                } else {
                    floor.raise(&mut linear.shares);
                }
            }
        }
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
        let clicked_request = self
            .app_ctx
            .borrow()
            .z_color_picker
            .borrow_mut()
            .copy_request
            .take();
        let shortcut_blocked =
            input_blocked || egui::text_selection::LabelSelectionState::load(ctx).has_selection();
        let copy_format = self.app_ctx.borrow().color_copy_format;
        let shortcut_request = ctx
            .input(|i| is_copy_shortcut(&i.events, shortcut_blocked))
            .then(|| {
                let anchor = ctx
                    .input(|i| i.pointer.hover_pos())
                    .unwrap_or(ctx.screen_rect().center());
                (anchor, copy_format)
            });
        let Some((anchor, format)) = clicked_request.or(shortcut_request) else {
            return;
        };

        let payload = {
            let app_ctx = self.app_ctx.borrow();
            let picker = app_ctx.z_color_picker.borrow();
            selected_color_payload(
                &app_ctx.control_points,
                picker.dragging_index.or(picker.last_modifying_point_index),
                app_ctx.picker_space,
                format,
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

fn picker_tile_min_size(app_ctx: &ZColorPickerAppContext) -> Vec2 {
    theme::pane_card_size(app_ctx.z_color_picker.borrow().min_size)
}

fn log_drawer_max_height(ctx: &egui::Context, tile_area_min_height: f32) -> f32 {
    let tile_area_frame = egui::Frame::central_panel(&ctx.style());
    let tile_area_margin = tile_area_frame.total_margin().sum().y;
    let header_only =
        ctx.style().spacing.interact_size.y + theme::drawer_frame().total_margin().sum().y;
    (ctx.available_rect().height() - tile_area_margin - tile_area_min_height).max(header_only)
}

fn log_drawer_grab(ctx: &egui::Context, grab_id: egui::Id, drawer: Rect) {
    let radius = ctx.style().interaction.resize_grab_radius_side;
    let down_to_header = f32::from(theme::drawer_frame().inner_margin.top).min(radius);
    let rect = Rect::from_x_y_ranges(
        drawer.x_range(),
        drawer.top() - radius..=drawer.top() + down_to_header,
    );
    let ui = Ui::new(
        ctx.clone(),
        grab_id.with("ui"),
        egui::UiBuilder::new().max_rect(rect),
    );
    let grab = ui
        .interact(rect, grab_id, egui::Sense::drag())
        .on_hover_and_drag_cursor(egui::CursorIcon::ResizeVertical);
    if grab.hovered() || grab.dragged() {
        let stroke = ui.style().interact(&grab).fg_stroke;
        ui.painter()
            .hline(drawer.x_range(), drawer.top() + 0.5 * stroke.width, stroke);
    }
}

struct ShareFloor {
    container: TileId,
    tile: TileId,
    visible: Vec<TileId>,
    available: f32,
    min: f32,
}

impl ShareFloor {
    fn of_picker(tree: &egui_tiles::Tree<Pane>, tree_rect: Rect, min_size: Vec2) -> Option<Self> {
        let (&tile, _) = tree
            .tiles
            .iter()
            .find(|(_, tile)| matches!(tile, Tile::Pane(Pane::ColorPicker(_))))?;
        let container = tree.tiles.parent_of(tile)?;
        let rect = if tree.root() == Some(container) {
            tree_rect
        } else {
            tree.tiles.rect(container)?
        };
        let Some(Tile::Container(Container::Linear(linear))) = tree.tiles.get(container) else {
            return None;
        };
        let visible: Vec<TileId> = (linear.children.iter().copied())
            .filter(|&child| tree.tiles.is_visible(child))
            .collect();
        let (extent, min) = match linear.dir {
            LinearDir::Horizontal => (rect.width(), min_size.x),
            LinearDir::Vertical => (rect.height(), min_size.y),
        };
        let gaps = theme::PANE_GAP * visible.len().saturating_sub(1) as f32;
        Some(Self {
            container,
            tile,
            visible,
            available: extent - gaps,
            min,
        })
    }

    fn linear_mut<'t>(&self, tree: &'t mut egui_tiles::Tree<Pane>) -> Option<&'t mut Linear> {
        match tree.tiles.get_mut(self.container) {
            Some(Tile::Container(Container::Linear(linear))) => Some(linear),
            _ => None,
        }
    }

    fn raise(&self, shares: &mut Shares) {
        let others = self.visible.len() as f32 - 1.0;
        let others_min = others * Behavior::<Pane>::min_size(&TileUi);
        let target = self.min.min(self.available - others_min);
        let total: f32 = self.visible.iter().map(|&child| shares[child]).sum();
        if others < 1.0 || total <= 0.0 || self.available * shares[self.tile] / total >= target {
            return;
        }
        let others_share = total - shares[self.tile];
        shares[self.tile] = target * others_share / (self.available - target);
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
        ClipboardPayload::Text(text) => (
            write_string_to_clipboard(&text, &text),
            format!("Copied {text}"),
        ),
        ClipboardPayload::Summarized { text, summary } => (
            write_string_to_clipboard(&text, &summary),
            format!("Copied {summary}"),
        ),
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
    use crate::ui_egui::color_picker::MIN_SQUARE_SIDE;
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
        assert!(!options.is_lightness_middle_interpolated);
        assert!(options.wants_clip_to_srgb);
    }

    #[test]
    fn default_options_want_clip_to_srgb() {
        assert!(ZColorPickerOptions::default().wants_clip_to_srgb);
    }

    #[test]
    fn middle_z_follows_auto_hue_in_hue_spaces_and_auto_lightness_in_oklab() {
        let oklab = PickerSpace::Oklab {
            clip_to_srgb: false,
        };
        let cases = [
            (PickerSpace::Hsv, true, false, true),
            (PickerSpace::Hsv, false, true, false),
            (oklab, true, false, false),
            (oklab, false, true, true),
        ];
        for (space, auto_hue, auto_lightness, expected) in cases {
            let options = ZColorPickerOptions {
                is_hue_middle_interpolated: auto_hue,
                is_lightness_middle_interpolated: auto_lightness,
                ..Default::default()
            };
            assert_eq!(
                options.is_middle_z_interpolated(space),
                expected,
                "{space:?} auto hue {auto_hue}, auto lightness {auto_lightness}"
            );
        }
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
            tree: ZApp::create_tree(app_ctx.clone()),
            app_ctx,
            is_log_expanded: false,
            log_drawer_height: LOG_DRAWER_DEFAULT_HEIGHT,
            layout_version: LAYOUT_VERSION,
            log_buffer,
            had_keyboard_focus: false,
        }
    }

    fn draw_toolbar_and_tree(
        app: &mut ZApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        draw_on_screen(app, ctx, DEFAULT_SCREEN, events)
    }

    fn draw_on_screen(
        app: &mut ZApp,
        ctx: &egui::Context,
        screen: Rect,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            theme::apply(ctx);
            app.draw_toolbar(ctx);
            app.draw_log_drawer(ctx);
            egui::CentralPanel::default().show(ctx, |ui| app.draw_tree(ui));
        })
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

    #[test]
    fn default_layout_fits_the_picker_column_beside_the_3d_view_over_the_previewer() {
        const MIN_PICKER_PANE_WIDTH: f32 = 383.0;
        let mut app = test_app();
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);
        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());

        let picker = pane_rect(&app.tree, is_picker_pane).unwrap();
        let view_3d = pane_rect(&app.tree, |pane| matches!(pane, Pane::View3d(_))).unwrap();
        let previewer = pane_rect(&app.tree, |pane| matches!(pane, Pane::Previewer(_))).unwrap();
        let last_row = text_centers(&output, "COMB")[0];
        assert!(
            picker.width() >= MIN_PICKER_PANE_WIDTH,
            "picker pane {picker:?}"
        );
        assert!(picker.contains(last_row), "{last_row:?} in {picker:?}");
        assert!(
            previewer.width() >= 0.6 * DEFAULT_SCREEN.width(),
            "previewer pane {previewer:?}"
        );
        assert!(
            view_3d.bottom() <= previewer.top(),
            "{view_3d:?} {previewer:?}"
        );
        assert!(
            view_3d.height() > previewer.height(),
            "{view_3d:?} {previewer:?}"
        );
        assert_eq!(
            app.tree.tiles.iter().filter(|(_, t)| t.is_pane()).count(),
            3
        );
    }

    #[test]
    fn toolbar_groups_run_preset_copy_export_then_log_and_the_curve_row_is_in_the_picker() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);
        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());

        let toolbar_height = 2.0 * ctx.style().spacing.interact_size.y;
        let toolbar_x = |text| {
            let centers = text_centers(&output, text);
            assert!(!centers.is_empty(), "{text:?} not painted");
            assert!(centers[0].y < toolbar_height, "{text:?}");
            centers[0].x
        };
        let order = ["PRESET", "COPY AS", "EXPORT", "Log"];
        let xs = order.map(toolbar_x);
        assert!(
            xs.windows(2).all(|pair| pair[0] < pair[1]),
            "{order:?} at {xs:?}"
        );
        let picker = pane_rect(&app.tree, is_picker_pane).unwrap();
        for text in ["CURVE", "Spline", "Blend", "Flip"] {
            let centers = text_centers(&output, text);
            assert_eq!(centers.len(), 1, "{text:?} at {centers:?}");
            assert!(picker.contains(centers[0]), "{text:?} outside {picker:?}");
        }
    }

    #[test]
    fn toolbar_export_writes_the_current_gradient_once_a_folder_is_set() {
        let tmp = tempfile::TempDir::new().unwrap();
        let mut app = test_app();
        app.app_ctx.borrow_mut().control_points =
            ZColorPickerAppContext::DEFAULT_STARTUP_CONTROL_POINTS.to_vec();
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);
        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        let export_button = text_centers(&output, "Export")[0];

        click(&mut app, &ctx, export_button);
        assert!(app.app_ctx.borrow().export.last_result.is_none());

        app.app_ctx.borrow_mut().export.folder = tmp.path().to_str().unwrap().to_string();
        draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        click(&mut app, &ctx, export_button);

        let result = app.app_ctx.borrow().export.last_result.clone();
        assert!(matches!(result, Some(Ok(_))), "{result:?}");
        assert_eq!(
            sorted_file_names(tmp.path()),
            [
                "Gradient.ase",
                "Gradient.csv",
                "Gradient.grd",
                "Gradient.png"
            ]
        );
    }

    #[test]
    fn toolbar_copy_ue_array_without_points_shows_nothing_to_copy_without_a_folder() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);
        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        assert!(app.app_ctx.borrow().control_points.is_empty());
        assert!(app.app_ctx.borrow().export.folder().is_none());
        assert!(!app.app_ctx.borrow().clipboard_copy_window.open);

        click(&mut app, &ctx, text_centers(&output, "Copy UE array")[0]);

        let app_ctx = app.app_ctx.borrow();
        assert!(app_ctx.clipboard_copy_window.open);
        assert_eq!(app_ctx.clipboard_copy_window.text, "Nothing to copy");
        assert!(app_ctx.export.last_result.is_none());
    }

    fn sorted_file_names(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn app_with_selected_preset_exporting_on_save(
        presets: &std::path::Path,
        exports: &std::path::Path,
    ) -> ZApp {
        let app = test_app();
        {
            let mut ctx = app.app_ctx.borrow_mut();
            let points = ZColorPickerAppContext::DEFAULT_STARTUP_CONTROL_POINTS.to_vec();
            let mut handler = PresetHandler::new(presets.to_path_buf());
            handler
                .create_preset(
                    &"Warm".to_string(),
                    &points,
                    &ctx.spline_mode,
                    ctx.picker_space,
                    ctx.blend_space,
                )
                .unwrap();
            ctx.preset_handler = handler;
            ctx.control_points = points;
            ctx.export.on_save = true;
            ctx.export.folder = exports.to_str().unwrap().to_string();
        }
        app
    }

    #[test]
    fn toolbar_save_with_on_save_exports_the_written_preset() {
        let presets = tempfile::TempDir::new().unwrap();
        let exports = tempfile::TempDir::new().unwrap();
        let mut app = app_with_selected_preset_exporting_on_save(presets.path(), exports.path());
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);
        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        assert!(sorted_file_names(exports.path()).is_empty());

        click(&mut app, &ctx, text_centers(&output, "Save")[0]);

        assert_eq!(
            sorted_file_names(exports.path()),
            ["Warm.ase", "Warm.csv", "Warm.grd", "Warm.png"]
        );
    }

    #[test]
    fn export_selected_preset_on_save_writes_the_selected_preset_files() {
        let presets = tempfile::TempDir::new().unwrap();
        let exports = tempfile::TempDir::new().unwrap();
        let app = app_with_selected_preset_exporting_on_save(presets.path(), exports.path());

        app.app_ctx.borrow_mut().export_selected_preset_on_save();

        assert_eq!(
            sorted_file_names(exports.path()),
            ["Warm.ase", "Warm.csv", "Warm.grd", "Warm.png"]
        );
    }

    #[test]
    fn toolbar_panes_and_log_drawer_have_their_own_fills() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);
        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        let filled = |fill: Color32| -> Vec<Rect> {
            output
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Rect(rect) if rect.fill == fill => Some(rect.rect),
                    _ => None,
                })
                .collect()
        };

        let spans_screen = |rect: &Rect| rect.width() == DEFAULT_SCREEN.width();
        let toolbar = filled(theme::TOOLBAR_FILL);
        assert!(
            toolbar.iter().any(|r| spans_screen(r) && r.top() == 0.0),
            "{toolbar:?}"
        );
        let drawer = filled(theme::DRAWER_FILL);
        assert!(
            drawer
                .iter()
                .any(|r| spans_screen(r) && r.bottom() == DEFAULT_SCREEN.bottom()),
            "{drawer:?}"
        );
        let cards = filled(theme::PANE_FILL);
        for (id, tile) in app.tree.tiles.iter() {
            if tile.is_pane() {
                let tile_rect = app.tree.tiles.rect(*id).unwrap();
                assert!(cards.contains(&tile_rect), "{tile_rect:?} not in {cards:?}");
            }
        }
    }

    #[test]
    fn picker_flip_reverses_the_points_and_ends_a_drag() {
        let mut app = test_app();
        let values = [[0.1, 0.2, 0.3], [0.5, 0.5, 0.5], [0.9, 0.8, 0.7]];
        app.app_ctx.borrow_mut().control_points = values
            .iter()
            .zip([0.0, 0.4, 1.0])
            .map(|(&value, t)| ControlPoint::new_simple(value.into(), t))
            .collect();
        app.app_ctx
            .borrow()
            .z_color_picker
            .borrow_mut()
            .dragging_index = Some(0);
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);

        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        click(&mut app, &ctx, text_centers(&output, "Flip")[0]);

        let app_ctx = app.app_ctx.borrow();
        let sv = |value: [f32; 3]| [value[0], value[1]];
        let flipped: Vec<[f32; 2]> = app_ctx
            .control_points
            .iter()
            .map(|cp| sv(cp.val().val))
            .collect();
        let reversed: Vec<[f32; 2]> = values.iter().rev().map(|&value| sv(value)).collect();
        assert_eq!(flipped, reversed);
        assert_eq!(app_ctx.z_color_picker.borrow().dragging_index, None);
    }

    const NARROW_SCREEN: Rect = Rect {
        min: Pos2::ZERO,
        max: Pos2::new(800.0, 700.0),
    };

    fn laid_out_picker_tile(app: &mut ZApp, ctx: &egui::Context, screen: Rect) -> Rect {
        for _ in 0..3 {
            draw_on_screen(app, ctx, screen, Vec::new());
        }
        pane_rect(&app.tree, is_picker_pane).unwrap()
    }

    #[test]
    fn a_narrow_window_keeps_the_picker_tile_at_its_min_width() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let picker = laid_out_picker_tile(&mut app, &ctx, NARROW_SCREEN);

        let min_width = picker_tile_min_size(&app.app_ctx.borrow()).x;
        assert!(min_width > MIN_SQUARE_SIDE, "{min_width}");
        assert!(
            picker.width() >= min_width - 0.5,
            "{picker:?} < {min_width}"
        );
        assert!(picker.width() < 0.5 * NARROW_SCREEN.width(), "{picker:?}");
    }

    #[test]
    fn widening_the_window_again_restores_the_picker_tile_width() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let wide = laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);
        laid_out_picker_tile(&mut app, &ctx, NARROW_SCREEN);
        let widened = laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);

        assert!(
            (widened.width() - wide.width()).abs() < 0.5,
            "{widened:?} vs {wide:?}"
        );
    }

    fn stored_picker_tile_width(app: &ZApp) -> f32 {
        let root = app.tree.root().unwrap();
        let Some(Tile::Container(Container::Linear(linear))) = app.tree.tiles.get(root) else {
            panic!("root is not a linear container");
        };
        let tree_width = app.tree.tiles.rect(root).unwrap().width();
        let available = tree_width - theme::PANE_GAP * (linear.children.len() - 1) as f32;
        linear.shares.split(&linear.children, available)[0]
    }

    #[test]
    fn dragging_the_splitter_past_the_picker_min_width_stores_the_min_width() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let picker = laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);
        let splitter = Pos2::new(picker.right() + 0.5 * theme::PANE_GAP, picker.center().y);
        let target = Pos2::new(100.0, splitter.y);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(splitter)],
            vec![button(splitter, true)],
            vec![egui::Event::PointerMoved(target)],
            vec![egui::Event::PointerMoved(target)],
            vec![button(target, false)],
        ] {
            draw_on_screen(&mut app, &ctx, DEFAULT_SCREEN, events);
        }

        let min_width = picker_tile_min_size(&app.app_ctx.borrow()).x;
        let dragged = laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);
        assert!(
            (dragged.width() - min_width).abs() < 0.5,
            "{dragged:?} vs {min_width}"
        );
        let stored = stored_picker_tile_width(&app);
        assert!((stored - min_width).abs() < 0.5, "{stored} vs {min_width}");
    }

    fn round_trip(app: &ZApp) -> ZApp {
        serde_json::from_str(&serde_json::to_string(app).unwrap()).unwrap()
    }

    fn text_centers(output: &egui::FullOutput, text: &str) -> Vec<Pos2> {
        let mut centers: Vec<Pos2> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(shape) if shape.galley.text() == text => {
                    Some(shape.galley.rect.translate(shape.pos.to_vec2()).center())
                }
                _ => None,
            })
            .collect();
        centers.sort_by(|a, b| a.y.total_cmp(&b.y));
        centers
    }

    fn click(app: &mut ZApp, ctx: &egui::Context, pos: Pos2) {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        draw_toolbar_and_tree(app, ctx, vec![egui::Event::PointerMoved(pos), button(true)]);
        draw_toolbar_and_tree(app, ctx, vec![button(false)]);
    }

    fn laid_out_picker_height(app: &mut ZApp, ctx: &egui::Context) -> f32 {
        for _ in 0..3 {
            draw_toolbar_and_tree(app, ctx, Vec::new());
        }
        pane_rect(&app.tree, is_picker_pane).unwrap().height()
    }

    fn tree_bottom_gap(app: &ZApp) -> f32 {
        let previewer = pane_rect(&app.tree, |pane| matches!(pane, Pane::Previewer(_))).unwrap();
        DEFAULT_SCREEN.bottom() - previewer.bottom()
    }

    #[test]
    fn zapp_json_without_the_log_flag_starts_collapsed() {
        let mut json = serde_json::to_value(test_app()).unwrap();
        assert!(json
            .as_object_mut()
            .unwrap()
            .remove("is_log_expanded")
            .is_some());

        let restored: ZApp = serde_json::from_value(json).unwrap();

        assert!(!restored.is_log_expanded);
    }

    #[test]
    fn collapsed_log_drawer_is_one_row_and_expanding_it_shrinks_the_panes() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let row = ctx.style().spacing.interact_size.y;

        let collapsed_picker = laid_out_picker_height(&mut app, &ctx);
        let collapsed_gap = tree_bottom_gap(&app);
        app.is_log_expanded = true;
        let expanded_picker = laid_out_picker_height(&mut app, &ctx);
        let expanded_gap = tree_bottom_gap(&app);

        assert!(collapsed_gap < 3.0 * row, "{collapsed_gap}");
        assert!(
            collapsed_picker > expanded_picker,
            "{collapsed_picker} vs {expanded_picker}"
        );
        assert!(expanded_gap > 5.0 * row, "{expanded_gap}");
        assert!(
            expanded_gap < 0.25 * DEFAULT_SCREEN.height(),
            "{expanded_gap}"
        );
    }

    fn screen_of_height(height: f32) -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(DEFAULT_SCREEN.width(), height))
    }

    fn drawer_height(app: &ZApp, ctx: &egui::Context, screen: Rect) -> f32 {
        let tile_area = app.tree.tiles.rect(app.tree.root().unwrap()).unwrap();
        let central_margin = egui::Frame::central_panel(&ctx.style()).total_margin();
        screen.bottom() - tile_area.bottom() - central_margin.bottom
    }

    fn drag_drawer_top(app: &mut ZApp, ctx: &egui::Context, screen: Rect, to_y: f32) {
        let top_y = screen.bottom() - drawer_height(app, ctx, screen);
        drag_drawer_grab(app, ctx, screen, [top_y, to_y]);
    }

    fn drag_drawer_grab(
        app: &mut ZApp,
        ctx: &egui::Context,
        screen: Rect,
        [from_y, to_y]: [f32; 2],
    ) {
        let from = Pos2::new(0.5 * screen.width(), from_y);
        let target = Pos2::new(from.x, to_y);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        for events in [
            vec![egui::Event::PointerMoved(from)],
            vec![button(from, true)],
            vec![egui::Event::PointerMoved(target)],
            vec![egui::Event::PointerMoved(target)],
            vec![button(target, false)],
        ] {
            draw_on_screen(app, ctx, screen, events);
        }
    }

    fn expanded_app_laid_out(ctx: &egui::Context) -> ZApp {
        let mut app = test_app();
        app.is_log_expanded = true;
        laid_out_picker_tile(&mut app, ctx, DEFAULT_SCREEN);
        app
    }

    #[test]
    fn the_log_drawer_leaves_the_tiles_the_picker_min_height() {
        let ctx = egui::Context::default();
        let mut app = expanded_app_laid_out(&ctx);
        drag_drawer_top(&mut app, &ctx, DEFAULT_SCREEN, 0.0);

        for height in [1040.0, 900.0, 760.0, 1040.0] {
            let screen = screen_of_height(height);
            let picker = laid_out_picker_tile(&mut app, &ctx, screen);
            let min_height = picker_tile_min_size(&app.app_ctx.borrow()).y;
            assert!(
                (picker.height() - min_height).abs() < 0.5,
                "{height}: {picker:?} vs {min_height}"
            );
            assert!(drawer_height(&app, &ctx, screen) > LOG_DRAWER_MIN_HEIGHT);
        }
    }

    #[test]
    fn in_a_window_too_small_for_both_the_drawer_shrinks_to_its_header_first() {
        let ctx = egui::Context::default();
        let mut app = expanded_app_laid_out(&ctx);
        let min_height = picker_tile_min_size(&app.app_ctx.borrow()).y;
        let header =
            ctx.style().spacing.interact_size.y + theme::drawer_frame().total_margin().sum().y;
        let mut tile_area_at = |screen_height: f32| {
            let screen = screen_of_height(screen_height);
            let picker = laid_out_picker_tile(&mut app, &ctx, screen);
            (picker.height(), drawer_height(&app, &ctx, screen))
        };

        let (picker, drawer) = tile_area_at(DEFAULT_SCREEN.height());
        let height_leaving = |room: f32| DEFAULT_SCREEN.height() - picker - drawer + room;
        let (squeezed_picker, squeezed_drawer) = tile_area_at(height_leaving(min_height + 50.0));
        let (tiny_picker, tiny_drawer) = tile_area_at(height_leaving(min_height - 50.0));

        assert!(50.0 < LOG_DRAWER_MIN_HEIGHT && 50.0 > header);
        assert!((squeezed_drawer - 50.0).abs() < 0.5, "{squeezed_drawer}");
        assert!(
            (squeezed_picker - min_height).abs() < 0.5,
            "{squeezed_picker} vs {min_height}"
        );
        assert!(
            (tiny_drawer - header).abs() < 0.5,
            "{tiny_drawer} vs {header}"
        );
        assert!(
            (tiny_picker - (min_height - 50.0 - header)).abs() < 0.5,
            "{tiny_picker} vs {min_height}"
        );
    }

    #[test]
    fn a_short_window_caps_the_drawer_without_lowering_its_stored_height() {
        let ctx = egui::Context::default();
        let mut app = expanded_app_laid_out(&ctx);
        app.log_drawer_height = 300.0;
        laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);
        let tall = drawer_height(&app, &ctx, DEFAULT_SCREEN);

        let short = screen_of_height(760.0);
        laid_out_picker_tile(&mut app, &ctx, short);
        let capped = drawer_height(&app, &ctx, short);
        let stored_while_capped = app.log_drawer_height;
        laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);

        assert!((tall - 300.0).abs() < 1.0, "{tall}");
        assert!(capped < tall - 10.0, "{capped} vs {tall}");
        assert_eq!(stored_while_capped, 300.0);
        let restored = drawer_height(&app, &ctx, DEFAULT_SCREEN);
        assert!((restored - tall).abs() < 0.5, "{restored} vs {tall}");
        assert_eq!(round_trip(&app).log_drawer_height, 300.0);
    }

    #[test]
    fn dragging_the_drawer_top_sets_its_stored_height_within_the_cap() {
        let ctx = egui::Context::default();
        let mut app = expanded_app_laid_out(&ctx);
        let top = DEFAULT_SCREEN.bottom() - drawer_height(&app, &ctx, DEFAULT_SCREEN);
        let hover = vec![egui::Event::PointerMoved(Pos2::new(100.0, top))];
        let output = draw_on_screen(&mut app, &ctx, DEFAULT_SCREEN, hover);
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::ResizeVertical
        );

        let dragged_top = DEFAULT_SCREEN.bottom() - 300.0;
        drag_drawer_top(&mut app, &ctx, DEFAULT_SCREEN, dragged_top);
        assert!(
            (app.log_drawer_height - 300.0).abs() < 1.0,
            "{}",
            app.log_drawer_height
        );

        drag_drawer_top(&mut app, &ctx, DEFAULT_SCREEN, DEFAULT_SCREEN.bottom());
        assert_eq!(app.log_drawer_height, LOG_DRAWER_MIN_HEIGHT);

        drag_drawer_top(&mut app, &ctx, DEFAULT_SCREEN, 0.0);
        laid_out_picker_tile(&mut app, &ctx, DEFAULT_SCREEN);
        let capped = drawer_height(&app, &ctx, DEFAULT_SCREEN);
        assert!(
            (app.log_drawer_height - capped).abs() < 1.0,
            "{} vs {capped}",
            app.log_drawer_height
        );
    }

    #[test]
    fn a_click_on_the_top_edge_of_the_log_header_collapses_the_drawer() {
        let ctx = egui::Context::default();
        let mut app = expanded_app_laid_out(&ctx);
        let top = DEFAULT_SCREEN.bottom() - drawer_height(&app, &ctx, DEFAULT_SCREEN);
        let header_top = top + f32::from(theme::drawer_frame().inner_margin.top);
        click(&mut app, &ctx, Pos2::new(100.0, header_top + 0.5));
        assert!(!app.is_log_expanded);
    }

    #[test]
    fn a_drawer_drag_pressed_off_its_top_edge_moves_the_top_by_the_pointer_motion() {
        let ctx = egui::Context::default();
        for press_offset in [-4.0, 3.0] {
            let mut app = expanded_app_laid_out(&ctx);
            let height = drawer_height(&app, &ctx, DEFAULT_SCREEN);
            let press_y = DEFAULT_SCREEN.bottom() - height + press_offset;
            drag_drawer_grab(&mut app, &ctx, DEFAULT_SCREEN, [press_y, press_y - 40.0]);
            let dragged = app.log_drawer_height;
            assert!(
                (dragged - (height + 40.0)).abs() < 0.5,
                "{press_offset}: {height} -> {dragged}"
            );
        }
    }

    #[test]
    fn zapp_json_without_a_drawer_height_uses_the_default() {
        let mut json = serde_json::to_value(test_app()).unwrap();
        assert!(json
            .as_object_mut()
            .unwrap()
            .remove("log_drawer_height")
            .is_some());

        let restored: ZApp = serde_json::from_value(json).unwrap();

        assert_eq!(restored.log_drawer_height, LOG_DRAWER_DEFAULT_HEIGHT);
    }

    #[test]
    fn log_header_click_toggles_the_drawer() {
        let mut app = test_app();
        app.log_buffer
            .lock()
            .unwrap()
            .push("[WARN] last line".to_owned());
        let ctx = egui::Context::default();
        laid_out_picker_height(&mut app, &ctx);

        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        click(&mut app, &ctx, text_centers(&output, "[WARN] last line")[0]);
        assert!(app.is_log_expanded);

        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        let header_title = *text_centers(&output, "Log").last().unwrap();
        click(&mut app, &ctx, header_title);
        assert!(!app.is_log_expanded);
    }

    #[test]
    fn toolbar_log_toggle_and_drawer_header_share_the_flag() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let collapsed_picker = laid_out_picker_height(&mut app, &ctx);

        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        click(&mut app, &ctx, text_centers(&output, "Log")[0]);
        assert!(app.is_log_expanded);
        assert!(round_trip(&app).is_log_expanded);
        assert!(laid_out_picker_height(&mut app, &ctx) < collapsed_picker);

        let output = draw_toolbar_and_tree(&mut app, &ctx, Vec::new());
        let [_, header_title] = text_centers(&output, "Log")[..] else {
            panic!("expected the toolbar toggle and the drawer header");
        };
        click(&mut app, &ctx, header_title);
        assert!(!app.is_log_expanded);
        assert_eq!(laid_out_picker_height(&mut app, &ctx), collapsed_picker);
    }

    const OLD_LAYOUT_APP_JSON: &str = include_str!("testdata/old_layout_app.json");

    fn laid_out_pane_rects(app: &mut ZApp) -> [Option<Rect>; 3] {
        laid_out_picker_height(app, &egui::Context::default());
        [
            pane_rect(&app.tree, is_picker_pane),
            pane_rect(&app.tree, |pane| matches!(pane, Pane::View3d(_))),
            pane_rect(&app.tree, |pane| matches!(pane, Pane::Previewer(_))),
        ]
    }

    fn panes_share_the_app_context(app: &ZApp) -> bool {
        app.tree.tiles.iter().all(|(_, tile)| match tile {
            Tile::Pane(Pane::ColorPicker(pane)) => Rc::ptr_eq(&pane.ctx, &app.app_ctx),
            Tile::Pane(Pane::View3d(pane)) => Rc::ptr_eq(&pane.ctx, &app.app_ctx),
            Tile::Pane(Pane::Previewer(pane)) => Rc::ptr_eq(&pane.ctx, &app.app_ctx),
            Tile::Pane(_) => false,
            Tile::Container(_) => true,
        })
    }

    #[test]
    fn old_layout_state_gets_the_default_layout_and_keeps_its_points() {
        let mut restored: ZApp = serde_json::from_str(OLD_LAYOUT_APP_JSON).unwrap();
        assert_eq!(restored.layout_version, 0);
        let old_points = restored.app_ctx.borrow().control_points.clone();
        assert_eq!(old_points.len(), 2);

        restored.upgrade_layout();

        assert_eq!(restored.layout_version, LAYOUT_VERSION);
        assert_eq!(restored.app_ctx.borrow().control_points, old_points);
        assert!(panes_share_the_app_context(&restored));
        assert_eq!(
            laid_out_pane_rects(&mut restored),
            laid_out_pane_rects(&mut test_app())
        );
        assert_eq!(round_trip(&restored).layout_version, LAYOUT_VERSION);
    }

    #[test]
    fn current_layout_state_keeps_its_tree() {
        let mut app = test_app();
        let (&master_id, _) = app
            .tree
            .tiles
            .iter()
            .find(|(_, tile)| {
                matches!(tile, Tile::Container(egui_tiles::Container::Linear(linear))
                    if linear.dir == egui_tiles::LinearDir::Horizontal)
            })
            .unwrap();
        let Some(Tile::Container(egui_tiles::Container::Linear(master))) =
            app.tree.tiles.get_mut(master_id)
        else {
            unreachable!();
        };
        let picker_column = master.children[0];
        master.shares.set_share(picker_column, 0.5);
        let resized = laid_out_pane_rects(&mut app);

        let mut restored = round_trip(&app);
        restored.upgrade_layout();

        assert_eq!(laid_out_pane_rects(&mut restored), resized);
        assert_ne!(resized, laid_out_pane_rects(&mut test_app()));
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
    fn ui_scale_log_line_names_the_scale_being_set() {
        assert_eq!(
            ui_scale_log_line(2.0, Some(Vec2::new(3840.0, 2160.0)), Some(1.5)),
            "UI scale 2.00 from monitor 3840x2160 px (native pixels_per_point 1.50)"
        );
        assert_eq!(
            ui_scale_log_line(ui_scale(None), None, None),
            "UI scale 1.33 from monitor size None unusable, fallback 2560x1440 px \
             (native pixels_per_point unknown)"
        );
    }

    #[test]
    fn zapp_json_with_removed_scale_fields_still_loads() {
        let log_buffer = Arc::new(Mutex::new(Vec::new()));
        let app_ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        let app = ZApp {
            state: AppState::Idle,
            tree: ZApp::create_tree(app_ctx.clone()),
            app_ctx,
            is_log_expanded: false,
            log_drawer_height: LOG_DRAWER_DEFAULT_HEIGHT,
            layout_version: LAYOUT_VERSION,
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
            tree: ZApp::create_tree(app_ctx.clone()),
            app_ctx,
            is_log_expanded: false,
            log_drawer_height: LOG_DRAWER_DEFAULT_HEIGHT,
            layout_version: LAYOUT_VERSION,
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
    fn init_holds_a_selected_preset_that_auto_comb_rewrites_on_the_first_frame() {
        let tmp = tempfile::TempDir::new().unwrap();
        let uncombed: Vec<ControlPoint> = [([0.2, 0.2], 0.0), ([0.5, 0.7], 0.5), ([0.8, 0.3], 1.0)]
            .map(|([s, v], t)| {
                let mut cp = ControlPoint::new_simple([s, v, 0.3].into(), t);
                *cp.tangents_mut() = [Some([-0.1, 0.0, 0.0].into()), Some([0.1, 0.0, 0.0].into())];
                cp
            })
            .to_vec();
        let preset = crate::preset::PresetEntity::new(
            "Old",
            (
                uncombed.clone(),
                SplineMode::Bezier,
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
            control_points: uncombed.clone(),
            spline_mode: SplineMode::Bezier,
            preset_handler,
            ..<ZColorPickerAppContext as Default>::default()
        }));
        app_ctx
            .borrow()
            .z_color_picker
            .borrow_mut()
            .options
            .is_auto_comb = true;
        let mut app = ZApp {
            state: AppState::Startup,
            tree: ZApp::create_tree(app_ctx.clone()),
            app_ctx,
            is_log_expanded: false,
            log_drawer_height: LOG_DRAWER_DEFAULT_HEIGHT,
            layout_version: LAYOUT_VERSION,
            log_buffer: Arc::new(Mutex::new(Vec::new())),
            had_keyboard_focus: false,
        };

        app.init();
        let ctx = &mut *app.app_ctx.borrow_mut();
        let mut picker = ctx.z_color_picker.borrow_mut();
        picker.pre_draw_update(
            &mut ctx.control_points,
            ctx.spline_mode,
            ctx.picker_space,
            ctx.blend_space,
        );
        picker.apply_control_point_constraints(&mut ctx.control_points, ctx.picker_space);

        assert_ne!(ctx.control_points, uncombed);
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
            tree: ZApp::create_tree(app_ctx.clone()),
            app_ctx,
            is_log_expanded: false,
            log_drawer_height: LOG_DRAWER_DEFAULT_HEIGHT,
            layout_version: LAYOUT_VERSION,
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
