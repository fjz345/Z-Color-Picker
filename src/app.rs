use arboard::ImageData;
use ecolor::Color32;
use eframe::egui::{
    color_picker::{color_picker_color32, Alpha},
    InnerResponse, Response, Slider, Ui, WidgetText,
};
use egui_dock::{
    egui::{self, Id, LayerId, Layout, PointerButton, Rect, TopBottomPanel, Window},
    DockArea, Node, NodeIndex, Style, TabViewer, Tree,
};
use std::{borrow::Cow, collections::HashSet, time::Instant};

use eframe::{
    epaint::{Pos2, Vec2},
    CreationContext,
};

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
    style: Option<Style>,
    z_color_picker: ZColorPickerWrapper,
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
            style: None,
            z_color_picker: ZColorPickerWrapper::default(),
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

    fn color_picker_menu(&mut self, ui: &mut Ui) {
        let style = self.style.as_mut().unwrap();

        let z_color_picker = ZColorPicker::new(&mut self.z_color_picker.control_points);
        ui.add(z_color_picker);
    }

    fn simple_demo_menu(&mut self, ui: &mut Ui) {
        ui.label("Egui widget example");
        ui.menu_button("Sub menu", |ui| {
            ui.label("hello :)");
        });
    }

    fn style_editor(&mut self, ui: &mut Ui) {
        ui.heading("Style Editor");

        let style = self.style.as_mut().unwrap();

        ui.collapsing("Border", |ui| {
            ui.separator();

            ui.label("Width");
            ui.add(Slider::new(&mut style.border_width, 1.0..=50.0));

            ui.separator();

            ui.label("Color");
            color_picker_color32(ui, &mut style.border_color, Alpha::OnlyBlend);
        });

        ui.collapsing("Selection", |ui| {
            ui.separator();

            ui.label("Color");
            color_picker_color32(ui, &mut style.selection_color, Alpha::OnlyBlend);
        });

        ui.collapsing("Separator", |ui| {
            ui.separator();

            ui.label("Width");
            ui.add(Slider::new(&mut style.separator_width, 1.0..=50.0));

            ui.label("Offset limit");
            ui.add(Slider::new(&mut style.separator_extra, 1.0..=300.0));

            ui.separator();

            ui.label("Idle color");
            // color_picker_color32(ui, &mut style.separator_color_idle, Alpha::OnlyBlend);

            ui.label("Hovered color");
            // color_picker_color32(ui, &mut style.separator_color_hovered, Alpha::OnlyBlend);

            ui.label("Dragged color");
            // color_picker_color32(ui, &mut style.separator_color_dragged, Alpha::OnlyBlend);
        });

        ui.collapsing("Tabs", |ui| {
            ui.separator();

            ui.checkbox(
                &mut style.tab_hover_name,
                "Show tab name when hoverd over them",
            );
            ui.checkbox(&mut style.tabs_are_draggable, "Tabs are draggable");
            ui.checkbox(&mut style.expand_tabs, "Expand tabs");
            ui.checkbox(&mut style.show_context_menu, "Show context_menu");
            ui.checkbox(
                &mut style.tab_include_scrollarea,
                "Include ScrollArea inside of tabs",
            );

            ui.separator();

            ui.label("Rounding");
            ui.horizontal(|ui| {
                ui.add(Slider::new(&mut style.tab_rounding.nw, 0.0..=15.0));
                ui.label("North-West");
            });
            ui.horizontal(|ui| {
                ui.add(Slider::new(&mut style.tab_rounding.ne, 0.0..=15.0));
                ui.label("North-East");
            });
            ui.horizontal(|ui| {
                ui.add(Slider::new(&mut style.tab_rounding.sw, 0.0..=15.0));
                ui.label("South-West");
            });
            ui.horizontal(|ui| {
                ui.add(Slider::new(&mut style.tab_rounding.se, 0.0..=15.0));
                ui.label("South-East");
            });

            ui.separator();

            ui.label("Title text color unfocused");
            // color_picker_color32(ui, &mut style.tab_text_color_unfocused, Alpha::OnlyBlend);

            ui.label("Title text color focused");
            // color_picker_color32(ui, &mut style.tab_text_color_focused, Alpha::OnlyBlend);

            ui.separator();

            ui.checkbox(&mut style.show_close_buttons, "Allow closing tabs");

            ui.separator();

            ui.label("Close button color unfocused");
            // color_picker_color32(ui, &mut style.close_tab_color, Alpha::OnlyBlend);

            ui.separator();

            ui.label("Close button color focused");
            // color_picker_color32(ui, &mut style.close_tab_active_color, Alpha::OnlyBlend);

            ui.separator();

            ui.label("Close button background color");
            // color_picker_color32(ui, &mut style.close_tab_background_color, Alpha::OnlyBlend);

            ui.separator();

            ui.label("Bar background color");
            // color_picker_color32(ui, &mut style.tab_bar_background_color, Alpha::OnlyBlend);

            ui.separator();

            ui.label("Outline color");
            // color_picker_color32(ui, &mut style.tab_outline_color, Alpha::OnlyBlend);

            ui.separator();

            ui.label("Background color");
            // color_picker_color32(ui, &mut style.tab_background_color, Alpha::OnlyBlend);
        });
    }
}

