use eframe::egui;
use eframe::egui::TextEdit;

use crate::common::ColorStringCopy;
use crate::common::SplineMode;
use crate::datatypes::control_point::ControlPoint;
use crate::egui::PointerButton;
use crate::egui::Ui;
use crate::preset::PresetEntity;
use crate::preset::PresetHandler;
use crate::ui_egui::app::ZColorPickerOptions;
use crate::ui_egui::ui_common::ContentWindow;

pub struct WindowPresetDrawResult {
    pub preset_result: PresetDrawResult,
}

impl Default for WindowPresetDrawResult {
    fn default() -> Self {
        Self {
            preset_result: Default::default(),
        }
    }
}

impl ContentWindow for WindowZColorPickerOptions {
    fn title(&self) -> &str {
        "ZColorPicker Options"
    }

    fn is_open(&self) -> bool {
        return self.open;
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn open(&mut self) {
        self.open = true;
    }
}

impl ContentWindow for WindowPreset {
    fn title(&self) -> &str {
        "Preset"
    }

    fn is_open(&self) -> bool {
        return self.open;
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn open(&mut self) {
        self.open = true;
    }
}

pub struct PresetDrawResult {
    pub should_apply: Option<PresetEntity>,
}

impl Default for PresetDrawResult {
    fn default() -> Self {
        Self {
            should_apply: Default::default(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct WindowZColorPickerOptions {
    pub open: bool,
}

impl WindowZColorPickerOptions {
    pub fn new() -> Self {
        Self { open: false }
    }

    pub fn update(&mut self) {}

    pub fn draw_content(
        &mut self,
        ui: &mut Ui,
        options: &mut ZColorPickerOptions,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
        color_copy_format: &mut ColorStringCopy,
    ) {
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut options.is_curve_locked, "Lock")
                .on_hover_text("Move all control points together");
            ui.checkbox(&mut options.is_hue_middle_interpolated, "Auto hue")
                .on_hover_text(
                    "Only the first and last hue are editable; middle hues are interpolated",
                );
            let insert_label = if options.is_insert_right {
                "Insert right"
            } else {
                "Insert left"
            };
            ui.checkbox(&mut options.is_insert_right, insert_label)
                .on_hover_text("Direction new points are inserted");
            ui.checkbox(&mut options.is_window_lock, "Clamp")
                .on_hover_text("Clamps the control points so they are contained");
            ui.checkbox(&mut options.is_show_curvature_comb, "Comb")
                .on_hover_text("Show curvature comb (Bezier mode)");
        });

        ui.horizontal(|ui| {
            ui.label("Copy as");
            egui::ComboBox::new(12312312, "")
                .selected_text(format!("{:?}", *color_copy_format))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    for format in [
                        ColorStringCopy::HEX,
                        ColorStringCopy::HEXNOA,
                        ColorStringCopy::SRGBHEX,
                        ColorStringCopy::HSV,
                        ColorStringCopy::HSVA,
                        ColorStringCopy::INT,
                        ColorStringCopy::FLOAT,
                        ColorStringCopy::RGB,
                        ColorStringCopy::SRGB,
                        ColorStringCopy::RGBA,
                        ColorStringCopy::SRGBA,
                    ] {
                        ui.selectable_value(color_copy_format, format, format!("{format:?}"));
                    }
                })
                .response
                .on_hover_text("Color Copy Format");

            ui.label("Spline");
            egui::ComboBox::new(12312313, "")
                .selected_text(spline_mode_label(*spline_mode))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    for mode in [
                        SplineMode::Linear,
                        SplineMode::Bezier,
                        SplineMode::HermiteBezier,
                    ] {
                        ui.selectable_value(spline_mode, mode, spline_mode_label(mode));
                    }
                })
                .response
                .on_hover_text("Spline Mode");

