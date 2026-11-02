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
use crate::ui_egui::color_picker::format_color_as;
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
        spline_mode: &mut SplineMode,
        color_copy_format: &mut ColorStringCopy,
    ) -> bool {
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
            let insert_hover = if options.is_insert_right {
                "New points go right of the selected point (at the end when none is selected)"
            } else {
                "New points go left of the selected point (at the start when none is selected)"
            };
            ui.checkbox(&mut options.is_insert_right, insert_label)
                .on_hover_text(insert_hover);
            ui.checkbox(&mut options.is_window_lock, "Clamp")
                .on_hover_text("Clamps the control points so they are contained");
            ui.add_enabled(
                *spline_mode == SplineMode::Bezier,
                egui::Checkbox::new(&mut options.is_show_curvature_comb, "Comb"),
            )
            .on_hover_text("Show curvature comb")
            .on_disabled_hover_text("The comb is only drawn in Bezier mode");
        });

        ui.horizontal_wrapped(|ui| {
            ui.label("Copy as");
            egui::ComboBox::new(12312312, "")
                .selected_text(format!("{:?}", *color_copy_format))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    for format in COPY_FORMATS {
                        ui.selectable_value(color_copy_format, format, copy_format_label(format));
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
                        ui.selectable_value(spline_mode, mode, spline_mode_label(mode))
                            .on_hover_text(spline_mode_hover_text(mode));
                    }
                })
                .response
                .on_hover_text("Spline Mode");

            ui.button("Flip").clicked_by(PointerButton::Primary)
        })
        .inner
    }
}

pub fn flip_control_points(
    points: &mut [ControlPoint],
    selected: &mut Option<usize>,
    preview_sizes: &mut [f32],
) {
    for cp in points.iter_mut() {
        cp.flip_tangents();
    }
    points.reverse();
    *selected = selected.and_then(|i| points.len().checked_sub(i + 1));
    if preview_sizes.len() == points.len() {
        preview_sizes.reverse();
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
    pending_switch: Option<usize>,
}

const COPY_FORMATS: [ColorStringCopy; 11] = [
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
];

const COPY_FORMAT_SAMPLE: egui::Color32 = egui::Color32::from_rgb(255, 128, 0);

fn copy_format_label(format: ColorStringCopy) -> String {
    let sample = format_color_as(COPY_FORMAT_SAMPLE, format, None);
    format!("{format:?}  {sample}")
}

fn spline_mode_label(mode: SplineMode) -> &'static str {
    match mode {
        SplineMode::Linear => "Linear",
        SplineMode::Bezier => "Bezier",
        SplineMode::HermiteBezier => "Hermite",
    }
}

fn spline_mode_hover_text(mode: SplineMode) -> &'static str {
    match mode {
        SplineMode::Linear => "Straight segments between points",
        SplineMode::Bezier => "Curved; select a point to drag its tangent handles",
        SplineMode::HermiteBezier => {
            "Smooth curve through every point (Catmull-Rom); tangents are automatic"
        }
    }
}

fn selected_preset(preset_handler: &PresetHandler) -> Option<&PresetEntity> {
    preset_handler
        .preset_selected_index
        .and_then(|i| preset_handler.presets.get(i))
}

