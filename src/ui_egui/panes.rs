use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::{
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::ControlPoint,
    logger::ui_log_window,
    preset::PresetHandler,
    ui_egui::{
        app::ZColorPickerAppContext, color_picker::ZColorPickerWrapper,
        content_windows::flip_control_points, previewer::PreviewerData,
    },
};
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
            Pane::ColorPickerOptionsPane(pane) => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| pane.ui(ui))
                    .inner
            }
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
        // TODO: Fix this borrowing stuff
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut mut_ctx = self.ctx.borrow_mut();
        let color_copy_format = mut_ctx.color_copy_format;
        let mut control_points = mut_ctx.control_points.clone();
        let spline_mode = mut_ctx.spline_mode;

        // ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::WHITE);
        ui.allocate_ui(ui.max_rect().size(), |ui| {
            let color_picker_response =
                color_picker.draw_ui(ui, &mut control_points, spline_mode, &color_copy_format);
            *mut_ctx.z_color_picker.borrow_mut() = color_picker;
            color_picker_response
        });

        mut_ctx.control_points = control_points;

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
        // TODO: FIX BORROWING HERE
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut preset_handler = self.ctx.borrow().preset_handler.clone();
        let mut mut_ctx = self.ctx.borrow_mut();
        let mut control_points = mut_ctx.control_points.clone();
        let mut spline_mode = mut_ctx.spline_mode;
        let color_copy_format = mut_ctx.color_copy_format;

        let mut options = color_picker.options.clone();
        let mut options_window = mut_ctx.options_window.clone();
        let mut presets_window = mut_ctx.presets_window.clone();
        options_window.update();
        presets_window.update();
        let mut color_copy_format = color_copy_format;

        let is_flip_clicked =
            options_window.draw_content(ui, &mut options, &mut spline_mode, &mut color_copy_format);
        if is_flip_clicked {
            flip_control_points(
                &mut control_points,
                &mut color_picker.last_modifying_point_index,
                &mut mut_ctx.previewer.data.points_preview_sizes,
            );
            color_picker.dragging_index = None;
        }
        let presets_result = presets_window.draw_content(
            ui,
            &mut preset_handler,
            &mut control_points,
            &mut spline_mode,
        );

        color_picker.options = options;
        if presets_result.preset_result.should_apply.is_some() {
            normalize_applied_preset(
                &mut color_picker,
                &mut preset_handler,
                &mut control_points,
                spline_mode,
                &mut mut_ctx.previewer.data,
            );
        }
        mut_ctx.color_copy_format = color_copy_format;
        mut_ctx.options_window = options_window;
        mut_ctx.presets_window = presets_window;
        mut_ctx.control_points = control_points;
        mut_ctx.spline_mode = spline_mode;
        mut_ctx.preset_handler = preset_handler;

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
        // TODO: FIX this borrowing
        let control_points = self.ctx.borrow().control_points.clone();
        let spline_mode = self.ctx.borrow().spline_mode;
        let mut mut_ctx = self.ctx.borrow_mut();

        let mut previewer = mut_ctx.previewer.clone();

        previewer.update(&control_points, spline_mode);
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
}
impl ZAppPane for LogPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        ui_log_window(ui, self.log_buffer.clone());
        return egui_tiles::UiResponse::None;
    }

    fn update_ctx(&mut self, _new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {}
}