            if ui.button("Flip").clicked_by(PointerButton::Primary) {
                // Also Flip the tangets
                for cp in control_points.iter_mut() {
                    cp.flip_tangents();
                }

                control_points.reverse();
            }
        });
    }
}

#[derive(Clone, Debug, Default)]
pub struct WindowPreset {
    pub open: bool,
    pub new_preset_window_text: String,
    pub new_preset_is_open: bool,
    pub delete_confirm_open: bool,
    pub delete_error: Option<String>,
    pub create_error: Option<String>,
    pub rename_error: Option<String>,
    pub save_error: Option<String>,
    last_selection: Option<usize>,
}

fn spline_mode_label(mode: SplineMode) -> &'static str {
    match mode {
        SplineMode::Linear => "Linear",
        SplineMode::Bezier => "Bezier",
        SplineMode::HermiteBezier => "Hermite",
    }
}

fn selected_preset(preset_handler: &PresetHandler) -> Option<&PresetEntity> {
    preset_handler
        .preset_selected_index
        .and_then(|i| preset_handler.presets.get(i))
}

impl WindowPreset {
    pub fn new() -> Self {
        Self {
            open: false,
            new_preset_window_text: String::new(),
            new_preset_is_open: false,
            delete_confirm_open: false,
            delete_error: None,
            create_error: None,
            rename_error: None,
            save_error: None,
            last_selection: None,
        }
    }

    pub fn update(&mut self) {}

    fn save_selected(
        &mut self,
        preset_handler: &mut PresetHandler,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
    ) {
        let Some(preset) = preset_handler
            .preset_selected_index
            .and_then(|s| preset_handler.presets.get_mut(s))
        else {
            self.save_error = Some("No preset selected".to_string());
            return;
        };
        preset.data.spline_mode = spline_mode;
        preset.data.control_points = control_points.to_vec();
        let name = preset.name.clone();
        match preset_handler.save_selected_preset() {
            Ok(()) => {
                log::info!("Saved preset [{name}]");
                self.save_error = None;
            }
            Err(e) => {
                log::error!("{e}");
                self.save_error = Some(e.to_string());
            }
        }
    }

    pub fn draw_content(
        &mut self,
        ui: &mut Ui,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
    ) -> WindowPresetDrawResult {
        let mut draw_result = WindowPresetDrawResult::default();

        ui.horizontal(|ui| {
            let combobox_selected_text_to_show =
                selected_preset(preset_handler).map_or("<None>", |p| p.name.as_str());

            let mut combobox_selected_index = 0;
            let mut combobox_has_selected = false;
            let _combobox_response = egui::ComboBox::new(1232313, "")
                .selected_text(combobox_selected_text_to_show)
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);

                    for (i, preset) in &mut preset_handler.presets.iter().enumerate() {
                        let selectable_value_response = ui.selectable_value(
                            &mut combobox_selected_index,
                            i + 1,
                            preset.name.as_str(),
                        );

                        if selectable_value_response.clicked() {
                            combobox_has_selected = true;
                        }
                    }

                    // New
                    let selectable_new_response =
                        ui.selectable_value(&mut combobox_selected_index, 0, "<NEW>");
                    // None
                    let selectable_none_response =
                        ui.selectable_value(&mut combobox_selected_index, 0, "<None>");

                    if selectable_new_response.clicked() {
                        combobox_has_selected = true;
                    } else if selectable_none_response.clicked() {
                        combobox_has_selected = false;
                        preset_handler.preset_selected_index = None;
                    }
                })
                .response
                .on_hover_text("Presets");

            if combobox_has_selected {
                if combobox_selected_index == 0 {
                    self.new_preset_is_open = true;
                    self.new_preset_window_text.clear();
                    self.create_error = None;
                    log::info!("Selected New Preset");
                } else {
                    preset_handler.preset_selected_index = Some(combobox_selected_index - 1);
                    if let Some(s) = preset_handler.preset_selected_index {
                        draw_result.preset_result.should_apply =
                            Some(preset_handler.presets[s].clone());
                        log::info!("Selected Preset {:?}", combobox_selected_index - 1);
                        preset_handler.apply_selected_preset(control_points, spline_mode);
                    }
                }
            };

