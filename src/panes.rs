use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::{app::ZColorPickerAppContext, common::ColorStringCopy, logger::ui_log_window};
pub struct TreeBehavior {}

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

#[derive(Serialize, Deserialize)]
pub enum Pane {
    ColorPicker(ColorPickerPane),
    ColorPickerOptionsPane(ColorPickerOptionsPane),
    Previewer(PreviewerPane),
    Log(LogPane),
}

impl ZAppPane for Pane {
    fn title(&self) -> String {
        match self {
            Pane::ColorPicker(pane) => pane.title().into(),
            Pane::ColorPickerOptionsPane(pane) => pane.title().into(),
            Pane::Previewer(pane) => pane.title().into(),
            Pane::Log(pane) => pane.title().into(),
        }
    }
    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        match self {
            Pane::ColorPicker(pane) => pane.update_ctx(new_ctx),
            Pane::ColorPickerOptionsPane(pane) => pane.update_ctx(new_ctx),
            Pane::Previewer(pane) => pane.update_ctx(new_ctx),
            Pane::Log(pane) => pane.update_ctx(new_ctx),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        match self {
            Pane::ColorPicker(pane) => pane.ui(ui),
            Pane::ColorPickerOptionsPane(pane) => pane.ui(ui),
            Pane::Previewer(pane) => pane.ui(ui),
            Pane::Log(pane) => pane.ui(ui),
        }
    }
}

pub trait ZAppPane {
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse;
    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>);
    fn title(&self) -> String {
        "Pane".to_string()
    }
    fn post_draw(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let color = egui::epaint::Hsva::new(0.103 as f32, 0.5, 0.5, 1.0);
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
#[derive(Serialize, Deserialize)]
pub struct ColorPickerPane {
    pub title: Option<String>,
    pub ctx: Rc<RefCell<ZColorPickerAppContext>>,
}

impl ZAppPane for ColorPickerPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut mut_ctx = self.ctx.borrow_mut();
        let color_copy_format = mut_ctx.color_copy_format;

        // ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::WHITE);
        ui.allocate_ui(ui.max_rect().size(), |ui| {
            let color_picker_response = color_picker.draw_ui(ui, &color_copy_format);
            *mut_ctx.z_color_picker.borrow_mut() = color_picker;
            color_picker_response
        });

        return egui_tiles::UiResponse::None;
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx.clone();
    }
}
#[derive(Serialize, Deserialize)]
pub struct ColorPickerOptionsPane {
    pub title: Option<String>,
    pub ctx: Rc<RefCell<ZColorPickerAppContext>>,
}
impl ZAppPane for ColorPickerOptionsPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut mut_ctx = self.ctx.borrow_mut();
        let color_copy_format = mut_ctx.color_copy_format;

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
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx.clone();
    }
}
#[derive(Serialize, Deserialize)]
pub struct PreviewerPane {
    pub title: Option<String>,
    pub ctx: Rc<RefCell<ZColorPickerAppContext>>,
}
impl ZAppPane for PreviewerPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut mut_ctx = self.ctx.borrow_mut();

        let mut previewer = mut_ctx.previewer.clone();

        previewer.update(
            &color_picker.control_points,
            color_picker.options.spline_mode,
        );
        let response = previewer.draw_ui(ui, ColorStringCopy::HEXNOA);

        mut_ctx.stored_ui_responses = response;
        mut_ctx.previewer = previewer;

        return egui_tiles::UiResponse::None;
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx.clone();
    }
}

#[derive(Serialize, Deserialize)]
pub struct LogPane {
    pub title: Option<String>,
    pub log_buffer: Arc<Mutex<Vec<String>>>,
    pub scroll_to_bottom: bool, // to remove, LogPane variable
}
impl ZAppPane for LogPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        ui_log_window(ui, self.log_buffer.clone(), &mut self.scroll_to_bottom);
        return egui_tiles::UiResponse::None;
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {}
}