fn has_unsaved_changes(
    preset_handler: &PresetHandler,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> bool {
    !preset_handler.auto_save_presets
        && preset_handler.is_selected_dirty(control_points, spline_mode)
}

fn combo_selected_text(name: Option<&str>, dirty: bool) -> String {
    match name {
        Some(name) if dirty => format!("{name} *"),
        Some(name) => name.to_string(),
        None => "<None>".to_string(),
    }
}

fn apply_preset(
    index: usize,
    preset_handler: &mut PresetHandler,
    control_points: &mut Vec<ControlPoint>,
    spline_mode: &mut SplineMode,
) -> PresetEntity {
    preset_handler.preset_selected_index = Some(index);
    log::info!("Selected Preset {index}");
    preset_handler.apply_selected_preset(control_points, spline_mode);
    preset_handler.presets[index].clone()
}

#[derive(Clone, Copy, Debug)]
enum PendingSwitchAction {
    SaveAndSwitch,
    Discard,
    Cancel,
}

#[derive(Clone, Copy, Debug)]
enum PresetComboChoice {
    Preset(usize),
    New,
    Clear,
}

fn is_combo_row_checked(row: PresetComboChoice, selected: Option<usize>) -> bool {
    match row {
        PresetComboChoice::Preset(i) => selected == Some(i),
        PresetComboChoice::New => false,
        PresetComboChoice::Clear => selected.is_none(),
    }
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
            pending_switch: None,
        }
    }

    pub fn update(&mut self) {}

    pub fn is_modal_open(&self) -> bool {
        self.delete_confirm_open || self.pending_switch.is_some()
    }

    fn choose_preset(
        &mut self,
        index: usize,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
    ) -> Option<PresetEntity> {
        if has_unsaved_changes(preset_handler, control_points, *spline_mode) {
            self.pending_switch = Some(index);
            self.save_error = None;
            return None;
        }
        Some(apply_preset(
            index,
            preset_handler,
            control_points,
            spline_mode,
        ))
    }

    fn resolve_pending_switch(
        &mut self,
        action: PendingSwitchAction,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
    ) -> Option<PresetEntity> {
        let target = self.pending_switch?;
        let should_apply = match action {
            PendingSwitchAction::Cancel => false,
            PendingSwitchAction::Discard => true,
            PendingSwitchAction::SaveAndSwitch => {
                self.save_selected(preset_handler, control_points, *spline_mode);
                if self.save_error.is_some() {
                    return None;
                }
                true
            }
        };
        self.pending_switch = None;
        should_apply.then(|| apply_preset(target, preset_handler, control_points, spline_mode))
    }

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
            let dirty = has_unsaved_changes(preset_handler, control_points, *spline_mode);
            let combobox_selected_text_to_show = combo_selected_text(
                selected_preset(preset_handler).map(|p| p.name.as_str()),
                dirty,
            );

            let selected_index = preset_handler.preset_selected_index;
            let mut choice = None;
            let _combobox_response = egui::ComboBox::new(1232313, "")
                .selected_text(combobox_selected_text_to_show)
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);

                    let rows = preset_handler
                        .presets
                        .iter()
                        .enumerate()
                        .map(|(i, preset)| (PresetComboChoice::Preset(i), preset.name.as_str()))
                        .chain([
                            (PresetComboChoice::New, "<NEW>"),
                            (PresetComboChoice::Clear, "<None>"),
                        ]);
                    for (row, text) in rows {
                        let checked = is_combo_row_checked(row, selected_index);
                        if ui.selectable_label(checked, text).clicked() {
                            choice = Some(row);
                        }
                    }
                })
                .response
                .on_hover_text("Presets");

            match choice {
                Some(PresetComboChoice::New) => {
                    self.new_preset_is_open = true;
                    self.new_preset_window_text.clear();
                    self.create_error = None;
                    log::info!("Selected New Preset");
                }
                Some(PresetComboChoice::Preset(i)) => {
                    draw_result.preset_result.should_apply =
                        self.choose_preset(i, preset_handler, control_points, spline_mode);
                }
                Some(PresetComboChoice::Clear) => preset_handler.preset_selected_index = None,
                None => {}
            }

            ui.checkbox(&mut preset_handler.auto_save_presets, "Auto-save")
                .on_hover_text("Auto save preset");
            let has_selection = selected_preset(preset_handler).is_some();
            let save_hover_text = if dirty {
                "Unsaved changes"
            } else {
                "Overwrite the selected preset with the current points"
            };
            if ui
                .add_enabled(has_selection, egui::Button::new("Save"))
                .on_hover_text(save_hover_text)
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
            rename_text_field.clear();
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
        if self.pending_switch.is_some() {
            if let Some(applied) =
                self.draw_pending_switch_modal(ui, preset_handler, control_points, spline_mode)
            {
                draw_result.preset_result.should_apply = Some(applied);
            }
        }

        draw_result
    }

    fn draw_pending_switch_modal(
        &mut self,
        ui: &mut Ui,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
    ) -> Option<PresetEntity> {
        let Some(name) = selected_preset(preset_handler).map(|preset| preset.name.clone()) else {
            self.pending_switch = None;
            return None;
        };

        let modal = egui::Modal::new(egui::Id::new("unsaved_preset_modal")).show(ui.ctx(), |ui| {
            ui.label(format!("\"{name}\" has unsaved changes."));
            if let Some(error) = &self.save_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| {
                let mut action = None;
                for (label, button_action) in [
                    ("Save and switch", PendingSwitchAction::SaveAndSwitch),
                    ("Discard", PendingSwitchAction::Discard),
                    ("Cancel", PendingSwitchAction::Cancel),
                ] {
                    if ui.button(label).clicked() {
                        action = Some(button_action);
                    }
                }
                action
            })
            .inner
        });
        let action = modal
            .inner
            .or_else(|| modal.should_close().then_some(PendingSwitchAction::Cancel))?;
        self.resolve_pending_switch(action, preset_handler, control_points, spline_mode)
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
    fn copy_format_label_shows_a_distinct_sample_after_each_format_name() {
        let mut samples: Vec<String> = Vec::new();
        for format in COPY_FORMATS {
            let label = copy_format_label(format);
            let sample = label
                .strip_prefix(&format!("{format:?}"))
                .unwrap_or_else(|| panic!("{label:?} does not start with {format:?}"))
                .trim()
                .to_string();
            assert!(!sample.is_empty(), "{format:?} has no sample");
            assert!(!samples.contains(&sample), "{format:?} repeats {sample:?}");
            samples.push(sample);
        }
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
    fn combo_row_checked_only_for_active_preset_or_none_row() {
        let cases = [
            (PresetComboChoice::Preset(1), Some(1), true),
            (PresetComboChoice::Preset(1), Some(0), false),
            (PresetComboChoice::Preset(1), None, false),
            (PresetComboChoice::New, None, false),
            (PresetComboChoice::New, Some(0), false),
            (PresetComboChoice::Clear, None, true),
            (PresetComboChoice::Clear, Some(0), false),
        ];
        for (row, selected, expected) in cases {
            assert_eq!(
                is_combo_row_checked(row, selected),
                expected,
                "{row:?} with {selected:?}"
            );
        }
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

    fn three_points() -> Vec<ControlPoint> {
        use crate::datatypes::control_point::{ControlPointStorage, ControlPointValue};
        (0..3)
            .map(|i| {
                let f = i as f32;
                ControlPoint::ControlPointLeftRightTangent(ControlPointStorage {
                    val: ControlPointValue::new(0.1 + 0.2 * f, 0.9 - 0.3 * f, 0.25 * f),
                    t: 0.5 * f,
                    tangents: [
                        Some(ControlPointValue::new(0.05 * (f + 1.0), 0.0, 0.0)),
                        None,
                    ],
                })
            })
            .collect()
    }

    #[test]
    fn flip_reverses_points_and_swaps_their_tangents() {
        let original = three_points();
        let mut points = original.clone();

        flip_control_points(&mut points, &mut None, &mut []);

        assert_eq!(points.len(), original.len());
        for (flipped, orig) in points.iter().zip(original.iter().rev()) {
            assert_eq!(flipped.val(), orig.val());
            assert_eq!(flipped.t(), orig.t());
            assert_eq!(flipped.tangents()[0], orig.tangents()[1]);
            assert_eq!(flipped.tangents()[1], orig.tangents()[0]);
        }
    }

    #[test]
    fn flip_keeps_the_same_point_selected() {
        for (before, after) in [
            (Some(0), Some(2)),
            (Some(1), Some(1)),
            (Some(2), Some(0)),
            (None, None),
            (Some(3), None),
        ] {
            let mut points = three_points();
            let mut selected = before;

            flip_control_points(&mut points, &mut selected, &mut []);

            assert_eq!(selected, after, "selected before flip: {before:?}");
        }
    }

    #[test]
    fn flip_reverses_matching_preview_sizes() {
        let mut points = three_points();
        let mut sizes = [10.0, 20.0, 30.0];

        flip_control_points(&mut points, &mut None, &mut sizes);

        assert_eq!(sizes, [30.0, 20.0, 10.0]);
    }

    #[test]
    fn flip_leaves_preview_sizes_of_another_length_untouched() {
        let mut points = three_points();
        let mut sizes = [10.0, 20.0];

        flip_control_points(&mut points, &mut None, &mut sizes);

        assert_eq!(sizes, [10.0, 20.0]);
    }

    #[test]
    fn flip_twice_is_the_identity() {
        let original = three_points();
        let mut points = original.clone();
        let mut selected = Some(0);
        let mut sizes = [10.0, 20.0, 30.0];

        flip_control_points(&mut points, &mut selected, &mut sizes);
        flip_control_points(&mut points, &mut selected, &mut sizes);

        assert_eq!(points, original);
        assert_eq!(selected, Some(0));
        assert_eq!(sizes, [10.0, 20.0, 30.0]);
    }

    fn handler_with_edited_a_and_distinct_b(tmp: &TempDir) -> (PresetHandler, Vec<ControlPoint>) {
        let mut handler = handler_with_presets(tmp, &["A", "B"]);
        handler.presets[1].data = AppDataCtx {
            spline_mode: SplineMode::Bezier,
            control_points: three_points(),
        };
        handler.preset_selected_index = Some(0);
        (handler, three_points()[..2].to_vec())
    }

    fn pending_switch_to_b(tmp: &TempDir) -> (WindowPreset, PresetHandler, Vec<ControlPoint>) {
        let (mut handler, mut points) = handler_with_edited_a_and_distinct_b(tmp);
        let mut window = WindowPreset::new();
        let applied = window.choose_preset(1, &mut handler, &mut points, &mut SplineMode::Linear);
        assert!(applied.is_none());
        (window, handler, points)
    }

    #[test]
    fn dirty_choose_defers_the_switch() {
        let tmp = TempDir::new().unwrap();
        let (mut handler, edited) = handler_with_edited_a_and_distinct_b(&tmp);
        let mut window = WindowPreset::new();
        let mut points = edited.clone();
        let mut mode = SplineMode::Linear;

        let applied = window.choose_preset(1, &mut handler, &mut points, &mut mode);

        assert!(applied.is_none());
        assert_eq!(window.pending_switch, Some(1));
        assert!(window.is_modal_open());
        assert_eq!(points, edited);
        assert_eq!(mode, SplineMode::Linear);
        assert_eq!(handler.preset_selected_index, Some(0));
    }

    #[test]
    fn discard_applies_the_pending_target() {
        let tmp = TempDir::new().unwrap();
        let (mut window, mut handler, mut points) = pending_switch_to_b(&tmp);
        let mut mode = SplineMode::Linear;

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::Discard,
            &mut handler,
            &mut points,
            &mut mode,
        );

        assert_eq!(applied.map(|p| p.name), Some("B".to_string()));
        assert_eq!(points, three_points());
        assert_eq!(mode, SplineMode::Bezier);
        assert_eq!(handler.preset_selected_index, Some(1));
        assert_eq!(window.pending_switch, None);
    }

    #[test]
    fn cancel_keeps_the_points_and_the_selection() {
        let tmp = TempDir::new().unwrap();
        let (mut window, mut handler, mut points) = pending_switch_to_b(&tmp);
        let edited = points.clone();
        let mut mode = SplineMode::Linear;

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::Cancel,
            &mut handler,
            &mut points,
            &mut mode,
        );

        assert!(applied.is_none());
        assert_eq!(points, edited);
        assert_eq!(mode, SplineMode::Linear);
        assert_eq!(handler.preset_selected_index, Some(0));
        assert_eq!(window.pending_switch, None);
        assert!(!window.is_modal_open());
        assert!(!tmp.path().join("A.json").exists());
    }

    #[test]
    fn save_and_switch_writes_the_old_preset_then_applies_the_target() {
        let tmp = TempDir::new().unwrap();
        let (mut window, mut handler, mut points) = pending_switch_to_b(&tmp);
        let edited = points.clone();
        let mut mode = SplineMode::Linear;

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::SaveAndSwitch,
            &mut handler,
            &mut points,
            &mut mode,
        );

        let saved: AppDataCtx =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("A.json")).unwrap())
                .unwrap();
        assert_eq!(saved.control_points, edited);
        assert_eq!(saved.spline_mode, SplineMode::Linear);
        assert_eq!(applied.map(|p| p.name), Some("B".to_string()));
        assert_eq!(points, three_points());
        assert_eq!(mode, SplineMode::Bezier);
        assert_eq!(handler.preset_selected_index, Some(1));
        assert_eq!(window.pending_switch, None);
    }

    #[test]
    fn failed_save_and_switch_keeps_the_modal_open() {
        let tmp = TempDir::new().unwrap();
        let not_a_dir = tmp.path().join("presets");
        std::fs::write(&not_a_dir, "").unwrap();
        let mut handler = PresetHandler::new(not_a_dir);
        for name in ["A", "B"] {
            handler.presets.push(PresetEntity::new(
                name,
                AppDataCtx {
                    spline_mode: SplineMode::Linear,
                    control_points: Vec::new(),
                },
            ));
        }
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();
        let edited = three_points();
        let mut points = edited.clone();
        let mut mode = SplineMode::Linear;
        assert!(window
            .choose_preset(1, &mut handler, &mut points, &mut mode)
            .is_none());

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::SaveAndSwitch,
            &mut handler,
            &mut points,
            &mut mode,
        );

        assert!(applied.is_none());
        assert!(window.save_error.is_some());
        assert_eq!(window.pending_switch, Some(1));
        assert_eq!(handler.preset_selected_index, Some(0));
        assert_eq!(points, edited);
    }

    #[test]
    fn clean_auto_saved_or_unselected_choose_applies_immediately() {
        let cases = [
            ("clean", false, Some(0), Vec::new()),
            ("auto-save", true, Some(0), three_points()[..2].to_vec()),
            ("no selection", false, None, three_points()[..2].to_vec()),
        ];
        for (case, auto_save, selected, mut points) in cases {
            let tmp = TempDir::new().unwrap();
            let (mut handler, _) = handler_with_edited_a_and_distinct_b(&tmp);
            handler.auto_save_presets = auto_save;
            handler.preset_selected_index = selected;
            let mut window = WindowPreset::new();
            let mut mode = SplineMode::Linear;

            let applied = window.choose_preset(1, &mut handler, &mut points, &mut mode);

            assert_eq!(applied.map(|p| p.name), Some("B".to_string()), "{case}");
            assert_eq!(points, three_points(), "{case}");
            assert_eq!(mode, SplineMode::Bezier, "{case}");
            assert_eq!(handler.preset_selected_index, Some(1), "{case}");
            assert_eq!(window.pending_switch, None, "{case}");
        }
    }

    #[test]
    fn unsaved_changes_follow_edits_saves_and_auto_save() {
        let tmp = TempDir::new().unwrap();
        let (mut handler, edited) = handler_with_edited_a_and_distinct_b(&tmp);
        let mut window = WindowPreset::new();

        assert!(!has_unsaved_changes(&handler, &[], SplineMode::Linear));
        assert!(has_unsaved_changes(&handler, &edited, SplineMode::Linear));

        window.save_selected(&mut handler, &edited, SplineMode::Linear);
        assert!(!has_unsaved_changes(&handler, &edited, SplineMode::Linear));

        handler.auto_save_presets = true;
        assert!(!has_unsaved_changes(&handler, &[], SplineMode::Bezier));
    }

    #[test]
    fn rename_text_cleared_when_selection_changes() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A", "C"]);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();
        let rename_id = egui::Id::new(819181);
        let rename_text = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<String>(rename_id))
                .unwrap_or_default()
        };

        handler.preset_selected_index = Some(0);
        draw_frame(&ctx, &mut window, &mut handler);
        ctx.data_mut(|d| d.insert_temp(rename_id, "X".to_string()));
        draw_frame(&ctx, &mut window, &mut handler);
        assert_eq!(rename_text(&ctx), "X");

        handler.preset_selected_index = Some(1);
        draw_frame(&ctx, &mut window, &mut handler);
        assert_eq!(rename_text(&ctx), "");
    }

    #[test]
    fn combo_selected_text_marks_unsaved_changes() {
        assert_eq!(combo_selected_text(Some("A"), false), "A");
        assert_eq!(combo_selected_text(Some("A"), true), "A *");
        assert_eq!(combo_selected_text(None, false), "<None>");
        assert_eq!(combo_selected_text(None, true), "<None>");
    }
}