            ui.checkbox(&mut preset_handler.auto_save_presets, "Auto-save")
                .on_hover_text("Auto save preset");
            let has_selection = selected_preset(preset_handler).is_some();
            if ui
                .add_enabled(has_selection, egui::Button::new("Save"))
                .on_hover_text("Overwrite the selected preset with the current points")
                .on_disabled_hover_text("Select a preset first")
                .clicked_by(PointerButton::Primary)
            {
                self.save_selected(preset_handler, control_points, *spline_mode);
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .on_hover_text("Delete the selected preset from disk")
                .on_disabled_hover_text("Select a preset first")
                .clicked_by(PointerButton::Primary)
            {
                self.delete_confirm_open = true;
                self.delete_error = None;
            }
        });

        let rename_text_field_id = egui::Id::new(819181);
        let mut rename_text_field: String = ui.memory(|mem| {
            mem.data
                .get_temp::<String>(rename_text_field_id)
                .unwrap_or_default()
        });
        let selected = preset_handler.preset_selected_index;
        if selected != self.last_selection {
            self.rename_error = None;
            self.save_error = None;
            self.last_selection = selected;
        }
        if let Some(error) = &self.save_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let rename_check = selected.map(|s| preset_handler.check_name(&rename_text_field, Some(s)));
        let rename_reason = match rename_check {
            Some(Err(reason)) if !rename_text_field.is_empty() => Some(reason),
            _ => None,
        };

        ui.horizontal(|ui| {
            let rename_button = ui.add_enabled(
                matches!(rename_check, Some(Ok(()))),
                egui::Button::new("Rename"),
            );

            let mut was_text_box_enter = false;
            ui.push_id(rename_text_field_id, |ui| {
                let text_widget = TextEdit::singleline(&mut rename_text_field)
                    .hint_text("New name")
                    .desired_width(200.0);
                let text_response = ui.add(text_widget);
                if text_response.changed() {
                    self.rename_error = None;
                }
                if text_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    was_text_box_enter = true;
                }
            });

            if let (Some(s), Some(Ok(()))) = (selected, rename_check) {
                if was_text_box_enter || rename_button.clicked_by(PointerButton::Primary) {
                    match preset_handler.rename_preset(s, &rename_text_field) {
                        Ok(()) => {
                            rename_text_field.clear();
                            self.rename_error = None;
                        }
                        Err(e) => {
                            log::error!("{e}");
                            self.rename_error = Some(e.to_string());
                        }
                    }
                }
            }
        });

        if let Some(reason) = rename_reason {
            ui.colored_label(ui.visuals().error_fg_color, reason.to_string());
        }
        if let Some(error) = &self.rename_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.memory_mut(|mem| {
            *mem.data.get_temp_mut_or_default(rename_text_field_id) = rename_text_field
        });

        if self.delete_confirm_open {
            self.draw_delete_confirm_modal(ui, preset_handler);
        }

        draw_result
    }

    fn draw_delete_confirm_modal(&mut self, ui: &mut Ui, preset_handler: &mut PresetHandler) {
        let Some(name) = preset_handler
            .preset_selected_index
            .and_then(|s| preset_handler.presets.get(s))
            .map(|preset| preset.name.clone())
        else {
            self.close_delete_confirm();
            return;
        };

        let modal = egui::Modal::new(egui::Id::new("delete_preset_modal")).show(ui.ctx(), |ui| {
            ui.label(format!("Delete \"{name}\" from disk?"));
            if let Some(error) = &self.delete_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| (ui.button("Delete").clicked(), ui.button("Cancel").clicked()))
                .inner
        });
        let (delete_clicked, cancel_clicked) = modal.inner;

        if delete_clicked {
            match preset_handler.delete_selected_preset() {
                Ok(()) => self.close_delete_confirm(),
                Err(e) => {
                    log::error!("{e}");
                    self.delete_error = Some(e.to_string());
                }
            }
        } else if cancel_clicked || modal.should_close() {
            self.close_delete_confirm();
        }
    }

    fn close_delete_confirm(&mut self) {
        self.delete_confirm_open = false;
        self.delete_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_egui::app::AppDataCtx;
    use tempfile::TempDir;

    fn handler_with_presets(tmp: &TempDir, names: &[&str]) -> PresetHandler {
        let mut handler = PresetHandler::new(tmp.path().to_path_buf());
        for name in names {
            let data = AppDataCtx {
                spline_mode: SplineMode::Linear,
                control_points: Vec::new(),
            };
            handler.presets.push(PresetEntity::new(name, data));
        }
        handler
    }

    fn draw_frame(ctx: &egui::Context, window: &mut WindowPreset, handler: &mut PresetHandler) {
        let mut control_points = Vec::new();
        let mut spline_mode = SplineMode::Linear;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                window.draw_content(ui, handler, &mut control_points, &mut spline_mode);
            });
        });
    }

    #[test]
    fn rename_error_cleared_when_selection_changes() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A", "C"]);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();

        handler.preset_selected_index = Some(0);
        draw_frame(&ctx, &mut window, &mut handler);
        window.rename_error = Some("not found".to_string());
        draw_frame(&ctx, &mut window, &mut handler);
        assert!(window.rename_error.is_some());

        handler.preset_selected_index = Some(1);
        draw_frame(&ctx, &mut window, &mut handler);
        assert_eq!(window.rename_error, None);
    }

    #[test]
    fn spline_mode_label_names_every_mode() {
        assert_eq!(spline_mode_label(SplineMode::Linear), "Linear");
        assert_eq!(spline_mode_label(SplineMode::Bezier), "Bezier");
        assert_eq!(spline_mode_label(SplineMode::HermiteBezier), "Hermite");
    }

    #[test]
    fn save_selected_failure_sets_save_error_and_keeps_preset() {
        let tmp = TempDir::new().unwrap();
        let not_a_dir = tmp.path().join("presets");
        std::fs::write(&not_a_dir, "").unwrap();
        let mut handler = PresetHandler::new(not_a_dir);
        handler.presets.push(PresetEntity::new(
            "A",
            AppDataCtx {
                spline_mode: SplineMode::Linear,
                control_points: Vec::new(),
            },
        ));
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();

        window.save_selected(&mut handler, &[], SplineMode::Bezier);

        assert!(window.save_error.is_some());
        assert_eq!(handler.presets.len(), 1);
        assert_eq!(handler.presets[0].name, "A");
    }

    #[test]
    fn save_selected_success_clears_save_error() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A"]);
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();
        window.save_error = Some("old".to_string());

        window.save_selected(&mut handler, &[], SplineMode::Bezier);

        assert_eq!(window.save_error, None);
        assert!(tmp.path().join("A.json").is_file());
    }

    #[test]
    fn save_selected_with_stale_index_sets_save_error() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &[]);
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();

        window.save_selected(&mut handler, &[], SplineMode::Bezier);

        assert!(window.save_error.is_some());
    }

    #[test]
    fn save_error_cleared_when_selection_changes() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A", "C"]);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();

        handler.preset_selected_index = Some(0);
        draw_frame(&ctx, &mut window, &mut handler);
        window.save_error = Some("disk full".to_string());
        draw_frame(&ctx, &mut window, &mut handler);
        assert!(window.save_error.is_some());

        handler.preset_selected_index = Some(1);
        draw_frame(&ctx, &mut window, &mut handler);
        assert_eq!(window.save_error, None);
    }

    #[test]
    fn stale_selection_with_no_presets_does_not_panic() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &[]);
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();

        draw_frame(&ctx, &mut window, &mut handler);
    }
}