impl TabViewer for ZColorPickerAppContext {
    type Tab = String;

    fn ui(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Color Picker" => self.color_picker_menu(ui),
            "Style Editor" => self.style_editor(ui),
            _ => {
                ui.label(tab.as_str());
            }
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut Self::Tab) {
        match tab.as_str() {
            "Simple Demo" => self.simple_demo_menu(ui),
            _ => {
                ui.label(tab.to_string());
                ui.label("This is a context menu");
            }
        }
    }

    fn title(&mut self, tab: &mut Self::Tab) -> WidgetText {
        tab.as_str().into()
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> bool {
        self.open_tabs.remove(tab);
        true
    }
}

pub struct ZApp {
    monitor_size: Vec2,
    scale_factor: f32,
    state: AppState,
    z_color_picker_ctx: ZColorPickerAppContext,
    tree: Tree<String>,
}

impl ZApp {
    pub fn new(cc: &CreationContext<'_>) -> Self {
        let monitor_size = cc.integration_info.window_info.monitor_size.unwrap();
        const RESOLUTION_REF: f32 = 1080.0;
        let scale_factor: f32 = monitor_size.x.min(monitor_size.y) / RESOLUTION_REF;

        let z_color_picker_ctx = ZColorPickerAppContext::default();

        let mut tree = Tree::new(vec!["Color Picker".to_owned()]);
        let [a, b] = tree.split_right(NodeIndex::root(), 0.3, vec!["Style Editor".to_owned()]);
        let mut open_tabs = HashSet::new();
        for node in tree.iter() {
            if let Node::Leaf { tabs, .. } = node {
                for tab in tabs {
                    open_tabs.insert(tab.clone());
                }
            }
        }

        Self {
            monitor_size: monitor_size,
            scale_factor: scale_factor,
            state: AppState::Startup,
            z_color_picker_ctx,
            tree,
        }
    }

    fn startup(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let visuals: egui::Visuals = egui::Visuals::dark();
        ctx.set_visuals(visuals);
        ctx.set_pixels_per_point(self.scale_factor);
        frame.set_window_size(self.monitor_size);
        frame.set_visible(true);
        // frame.set_fullscreen(false);
        // frame.set_maximized(true);
    }