fn normalize_applied_preset(
    color_picker: &mut ZColorPickerWrapper,
    preset_handler: &mut PresetHandler,
    control_points: &mut Vec<ControlPoint>,
    spline_mode: SplineMode,
    preview: &mut PreviewerData,
) {
    color_picker.pre_draw_update(control_points, spline_mode);
    // Twice: the hue lerp runs before the clamp, so one pass lerps from unclamped endpoint hues.
    color_picker.apply_control_point_constraints(control_points);
    color_picker.apply_control_point_constraints(control_points);
    preset_handler.set_selected_data_in_memory(control_points, spline_mode);
    color_picker.last_modifying_point_index = None;
    color_picker.dragging_index = None;
    preview.reset_preview_sizes();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;
    use std::fs;
    use tempfile::TempDir;

    const MODE: SplineMode = SplineMode::Bezier;

    fn unconstrained_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 1.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.2, 0.5, 0.9), 1.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.1), 2.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.4), 3.0),
        ]
    }

    fn next_picker_frame(color_picker: &mut ZColorPickerWrapper, points: &mut Vec<ControlPoint>) {
        color_picker.pre_draw_update(points, MODE);
        color_picker.apply_control_point_constraints(points);
    }

    struct SelectedPreset {
        tmp: TempDir,
        handler: PresetHandler,
        color_picker: ZColorPickerWrapper,
        points: Vec<ControlPoint>,
    }

    impl SelectedPreset {
        fn file_bytes(&self) -> Vec<u8> {
            fs::read(self.tmp.path().join("A.json")).unwrap()
        }
    }

    fn select_preset_with_auto_save() -> SelectedPreset {
        let tmp = TempDir::new().unwrap();
        let mut handler = PresetHandler::new(tmp.path().into());
        handler.auto_save_presets = true;
        handler
            .create_preset(&"A".to_string(), &unconstrained_points(), &MODE)
            .unwrap();
        handler.preset_selected_index = Some(0);
        let mut points = Vec::new();
        let mut spline_mode = SplineMode::Linear;
        handler.apply_selected_preset(&mut points, &mut spline_mode);
        assert_eq!(spline_mode, MODE);
        let mut color_picker = ZColorPickerWrapper::default();
        let mut preview = PreviewerData::new(points.len());
        normalize_applied_preset(
            &mut color_picker,
            &mut handler,
            &mut points,
            spline_mode,
            &mut preview,
        );
        SelectedPreset {
            tmp,
            handler,
            color_picker,
            points,
        }
    }

    #[test]
    fn selecting_preset_with_auto_save_does_not_rewrite_it() {
        let mut selected = select_preset_with_auto_save();
        assert!(!selected.handler.is_selected_dirty(&selected.points, MODE));
        let before = selected.file_bytes();

        next_picker_frame(&mut selected.color_picker, &mut selected.points);
        assert!(!selected.handler.auto_save(&selected.points, MODE).unwrap());
        assert!(!selected.handler.auto_save(&selected.points, MODE).unwrap());
        assert_eq!(selected.file_bytes(), before);
    }

    #[test]
    fn editing_selected_preset_with_auto_save_rewrites_it() {
        let mut selected = select_preset_with_auto_save();
        let before = selected.file_bytes();

        selected.points[1].val_mut()[0] = 0.25;
        next_picker_frame(&mut selected.color_picker, &mut selected.points);
        assert!(selected.handler.auto_save(&selected.points, MODE).unwrap());
        assert_ne!(selected.file_bytes(), before);
    }

    #[test]
    fn applying_a_preset_clears_the_selection_and_band_widths() {
        let mut selected = select_preset_with_auto_save();
        selected.color_picker.last_modifying_point_index = Some(2);
        selected.color_picker.dragging_index = Some(1);
        let mut preview = PreviewerData::new(4);
        preview.points_preview_sizes = vec![300.0, 50.0, 100.0, 100.0];

        normalize_applied_preset(
            &mut selected.color_picker,
            &mut selected.handler,
            &mut selected.points,
            MODE,
            &mut preview,
        );

        assert_eq!(selected.color_picker.last_modifying_point_index, None);
        assert_eq!(selected.color_picker.dragging_index, None);
        assert_eq!(
            preview.points_preview_sizes,
            PreviewerData::new(4).points_preview_sizes
        );
    }

    #[test]
    fn old_log_pane_json_still_loads() {
        let json = r#"{"title":"Log","log_buffer":[],"scroll_to_bottom":true}"#;
        assert!(serde_json::from_str::<LogPane>(json).is_ok());
    }
}