    fn draw_ui_tree(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::top("egui_dock::MenuBar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("View", |ui| {
                    // allow certain tabs to be toggled
                    for tab in &["Color Picker"] {
                        if ui
                            .selectable_label(
                                self.z_color_picker_ctx.open_tabs.contains(*tab),
                                *tab,
                            )
                            .clicked()
                        {
                            if let Some(index) = self.tree.find_tab(&tab.to_string()) {
                                self.tree.remove_tab(index);
                                self.z_color_picker_ctx.open_tabs.remove(*tab);
                            } else {
                                self.tree.push_to_focused_leaf(tab.to_string());
                            }

                            ui.close_menu();
                        }
                    }
                });
            })
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.with_layout(Layout::left_to_right(egui::Align::Min), |ui| {
                let layer_id = LayerId::background();
                let max_rect = ctx.available_rect();
                let clip_rect = ctx.available_rect();
                let id = Id::new("egui_dock::DockArea");
                let mut ui = Ui::new(ctx.clone(), layer_id, id, max_rect, clip_rect);

                let style = self
                    .z_color_picker_ctx
                    .style
                    .get_or_insert(Style::from_egui(&ui.ctx().style()))
                    .clone();
                DockArea::new(&mut self.tree)
                    .style(style)
                    .show_inside(&mut ui, &mut self.z_color_picker_ctx);

                // let color_picker_desired_size = Vec2 {
                //     x: ui.available_width() * 0.5,
                //     y: ui.available_height().min(ui.available_width()),
                // };
                // ui.spacing_mut().slider_width =
                //     color_picker_desired_size.x.min(color_picker_desired_size.y);

                // let left_side_reponse = ui.vertical(|ui| {
                //     let z_color_picker_response = self
                //         .z_color_picker_ctx
                //         .z_color_picker
                //         .draw_ui(ui, &mut self.z_color_picker_ctx.color_copy_format);

                //     z_color_picker_response
                // });

                // let z_color_picker_response_option = left_side_reponse.inner;

                // self.z_color_picker_ctx.previewer.update(
                //     &self.z_color_picker_ctx.z_color_picker.control_points,
                //     self.z_color_picker_ctx.z_color_picker.options.spline_mode,
                // );
                // self.z_color_picker_ctx.stored_ui_responses = self
                //     .z_color_picker_ctx
                //     .previewer
                //     .draw_ui(&mut ui, self.z_color_picker_ctx.color_copy_format);

                // self.handle_doubleclick_event(&z_color_picker_response_option);

                self.handle_middleclick_event(&mut ui);

                self.update_and_draw_debug_windows(&mut ui);
            });

            self.z_color_picker_ctx.clipboard_copy_window.update();
            self.z_color_picker_ctx.clipboard_copy_window.draw_ui(ui);
        });
    }

    fn handle_doubleclick_event(&mut self, z_color_picker_response: &Response) -> bool {
        match &self.z_color_picker_ctx.double_click_event {
            Some(pos) => {
                if z_color_picker_response.rect.contains(pos.mouse_pos) {
                    let z_color_picker_response_xy =
                        pos.mouse_pos - z_color_picker_response.rect.min;
                    let normalized_xy =
                        z_color_picker_response_xy / z_color_picker_response.rect.size();

                    let closest = self
                        .z_color_picker_ctx
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
                                self.z_color_picker_ctx
                                    .z_color_picker
                                    .spawn_control_point(cp.clone());
                            }
                        }
                        _ => {
                            let color: [f32; 3] = [color_xy[0], color_xy[1], 0.0];
                            let new_cp = ControlPoint::new_simple(color.into(), 0.0);
                            self.z_color_picker_ctx
                                .z_color_picker
                                .spawn_control_point(new_cp);
                        }
                    };
                    self.z_color_picker_ctx
                        .z_color_picker
                        .post_update_control_points();
                }
            }
            _ => {}
        }

        false
    }

    fn handle_middleclick_event(&mut self, _ui: &mut Ui) -> bool {
        if let Some(event) = &self.z_color_picker_ctx.middle_click_event {
            let mut found_rect = None;
            for rect in self.z_color_picker_ctx.stored_ui_responses.get_rects() {
                if rect.contains(event.mouse_pos) {
                    found_rect = Some(rect.clone());
                    break;
                }
            }

            let rect =
                found_rect.unwrap_or(Rect::from_min_size(event.mouse_pos, Vec2::new(1.0, 1.0)));
            self.z_color_picker_ctx.clipboard_event = Some(ClipboardCopyEvent {
                frame_rect: rect,
                frame_pixels: None,
            });
        }

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
        self.z_color_picker_ctx
            .debug_window_control_points
            .update(&self.z_color_picker_ctx.z_color_picker.control_points);
        self.z_color_picker_ctx
            .debug_window_control_points
            .draw_ui(ui);

        if self.z_color_picker_ctx.z_color_picker.control_points.len() >= 2 {
            let src_color = self
                .z_color_picker_ctx
                .z_color_picker
                .control_points
                .first()
                .unwrap()
                .val()
                .hsv();
            let trg_color = self
                .z_color_picker_ctx
                .z_color_picker
                .control_points
                .last()
                .unwrap()
                .val()
                .hsv();

            self.z_color_picker_ctx
                .debug_window_test
                .update(src_color, trg_color);
        }

        self.z_color_picker_ctx.debug_window_test.draw_ui(ui);
    }

    fn process_ctx_inputs(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let input_ctx = ctx.input();

        // Esc
        if input_ctx.key_down(egui::Key::Escape) {
            _frame.close();
        }

        // DoubleLeftClick
        self.z_color_picker_ctx.double_click_event = None;
        if input_ctx
            .pointer
            .button_double_clicked(PointerButton::Primary)
        {
            let mouse_pos = input_ctx.pointer.interact_pos().unwrap();
            self.z_color_picker_ctx.double_click_event = Some(MouseClickEvent { mouse_pos });
            println!("double click @({},{})", mouse_pos.x, mouse_pos.y);
        }

        self.z_color_picker_ctx.middle_click_event = None;
        if input_ctx.pointer.button_clicked(PointerButton::Middle) {
            let mouse_pos: Pos2 = input_ctx.pointer.interact_pos().unwrap();
            self.z_color_picker_ctx.middle_click_event = Some(MouseClickEvent { mouse_pos });

            println!("middle click @({},{})", mouse_pos.x, mouse_pos.y);
        }

        // Debug toggles
        self.z_color_picker_ctx.double_click_event = None;
        if input_ctx.key_pressed(egui::Key::F12) {
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
        if input_ctx.key_pressed(egui::Key::F11) {
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
                frame.close();
            }
            _ => {
                panic!("Not a valid state {:?}", self.state);
            }
        }
    }

    fn post_rendering(&mut self, screen_size_px: [u32; 2], frame: &eframe::Frame) {
        if let Some(event) = &mut self.z_color_picker_ctx.clipboard_event {
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
