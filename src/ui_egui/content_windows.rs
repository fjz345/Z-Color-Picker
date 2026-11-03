use std::path::Path;

use eframe::egui;
use eframe::egui::TextEdit;

use crate::color_space::{BlendSpace, PickerSpace};
use crate::common::ColorStringCopy;
use crate::common::SplineMode;
use crate::datatypes::control_point::ControlPoint;
use crate::egui::PointerButton;
use crate::egui::Ui;
use crate::preset::PresetEntity;
use crate::preset::PresetHandler;
use crate::ui_egui::app::ZColorPickerOptions;
use crate::ui_egui::color_picker::format_color_as;
use crate::ui_egui::control_points::mirror_t;
use crate::ui_egui::curvature::COMB_SCALE_RANGE;
use crate::ui_egui::theme::{row_header, section_header, unbroken_group};
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

pub fn picker_space_combo(ui: &mut Ui, picker_space: &mut PickerSpace) {
    ui.label("Space");
    let clip_to_srgb = picker_space.clips_to_srgb();
    egui::ComboBox::from_id_salt("picker_space")
        .selected_text(picker_space_label(*picker_space))
        .show_ui(ui, |ui| {
            ui.set_min_width(60.0);
            for space in PickerSpace::ALL {
                let space = space.with_clip_to_srgb(clip_to_srgb);
                ui.selectable_value(picker_space, space, picker_space_label(space))
                    .on_hover_text(picker_space_hover_text(space));
            }
        })
        .response
        .on_hover_text(
            "Color space of the square and the bar; switching converts every point \
             and keeps its color, except middle points under Auto hue: their hues are \
             re-interpolated in the new space. Points outside sRGB keep their color only \
             between OkLCh and Oklab",
        );
}

pub fn clip_to_srgb_checkbox(ui: &mut Ui, picker_space: &mut PickerSpace) {
    let mut clip_to_srgb = picker_space.clips_to_srgb();
    ui.add_enabled(
        picker_space.can_leave_srgb(),
        egui::Checkbox::new(&mut clip_to_srgb, "Clip"),
    )
    .on_hover_text(
        "Clip to sRGB: blank the colors outside sRGB, and pull every point and handle you edit \
         into sRGB by reducing its chroma; under Lock the group stops at the edge instead. \
         Turning it on pulls in every point and handle once",
    )
    .on_disabled_hover_text("Only OkLCh and Oklab reach outside sRGB");
    *picker_space = picker_space.with_clip_to_srgb(clip_to_srgb);
}

pub fn points_row(ui: &mut Ui, options: &mut ZColorPickerOptions, picker_space: PickerSpace) {
    ui.horizontal_wrapped(|ui| {
        row_header(ui, "Points");
        ui.checkbox(&mut options.is_curve_locked, "Lock")
            .on_hover_text("Move all control points together, also for S/V edits in the inspector");
        ui.checkbox(&mut options.is_window_lock, "Clamp")
            .on_hover_text(
                "Keep points inside the square and wrap hue into 0..1 (Oklab: clamp L into 0..1)",
            );
        let mut auto_hue = options.is_auto_hue(picker_space);
        let changed = ui
            .add_enabled(
                picker_space.is_z_hue(),
                egui::Checkbox::new(&mut auto_hue, "Auto hue"),
            )
            .on_hover_text("Only the first and last hue are editable; middle hues are interpolated")
            .on_disabled_hover_text("Oklab has no hue: its bar is lightness")
            .changed();
        if changed {
            options.is_hue_middle_interpolated = auto_hue;
        }
        unbroken_group(ui, "insert", |ui| {
            ui.label("Insert");
            ui.radio_value(&mut options.is_insert_right, false, "left")
                .on_hover_text(
                    "New points go left of the selected point \
                     (at the start when none is selected)",
                );
            ui.radio_value(&mut options.is_insert_right, true, "right")
                .on_hover_text(
                    "New points go right of the selected point \
                     (at the end when none is selected)",
                );
        });
    });
}

pub fn curve_row(ui: &mut Ui, spline_mode: &mut SplineMode, blend_space: &mut BlendSpace) -> bool {
    ui.horizontal_wrapped(|ui| {
        row_header(ui, "Curve");
        unbroken_group(ui, "spline", |ui| {
            ui.label("Spline");
            egui::ComboBox::from_id_salt("spline_mode")
                .selected_text(spline_mode_label(*spline_mode))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    for mode in SplineMode::ALL {
                        ui.selectable_value(spline_mode, mode, spline_mode_label(mode))
                            .on_hover_text(spline_mode_hover_text(mode));
                    }
                })
                .response
                .on_hover_text(spline_mode_hover_text(*spline_mode));
        });

        unbroken_group(ui, "blend", |ui| {
            ui.label("Blend");
            egui::ComboBox::from_id_salt("blend_space")
                .selected_text(blend_space_label(*blend_space))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    for space in BlendSpace::ALL {
                        ui.selectable_value(blend_space, space, blend_space_label(space))
                            .on_hover_text(blend_space_hover_text(space));
                    }
                })
                .response
                .on_hover_text(
                    "Color space the gradient is interpolated in; the points keep their \
                     colors. Any blend but Picker turns off the comb and Auto comb",
                );
        });

        ui.button("Flip")
            .on_hover_text("Reverse the point order (tangents and T follow)")
            .clicked_by(PointerButton::Primary)
    })
    .inner
}

pub fn comb_row(
    ui: &mut Ui,
    options: &mut ZColorPickerOptions,
    spline_mode: SplineMode,
    blend_space: BlendSpace,
    auto_comb_unsolved: usize,
) {
    let enabled = comb_controls_enabled(spline_mode, blend_space, options.is_show_curvature_comb);
    let is_picker_blend = blend_space == BlendSpace::Picker;
    let straight_hint = "Linear segments are straight: there is no curvature to show";
    let comb_off_hint = if is_picker_blend {
        straight_hint
    } else {
        NON_PICKER_BLEND_COMB_HINT
    };
    ui.horizontal_wrapped(|ui| {
        row_header(ui, "Comb");
        ui.add_enabled(
            enabled.show,
            egui::Checkbox::new(&mut options.is_show_curvature_comb, "Show"),
        )
        .on_hover_text("Curvature comb: spikes show how sharply the curve bends")
        .on_disabled_hover_text(comb_off_hint);
        unbroken_group(ui, "scale", |ui| {
            ui.add_enabled_ui(enabled.scale, |ui| {
                ui.label("Scale");
                ui.add(
                    egui::Slider::new(&mut options.curvature_comb_scale, COMB_SCALE_RANGE)
                        .logarithmic(true)
                        .fixed_decimals(2)
                        .suffix("×"),
                )
                .on_hover_text("Length of the comb spikes")
                .on_disabled_hover_text(if enabled.show {
                    "Turn on Show first"
                } else {
                    comb_off_hint
                });
            });
        });

        ui.add_enabled(
            enabled.auto,
            egui::Checkbox::new(&mut options.is_auto_comb, "Auto comb"),
        )
        .on_hover_text(
            "Solve the handle lengths so the comb is continuous, keeping each point's \
             curvature as close as it can. Turn a handle to turn its point; pull it longer \
             or shorter to change the point's curvature",
        )
        .on_disabled_hover_text(if is_picker_blend {
            "Auto comb only runs in Bezier mode"
        } else {
            NON_PICKER_BLEND_COMB_HINT
        });
        if enabled.auto && options.is_auto_comb && auto_comb_unsolved > 0 {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!("{auto_comb_unsolved} unsolved"),
            )
            .on_hover_text(
                "Segments that keep their handle lengths: no lengths inside the square make \
                 the comb continuous at their point with the current handle directions; \
                 turn a handle there",
            );
        }
    });
}

const NON_PICKER_BLEND_COMB_HINT: &str =
    "Off while Blend is not Picker: the comb measures the curve on the square, \
     and the gradient is blended in another space";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombControlsEnabled {
    pub show: bool,
    pub scale: bool,
    pub auto: bool,
}

pub fn comb_controls_enabled(
    spline_mode: SplineMode,
    blend_space: BlendSpace,
    is_shown: bool,
) -> CombControlsEnabled {
    let is_picker_curve = spline_mode != SplineMode::Linear && blend_space == BlendSpace::Picker;
    CombControlsEnabled {
        show: is_picker_curve,
        scale: is_picker_curve && is_shown,
        auto: spline_mode == SplineMode::Bezier && blend_space == BlendSpace::Picker,
    }
}

pub fn copy_format_combo(ui: &mut Ui, color_copy_format: &mut ColorStringCopy) {
    section_header(ui, "Copy as");
    egui::ComboBox::from_id_salt("copy_format")
        .selected_text(copy_format_label(*color_copy_format))
        .show_ui(ui, |ui| {
            ui.set_min_width(60.0);
            for format in COPY_FORMATS {
                ui.selectable_value(color_copy_format, format, copy_format_label(format));
            }
        })
        .response
        .on_hover_text(
            "Text format used when copying a color (readout click, Ctrl+C or \
             middle-click); the previewer strips are copied as images",
        );
}

pub fn flip_control_points(points: &mut [ControlPoint], selected: &mut Option<usize>) {
    for cp in points.iter_mut() {
        cp.flip_tangents();
        if let Some(k) = cp.curvature_mut() {
            *k = -*k;
        }
    }
    points.reverse();
    mirror_t(points);
    *selected = selected.and_then(|i| points.len().checked_sub(i + 1));
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

const COPY_FORMATS: [ColorStringCopy; 14] = [
    ColorStringCopy::Hex,
    ColorStringCopy::HexNoAlpha,
    ColorStringCopy::SrgbHex,
    ColorStringCopy::Hsv,
    ColorStringCopy::Hsva,
    ColorStringCopy::Int,
    ColorStringCopy::Float,
    ColorStringCopy::Rgb,
    ColorStringCopy::Srgb,
    ColorStringCopy::Rgba,
    ColorStringCopy::Srgba,
    ColorStringCopy::Hsl,
    ColorStringCopy::Oklab,
    ColorStringCopy::Oklch,
];

const COPY_FORMAT_SAMPLE: egui::Color32 = egui::Color32::from_rgb(255, 128, 0);

const RENAME_POPUP_ID: &str = "rename_preset_popup";
const SAVE_ERROR_MAX_WIDTH: f32 = 320.0;

fn copy_format_label(format: ColorStringCopy) -> String {
    let sample = format_color_as(COPY_FORMAT_SAMPLE, format, None);
    format!("{}  {sample}", format.label())
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

fn picker_space_label(space: PickerSpace) -> &'static str {
    match space {
        PickerSpace::Hsv => "HSV",
        PickerSpace::Hsl => "HSL",
        PickerSpace::Okhsv => "OKHSV",
        PickerSpace::Okhsl => "OKHSL",
        PickerSpace::Oklch { .. } => "OkLCh",
        PickerSpace::Oklab { .. } => "Oklab",
    }
}

fn picker_space_hover_text(space: PickerSpace) -> &'static str {
    match space {
        PickerSpace::Hsv => "Hue, saturation, value",
        PickerSpace::Hsl => "Hue, saturation, lightness: the full hue sits at L 50%",
        PickerSpace::Okhsv => "Perceptual HSV built on Oklab: even hue steps",
        PickerSpace::Okhsl => {
            "Perceptual HSL built on Oklab: equal L looks equally light across hues"
        }
        PickerSpace::Oklch { .. } => {
            "Oklab's lightness, chroma and hue: chroma across the square, lightness up, \
             hue on the bar. Reaches outside sRGB"
        }
        PickerSpace::Oklab { .. } => {
            "Oklab: a (green to red) across the square, b (blue to yellow) up, lightness \
             on the bar. Reaches outside sRGB"
        }
    }
}

fn blend_space_label(space: BlendSpace) -> &'static str {
    match space {
        BlendSpace::Picker => "Picker",
        BlendSpace::Srgb => "sRGB",
        BlendSpace::LinearRgb => "Linear RGB",
        BlendSpace::Oklab => "Oklab",
        BlendSpace::Oklch => "OkLCh",
    }
}

fn blend_space_hover_text(space: BlendSpace) -> &'static str {
    match space {
        BlendSpace::Picker => "The square's S/V (or S/L) and hue, as drawn on the picker",
        BlendSpace::Srgb => "Gamma-encoded sRGB channels, like most image editors",
        BlendSpace::LinearRgb => "Linear-light RGB: mixes like light, bright in the middle",
        BlendSpace::Oklab => "Perceptual Lab: even lightness steps, no hue shift",
        BlendSpace::Oklch => "Perceptual lightness, chroma and hue; hue takes the short way around",
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
    picker_space: PickerSpace,
    blend_space: BlendSpace,
) -> bool {
    preset_handler.is_selected_dirty(control_points, spline_mode, picker_space, blend_space)
        && (!preset_handler.auto_save_presets || preset_handler.has_failed_auto_save())
}

fn points_saved_nowhere(
    preset_handler: &PresetHandler,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    picker_space: PickerSpace,
    blend_space: BlendSpace,
) -> bool {
    selected_preset(preset_handler).is_none()
        && !control_points.is_empty()
        && !preset_handler.matches_any_preset(
            control_points,
            spline_mode,
            picker_space,
            blend_space,
        )
}

fn combo_selected_text(name: Option<&str>, dirty: bool) -> String {
    match name {
        Some(name) if dirty => format!("{name} *"),
        Some(name) => name.to_string(),
        None => "<None>".to_string(),
    }
}

fn auto_save_hint(has_selection: bool) -> &'static str {
    if has_selection {
        "Rewrite the selected preset file after every edit"
    } else {
        "Select a preset to auto-save into"
    }
}

fn no_presets_text(dir: &Path) -> String {
    format!("No presets in {}", dir.display())
}

fn apply_preset(
    index: usize,
    preset_handler: &mut PresetHandler,
    control_points: &mut Vec<ControlPoint>,
    spline_mode: &mut SplineMode,
    picker_space: &mut PickerSpace,
    blend_space: &mut BlendSpace,
) -> PresetEntity {
    preset_handler.preset_selected_index = Some(index);
    log::info!("Selected Preset {index}");
    preset_handler.apply_selected_preset(control_points, spline_mode, picker_space, blend_space);
    preset_handler.presets[index].clone()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum PendingSwitchAction {
    SaveAndSwitch,
    Discard,
    Cancel,
}

fn pending_switch_buttons(
    has_selection: bool,
    target_is_selected: bool,
) -> &'static [(&'static str, PendingSwitchAction)] {
    match (has_selection, target_is_selected) {
        (true, true) => &[
            ("Save", PendingSwitchAction::SaveAndSwitch),
            ("Revert to saved", PendingSwitchAction::Discard),
            ("Cancel", PendingSwitchAction::Cancel),
        ],
        (true, false) => &[
            ("Save and switch", PendingSwitchAction::SaveAndSwitch),
            ("Discard", PendingSwitchAction::Discard),
            ("Cancel", PendingSwitchAction::Cancel),
        ],
        (false, _) => &[
            ("Discard", PendingSwitchAction::Discard),
            ("Cancel", PendingSwitchAction::Cancel),
        ],
    }
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
        picker_space: &mut PickerSpace,
        blend_space: &mut BlendSpace,
    ) -> Option<PresetEntity> {
        if has_unsaved_changes(
            preset_handler,
            control_points,
            *spline_mode,
            *picker_space,
            *blend_space,
        ) || points_saved_nowhere(
            preset_handler,
            control_points,
            *spline_mode,
            *picker_space,
            *blend_space,
        ) {
            self.pending_switch = Some(index);
            self.save_error = None;
            return None;
        }
        Some(apply_preset(
            index,
            preset_handler,
            control_points,
            spline_mode,
            picker_space,
            blend_space,
        ))
    }

    fn resolve_pending_switch(
        &mut self,
        action: PendingSwitchAction,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
        picker_space: &mut PickerSpace,
        blend_space: &mut BlendSpace,
    ) -> Option<PresetEntity> {
        let target = self.pending_switch?;
        let should_apply = match action {
            PendingSwitchAction::Cancel => false,
            PendingSwitchAction::Discard => true,
            PendingSwitchAction::SaveAndSwitch => {
                self.save_selected(
                    preset_handler,
                    control_points,
                    *spline_mode,
                    *picker_space,
                    *blend_space,
                );
                if self.save_error.is_some() {
                    return None;
                }
                true
            }
        };
        self.pending_switch = None;
        should_apply.then(|| {
            apply_preset(
                target,
                preset_handler,
                control_points,
                spline_mode,
                picker_space,
                blend_space,
            )
        })
    }

    fn save_selected(
        &mut self,
        preset_handler: &mut PresetHandler,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
        picker_space: PickerSpace,
        blend_space: BlendSpace,
    ) {
        let Some(name) = selected_preset(preset_handler).map(|p| p.name.clone()) else {
            self.save_error = Some("No preset selected".to_string());
            return;
        };
        match preset_handler.save_selected_preset(
            control_points,
            spline_mode,
            picker_space,
            blend_space,
        ) {
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

    fn open_create_window(&mut self) {
        self.new_preset_is_open = true;
        self.new_preset_window_text.clear();
        self.create_error = None;
        log::info!("Opened Create Preset");
    }

    fn on_save_clicked(
        &mut self,
        preset_handler: &mut PresetHandler,
        control_points: &[ControlPoint],
        spline_mode: SplineMode,
        picker_space: PickerSpace,
        blend_space: BlendSpace,
    ) {
        if selected_preset(preset_handler).is_some() {
            self.save_selected(
                preset_handler,
                control_points,
                spline_mode,
                picker_space,
                blend_space,
            );
        } else {
            self.open_create_window();
        }
    }

    pub fn draw_content(
        &mut self,
        ui: &mut Ui,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
        picker_space: &mut PickerSpace,
        blend_space: &mut BlendSpace,
    ) -> WindowPresetDrawResult {
        let mut draw_result = WindowPresetDrawResult::default();

        let rename_text_field_id = egui::Id::new(819181);
        let mut rename_text_field: String = ui.memory(|mem| {
            mem.data
                .get_temp::<String>(rename_text_field_id)
                .unwrap_or_default()
        });

        ui.horizontal(|ui| {
            let dirty = has_unsaved_changes(
                preset_handler,
                control_points,
                *spline_mode,
                *picker_space,
                *blend_space,
            );
            let combobox_selected_text_to_show = combo_selected_text(
                selected_preset(preset_handler).map(|p| p.name.as_str()),
                dirty,
            );

            let selected_index = preset_handler.preset_selected_index;
            let mut choice = None;
            section_header(ui, "Preset");
            let _combobox_response = egui::ComboBox::new(1232313, "")
                .selected_text(combobox_selected_text_to_show)
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);

                    if preset_handler.presets.is_empty() {
                        ui.weak(no_presets_text(preset_handler.presets_dir()));
                    }
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
                .on_hover_text(format!(
                    "Presets in {}",
                    preset_handler.presets_dir().display()
                ));

            match choice {
                Some(PresetComboChoice::New) => self.open_create_window(),
                Some(PresetComboChoice::Preset(i)) => {
                    draw_result.preset_result.should_apply = self.choose_preset(
                        i,
                        preset_handler,
                        control_points,
                        spline_mode,
                        picker_space,
                        blend_space,
                    );
                }
                Some(PresetComboChoice::Clear) => preset_handler.preset_selected_index = None,
                None => {}
            }

            let selected = preset_handler.preset_selected_index;
            if selected != self.last_selection {
                rename_text_field.clear();
                self.rename_error = None;
                self.save_error = None;
                self.last_selection = selected;
            }

            let has_selection = selected_preset(preset_handler).is_some();
            let save_hover_text = if !has_selection {
                "Save the current points as a new preset"
            } else if dirty {
                "Unsaved changes"
            } else {
                "Overwrite the selected preset with the current points"
            };
            if ui
                .button("Save")
                .on_hover_text(save_hover_text)
                .clicked_by(PointerButton::Primary)
            {
                self.on_save_clicked(
                    preset_handler,
                    control_points,
                    *spline_mode,
                    *picker_space,
                    *blend_space,
                );
            }

            let rename_button = ui
                .add_enabled(has_selection, egui::Button::new("Rename..."))
                .on_hover_text("Rename the selected preset")
                .on_disabled_hover_text("Select a preset first");
            let rename_popup_id = egui::Id::new(RENAME_POPUP_ID);
            let is_rename_opening = rename_button.clicked_by(PointerButton::Primary);
            if is_rename_opening {
                ui.memory_mut(|mem| mem.toggle_popup(rename_popup_id));
            }
            egui::popup_below_widget(
                ui,
                rename_popup_id,
                &rename_button,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    self.draw_rename_popup(
                        ui,
                        preset_handler,
                        &mut rename_text_field,
                        is_rename_opening,
                    )
                },
            );

            if ui
                .add_enabled(has_selection, egui::Button::new("Delete"))
                .on_hover_text("Delete the selected preset from disk")
                .on_disabled_hover_text("Select a preset first")
                .clicked_by(PointerButton::Primary)
            {
                self.delete_confirm_open = true;
                self.delete_error = None;
            }
            ui.checkbox(&mut preset_handler.auto_save_presets, "Auto-save")
                .on_hover_text(auto_save_hint(has_selection));

            if let Some(error) = &self.save_error {
                let text = egui::RichText::new(error).color(ui.visuals().error_fg_color);
                ui.scope(|ui| {
                    ui.set_max_width(SAVE_ERROR_MAX_WIDTH);
                    ui.add(egui::Label::new(text).truncate());
                });
            }
        });

        ui.memory_mut(|mem| {
            *mem.data.get_temp_mut_or_default(rename_text_field_id) = rename_text_field
        });

        if self.delete_confirm_open {
            self.draw_delete_confirm_modal(ui, preset_handler);
        }
        if self.pending_switch.is_some() {
            if let Some(applied) = self.draw_pending_switch_modal(
                ui,
                preset_handler,
                control_points,
                spline_mode,
                picker_space,
                blend_space,
            ) {
                draw_result.preset_result.should_apply = Some(applied);
            }
        }

        draw_result
    }

    fn draw_rename_popup(
        &mut self,
        ui: &mut Ui,
        preset_handler: &mut PresetHandler,
        rename_text_field: &mut String,
        is_opening: bool,
    ) {
        let selected = preset_handler.preset_selected_index;
        let rename_reason = ui
            .horizontal(|ui| {
                let text_response = ui.add_enabled(
                    selected.is_some(),
                    TextEdit::singleline(rename_text_field)
                        .hint_text("New name")
                        .desired_width(200.0),
                );
                if is_opening {
                    text_response.request_focus();
                }
                if text_response.changed() {
                    self.rename_error = None;
                }
                let was_enter =
                    text_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

                let rename_check =
                    selected.map(|s| preset_handler.check_name(rename_text_field, Some(s)));
                let rename_button = ui.add_enabled(
                    matches!(rename_check, Some(Ok(()))),
                    egui::Button::new("Rename"),
                );
                let should_rename = was_enter || rename_button.clicked_by(PointerButton::Primary);
                match (selected, rename_check) {
                    (Some(s), Some(Ok(()))) if should_rename => {
                        match preset_handler.rename_preset(s, rename_text_field) {
                            Ok(()) => {
                                rename_text_field.clear();
                                self.rename_error = None;
                                ui.memory_mut(|mem| mem.close_popup());
                            }
                            Err(e) => {
                                log::error!("{e}");
                                self.rename_error = Some(e.to_string());
                            }
                        }
                        None
                    }
                    (_, Some(Err(reason))) if !rename_text_field.is_empty() => Some(reason),
                    _ => None,
                }
            })
            .inner;

        if let Some(reason) = rename_reason {
            ui.colored_label(ui.visuals().error_fg_color, reason.to_string());
        }
        if let Some(error) = &self.rename_error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn draw_pending_switch_modal(
        &mut self,
        ui: &mut Ui,
        preset_handler: &mut PresetHandler,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: &mut SplineMode,
        picker_space: &mut PickerSpace,
        blend_space: &mut BlendSpace,
    ) -> Option<PresetEntity> {
        let name = selected_preset(preset_handler).map(|preset| preset.name.clone());
        let buttons = pending_switch_buttons(
            name.is_some(),
            self.pending_switch == preset_handler.preset_selected_index,
        );

        let modal = egui::Modal::new(egui::Id::new("unsaved_preset_modal")).show(ui.ctx(), |ui| {
            match &name {
                Some(name) => {
                    ui.label(format!("\"{name}\" has unsaved changes."));
                }
                None => {
                    ui.label("The current points are not saved in any preset.");
                    ui.weak("Use <NEW> first to keep them.");
                }
            }
            if let Some(error) = &self.save_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| {
                let mut action = None;
                for &(label, button_action) in buttons {
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
        self.resolve_pending_switch(
            action,
            preset_handler,
            control_points,
            spline_mode,
            picker_space,
            blend_space,
        )
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
                picker_space: PickerSpace::Hsv,
                blend_space: BlendSpace::Picker,
            };
            handler.presets.push(PresetEntity::new(name, data));
        }
        handler
    }

    fn draw_frame(ctx: &egui::Context, window: &mut WindowPreset, handler: &mut PresetHandler) {
        draw_preset_frame(ctx, window, handler, Vec::new());
    }

    fn draw_preset_frame(
        ctx: &egui::Context,
        window: &mut WindowPreset,
        handler: &mut PresetHandler,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let mut control_points = Vec::new();
        let mut spline_mode = SplineMode::Linear;
        let mut picker_space = PickerSpace::Hsv;
        let mut blend_space = BlendSpace::Picker;
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                window.draw_content(
                    ui,
                    handler,
                    &mut control_points,
                    &mut spline_mode,
                    &mut picker_space,
                    &mut blend_space,
                );
            });
        })
    }

    fn primary_click(pos: egui::Pos2) -> [Vec<egui::Event>; 2] {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        [
            vec![egui::Event::PointerMoved(pos), button(true)],
            vec![button(false)],
        ]
    }

    fn click_preset_bar_text(
        ctx: &egui::Context,
        window: &mut WindowPreset,
        handler: &mut PresetHandler,
        text: &str,
    ) {
        let output = draw_preset_frame(ctx, window, handler, Vec::new());
        for events in primary_click(painted_text_center(&output, text)) {
            draw_preset_frame(ctx, window, handler, events);
        }
    }

    fn is_rename_popup_open(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.is_popup_open(egui::Id::new(RENAME_POPUP_ID)))
    }

    #[test]
    fn rename_popup_needs_a_selection() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A"]);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();

        click_preset_bar_text(&ctx, &mut window, &mut handler, "Rename...");
        draw_frame(&ctx, &mut window, &mut handler);

        assert!(!ctx.memory(|m| m.any_popup_open()));
    }

    #[test]
    fn open_rename_popup_counts_as_a_popup() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A"]);
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();

        click_preset_bar_text(&ctx, &mut window, &mut handler, "Rename...");
        draw_frame(&ctx, &mut window, &mut handler);

        assert!(is_rename_popup_open(&ctx));
        assert!(ctx.memory(|m| m.any_popup_open()));
    }

    #[test]
    fn rename_popup_renames_on_enter_and_closes() {
        let tmp = TempDir::new().unwrap();
        let mut handler = PresetHandler::new(tmp.path().to_path_buf());
        handler
            .create_preset(
                &"A".to_string(),
                &Vec::new(),
                &SplineMode::Linear,
                PickerSpace::Hsv,
                BlendSpace::Picker,
            )
            .unwrap();
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();
        let enter = egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };

        click_preset_bar_text(&ctx, &mut window, &mut handler, "Rename...");
        draw_preset_frame(
            &ctx,
            &mut window,
            &mut handler,
            vec![egui::Event::Text("B".into())],
        );
        draw_preset_frame(&ctx, &mut window, &mut handler, vec![enter]);
        draw_frame(&ctx, &mut window, &mut handler);

        assert_eq!(handler.presets[0].name, "B");
        assert!(tmp.path().join("B.json").is_file());
        assert!(!is_rename_popup_open(&ctx));
    }

    #[test]
    fn copy_format_label_shows_a_distinct_sample_after_each_format_name() {
        let mut samples: Vec<String> = Vec::new();
        for format in COPY_FORMATS {
            let label = copy_format_label(format);
            let sample = label
                .strip_prefix(format.label())
                .unwrap_or_else(|| panic!("{label:?} does not start with {format:?}"))
                .trim()
                .to_string();
            assert!(!sample.is_empty(), "{format:?} has no sample");
            assert!(!samples.contains(&sample), "{format:?} repeats {sample:?}");
            samples.push(sample);
        }
    }

    #[test]
    fn copy_format_labels_keep_the_all_caps_names() {
        let labels = COPY_FORMATS.map(ColorStringCopy::label);
        assert_eq!(
            labels,
            [
                "HEX", "HEXNOA", "SRGBHEX", "HSV", "HSVA", "INT", "FLOAT", "RGB", "SRGB", "RGBA",
                "SRGBA", "HSL", "OKLAB", "OKLCH"
            ]
        );
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

    fn options_painted_texts(is_insert_right: bool) -> Vec<String> {
        let options = ZColorPickerOptions {
            is_insert_right,
            ..Default::default()
        };
        painted_option_texts(options, SplineMode::Linear, 0)
    }

    fn painted_option_texts(
        mut options: ZColorPickerOptions,
        spline_mode: SplineMode,
        auto_comb_unsolved: usize,
    ) -> Vec<String> {
        let ctx = egui::Context::default();
        let output = draw_options_frame(
            &ctx,
            Vec::new(),
            &mut options,
            spline_mode,
            auto_comb_unsolved,
        );
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    fn draw_options_frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        options: &mut ZColorPickerOptions,
        spline_mode: SplineMode,
        auto_comb_unsolved: usize,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                points_row(ui, options, PickerSpace::Hsv);
                comb_row(
                    ui,
                    options,
                    spline_mode,
                    BlendSpace::Picker,
                    auto_comb_unsolved,
                );
            });
        })
    }

    fn painted_text_center(output: &egui::FullOutput, wanted: &str) -> egui::Pos2 {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == wanted => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{wanted:?} not painted"))
    }

    fn click_comb(spline_mode: SplineMode) -> ZColorPickerOptions {
        let ctx = egui::Context::default();
        let mut options = ZColorPickerOptions::default();
        let output = draw_options_frame(&ctx, Vec::new(), &mut options, spline_mode, 0);
        for events in primary_click(painted_text_center(&output, "Show")) {
            draw_options_frame(&ctx, events, &mut options, spline_mode, 0);
        }
        options
    }

    #[test]
    fn comb_controls_follow_spline_mode() {
        let enabled = |show, scale, auto| CombControlsEnabled { show, scale, auto };
        let hermite = SplineMode::HermiteBezier;
        let cases = [
            (SplineMode::Linear, true, enabled(false, false, false)),
            (SplineMode::Linear, false, enabled(false, false, false)),
            (hermite, true, enabled(true, true, false)),
            (hermite, false, enabled(true, false, false)),
            (SplineMode::Bezier, true, enabled(true, true, true)),
            (SplineMode::Bezier, false, enabled(true, false, true)),
        ];
        for (mode, is_shown, expected) in cases {
            assert_eq!(
                comb_controls_enabled(mode, BlendSpace::Picker, is_shown),
                expected,
                "{mode:?} show={is_shown}"
            );
        }
    }

    #[test]
    fn comb_controls_are_off_in_a_non_picker_blend() {
        let off = CombControlsEnabled {
            show: false,
            scale: false,
            auto: false,
        };
        for blend in BlendSpace::ALL {
            if blend == BlendSpace::Picker {
                continue;
            }
            for mode in SplineMode::ALL {
                for is_shown in [true, false] {
                    assert_eq!(
                        comb_controls_enabled(mode, blend, is_shown),
                        off,
                        "{blend:?} {mode:?} show={is_shown}"
                    );
                }
            }
        }
    }

    #[test]
    fn options_rows_start_with_their_group_labels() {
        let texts = options_painted_texts(true);
        let position = |wanted: &str| texts.iter().position(|text| text == wanted);
        assert_eq!(position("POINTS"), Some(0), "{texts:?}");
        assert!(position("COMB").is_some(), "{texts:?}");
        for moved in ["Space", "Spline", "Blend", "Flip", "COPY AS"] {
            assert_eq!(position(moved), None, "{moved:?}: {texts:?}");
        }
    }

    #[test]
    fn comb_checkbox_toggles_in_curved_modes_only() {
        assert!(ZColorPickerOptions::default().is_show_curvature_comb);
        for (mode, toggles) in [
            (SplineMode::Bezier, true),
            (SplineMode::HermiteBezier, true),
            (SplineMode::Linear, false),
        ] {
            let options = click_comb(mode);
            assert_eq!(options.is_show_curvature_comb, !toggles, "{mode:?}");
        }
    }

    #[test]
    fn options_show_both_insert_sides_in_either_state() {
        for is_insert_right in [true, false] {
            let texts = options_painted_texts(is_insert_right);
            for side in ["left", "right"] {
                assert!(
                    texts.iter().any(|text| text == side),
                    "{side:?} not painted with is_insert_right={is_insert_right}: {texts:?}"
                );
            }
        }
    }

    #[test]
    fn auto_comb_unsolved_count_shows_only_while_auto_comb_runs() {
        let options = |is_auto_comb| ZColorPickerOptions {
            is_auto_comb,
            ..Default::default()
        };
        let cases = [
            (SplineMode::Bezier, true, 2, true),
            (SplineMode::Bezier, true, 0, false),
            (SplineMode::Bezier, false, 2, false),
            (SplineMode::Linear, true, 2, false),
        ];
        for (spline_mode, is_auto_comb, unsolved, is_shown) in cases {
            let texts = painted_option_texts(options(is_auto_comb), spline_mode, unsolved);
            assert!(texts.iter().any(|text| text == "Auto comb"), "{texts:?}");
            assert_eq!(
                texts.iter().any(|text| text == "2 unsolved"),
                is_shown,
                "{spline_mode:?} {is_auto_comb} {unsolved}: {texts:?}"
            );
        }
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
                picker_space: PickerSpace::Hsv,
                blend_space: BlendSpace::Picker,
            },
        ));
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();

        window.save_selected(
            &mut handler,
            &[],
            SplineMode::Bezier,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );

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

        window.save_selected(
            &mut handler,
            &[],
            SplineMode::Bezier,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );

        assert_eq!(window.save_error, None);
        assert!(tmp.path().join("A.json").is_file());
    }

    #[test]
    fn save_selected_with_stale_index_sets_save_error() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &[]);
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();

        window.save_selected(
            &mut handler,
            &[],
            SplineMode::Bezier,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );

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
                    curvature: None,
                })
            })
            .collect()
    }

    #[test]
    fn flip_reverses_points_and_swaps_their_tangents() {
        let original = three_points();
        let mut points = original.clone();

        flip_control_points(&mut points, &mut None);

        assert_eq!(points.len(), original.len());
        for (flipped, orig) in points.iter().zip(original.iter().rev()) {
            assert_eq!(flipped.val(), orig.val());
            assert_eq!(*flipped.t(), 1.0 - *orig.t());
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

            flip_control_points(&mut points, &mut selected);

            assert_eq!(selected, after, "selected before flip: {before:?}");
        }
    }

    #[test]
    fn flip_mirrors_uneven_t_and_keeps_it_increasing() {
        let mut points = three_points();
        *points[1].t_mut() = 0.25;

        flip_control_points(&mut points, &mut None);

        let ts: Vec<f32> = points.iter().map(|cp| *cp.t()).collect();
        assert_eq!(ts, [0.0, 0.75, 1.0]);
    }

    #[test]
    fn flip_twice_is_the_identity() {
        let original = three_points();
        let mut points = original.clone();
        let mut selected = Some(0);

        flip_control_points(&mut points, &mut selected);
        flip_control_points(&mut points, &mut selected);

        assert_eq!(points, original);
        assert_eq!(selected, Some(0));
    }

    fn handler_with_edited_a_and_distinct_b(tmp: &TempDir) -> (PresetHandler, Vec<ControlPoint>) {
        let mut handler = handler_with_presets(tmp, &["A", "B"]);
        handler.presets[1].data = AppDataCtx {
            spline_mode: SplineMode::Bezier,
            control_points: three_points(),
            picker_space: PickerSpace::Hsv,
            blend_space: BlendSpace::Picker,
        };
        handler.preset_selected_index = Some(0);
        (handler, three_points()[..2].to_vec())
    }

    fn pending_switch_to_b(tmp: &TempDir) -> (WindowPreset, PresetHandler, Vec<ControlPoint>) {
        let (mut handler, mut points) = handler_with_edited_a_and_distinct_b(tmp);
        let mut window = WindowPreset::new();
        let applied = window.choose_preset(
            1,
            &mut handler,
            &mut points,
            &mut SplineMode::Linear,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );
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

        let applied = window.choose_preset(
            1,
            &mut handler,
            &mut points,
            &mut mode,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );

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
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
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
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
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
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
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

    fn unwritable_handler_with_a_and_b(tmp: &TempDir) -> PresetHandler {
        let not_a_dir = tmp.path().join("presets");
        std::fs::write(&not_a_dir, "").unwrap();
        let mut handler = PresetHandler::new(not_a_dir);
        for name in ["A", "B"] {
            handler.presets.push(PresetEntity::new(
                name,
                AppDataCtx {
                    spline_mode: SplineMode::Linear,
                    control_points: Vec::new(),
                    picker_space: PickerSpace::Hsv,
                    blend_space: BlendSpace::Picker,
                },
            ));
        }
        handler.preset_selected_index = Some(0);
        handler
    }

    #[test]
    fn failed_save_and_switch_keeps_the_modal_open() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a_and_b(&tmp);
        let mut window = WindowPreset::new();
        let edited = three_points();
        let mut points = edited.clone();
        let mut mode = SplineMode::Linear;
        assert!(window
            .choose_preset(
                1,
                &mut handler,
                &mut points,
                &mut mode,
                &mut PickerSpace::Hsv,
                &mut BlendSpace::Picker
            )
            .is_none());

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::SaveAndSwitch,
            &mut handler,
            &mut points,
            &mut mode,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );

        assert!(applied.is_none());
        assert!(window.save_error.is_some());
        assert_eq!(window.pending_switch, Some(1));
        assert_eq!(handler.preset_selected_index, Some(0));
        assert_eq!(points, edited);
        assert!(has_unsaved_changes(
            &handler,
            &points,
            mode,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));
    }

    #[test]
    fn failed_auto_save_keeps_the_switch_confirm() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a_and_b(&tmp);
        handler.auto_save_presets = true;
        let mut window = WindowPreset::new();
        let edited = three_points();
        let mut points = edited.clone();
        let mut mode = SplineMode::Linear;
        assert!(handler
            .auto_save(&points, mode, PickerSpace::Hsv, BlendSpace::Picker)
            .is_err());

        assert!(has_unsaved_changes(
            &handler,
            &points,
            mode,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));
        assert!(window
            .choose_preset(
                1,
                &mut handler,
                &mut points,
                &mut mode,
                &mut PickerSpace::Hsv,
                &mut BlendSpace::Picker
            )
            .is_none());
        assert_eq!(window.pending_switch, Some(1));
        assert_eq!(points, edited);
    }

    #[test]
    fn failed_manual_save_keeps_the_preset_dirty() {
        let tmp = TempDir::new().unwrap();
        let mut handler = unwritable_handler_with_a_and_b(&tmp);
        let before = handler.presets[0].data.clone();
        let mut window = WindowPreset::new();

        window.save_selected(
            &mut handler,
            &three_points(),
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );

        assert!(window.save_error.is_some());
        assert_eq!(handler.presets[0].data, before);
        assert!(has_unsaved_changes(
            &handler,
            &three_points(),
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));
    }

    #[test]
    fn clean_auto_saved_or_unselected_choose_applies_immediately() {
        let cases = [
            ("clean", false, Some(0), Vec::new()),
            ("auto-save", true, Some(0), three_points()[..2].to_vec()),
            ("no selection", false, None, Vec::new()),
        ];
        for (case, auto_save, selected, mut points) in cases {
            let tmp = TempDir::new().unwrap();
            let (mut handler, _) = handler_with_edited_a_and_distinct_b(&tmp);
            handler.auto_save_presets = auto_save;
            handler.preset_selected_index = selected;
            let mut window = WindowPreset::new();
            let mut mode = SplineMode::Linear;

            let applied = window.choose_preset(
                1,
                &mut handler,
                &mut points,
                &mut mode,
                &mut PickerSpace::Hsv,
                &mut BlendSpace::Picker,
            );

            assert_eq!(applied.map(|p| p.name), Some("B".to_string()), "{case}");
            assert_eq!(points, three_points(), "{case}");
            assert_eq!(mode, SplineMode::Bezier, "{case}");
            assert_eq!(handler.preset_selected_index, Some(1), "{case}");
            assert_eq!(window.pending_switch, None, "{case}");
        }
    }

    #[test]
    fn unselected_points_saved_nowhere_defer_the_switch() {
        let tmp = TempDir::new().unwrap();
        let (mut handler, edited) = handler_with_edited_a_and_distinct_b(&tmp);
        handler.preset_selected_index = None;
        let mut window = WindowPreset::new();
        let mut points = edited.clone();
        let mut mode = SplineMode::Linear;

        let applied = window.choose_preset(
            1,
            &mut handler,
            &mut points,
            &mut mode,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );

        assert!(applied.is_none());
        assert_eq!(window.pending_switch, Some(1));
        assert_eq!(points, edited);
        assert_eq!(mode, SplineMode::Linear);
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn unselected_points_saved_in_a_preset_or_empty_apply_immediately() {
        let cases = [
            (
                "saved in A",
                three_points()[..2].to_vec(),
                SplineMode::Linear,
            ),
            ("empty", Vec::new(), SplineMode::Bezier),
        ];
        for (case, mut points, mut mode) in cases {
            let tmp = TempDir::new().unwrap();
            let (mut handler, edited) = handler_with_edited_a_and_distinct_b(&tmp);
            handler.presets[0].data = AppDataCtx {
                spline_mode: SplineMode::Linear,
                control_points: edited,
                picker_space: PickerSpace::Hsv,
                blend_space: BlendSpace::Picker,
            };
            handler.preset_selected_index = None;
            let mut window = WindowPreset::new();

            let applied = window.choose_preset(
                1,
                &mut handler,
                &mut points,
                &mut mode,
                &mut PickerSpace::Hsv,
                &mut BlendSpace::Picker,
            );

            assert_eq!(applied.map(|p| p.name), Some("B".to_string()), "{case}");
            assert_eq!(points, three_points(), "{case}");
            assert_eq!(mode, SplineMode::Bezier, "{case}");
            assert_eq!(handler.preset_selected_index, Some(1), "{case}");
            assert_eq!(window.pending_switch, None, "{case}");
        }
    }

    fn unselected_pending_switch_to_b(
        tmp: &TempDir,
    ) -> (WindowPreset, PresetHandler, Vec<ControlPoint>) {
        let (mut handler, mut points) = handler_with_edited_a_and_distinct_b(tmp);
        handler.preset_selected_index = None;
        let mut window = WindowPreset::new();
        let applied = window.choose_preset(
            1,
            &mut handler,
            &mut points,
            &mut SplineMode::Linear,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );
        assert!(applied.is_none());
        (window, handler, points)
    }

    #[test]
    fn discard_without_selection_applies_the_pending_target() {
        let tmp = TempDir::new().unwrap();
        let (mut window, mut handler, mut points) = unselected_pending_switch_to_b(&tmp);
        let mut mode = SplineMode::Linear;

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::Discard,
            &mut handler,
            &mut points,
            &mut mode,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );

        assert_eq!(applied.map(|p| p.name), Some("B".to_string()));
        assert_eq!(points, three_points());
        assert_eq!(mode, SplineMode::Bezier);
        assert_eq!(handler.preset_selected_index, Some(1));
        assert_eq!(window.pending_switch, None);
    }

    #[test]
    fn cancel_without_selection_keeps_the_points_unselected() {
        let tmp = TempDir::new().unwrap();
        let (mut window, mut handler, mut points) = unselected_pending_switch_to_b(&tmp);
        let edited = points.clone();
        let mut mode = SplineMode::Linear;

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::Cancel,
            &mut handler,
            &mut points,
            &mut mode,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );

        assert!(applied.is_none());
        assert_eq!(points, edited);
        assert_eq!(mode, SplineMode::Linear);
        assert_eq!(handler.preset_selected_index, None);
        assert_eq!(window.pending_switch, None);
        assert!(!window.is_modal_open());
    }

    #[test]
    fn switch_pending_without_selection_keeps_the_modal_open_across_frames() {
        let tmp = TempDir::new().unwrap();
        let (mut window, mut handler, _) = unselected_pending_switch_to_b(&tmp);
        let ctx = egui::Context::default();

        draw_frame(&ctx, &mut window, &mut handler);

        assert!(window.is_modal_open());
        assert_eq!(window.pending_switch, Some(1));
    }

    #[test]
    fn unsaved_changes_follow_edits_saves_and_auto_save() {
        let tmp = TempDir::new().unwrap();
        let (mut handler, edited) = handler_with_edited_a_and_distinct_b(&tmp);
        let mut window = WindowPreset::new();

        assert!(!has_unsaved_changes(
            &handler,
            &[],
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));
        assert!(has_unsaved_changes(
            &handler,
            &edited,
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));

        window.save_selected(
            &mut handler,
            &edited,
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );
        assert!(!has_unsaved_changes(
            &handler,
            &edited,
            SplineMode::Linear,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));

        handler.auto_save_presets = true;
        assert!(!has_unsaved_changes(
            &handler,
            &[],
            SplineMode::Bezier,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));
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

    #[test]
    fn auto_save_hint_depends_on_the_selection() {
        assert_eq!(
            auto_save_hint(true),
            "Rewrite the selected preset file after every edit"
        );
        assert_eq!(auto_save_hint(false), "Select a preset to auto-save into");
    }

    #[test]
    fn no_presets_text_names_the_folder() {
        let dir = Path::new("some").join("presets");

        let text = no_presets_text(&dir);

        assert!(text.starts_with("No presets in "), "{text:?}");
        assert!(text.contains(&dir.display().to_string()), "{text:?}");
    }

    #[test]
    fn draw_without_selection_keeps_rename_text_empty() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A"]);
        let mut window = WindowPreset::new();
        let ctx = egui::Context::default();

        draw_frame(&ctx, &mut window, &mut handler);
        draw_frame(&ctx, &mut window, &mut handler);

        let rename_text = ctx.data(|d| d.get_temp::<String>(egui::Id::new(819181)));
        assert_eq!(rename_text.unwrap_or_default(), "");
        assert_eq!(handler.preset_selected_index, None);
    }

    #[test]
    fn open_create_window_resets_the_create_state() {
        let mut window = WindowPreset::new();
        window.new_preset_window_text = "stale".to_string();
        window.create_error = Some("Name already exists".to_string());

        window.open_create_window();

        assert!(window.new_preset_is_open);
        assert_eq!(window.new_preset_window_text, "");
        assert_eq!(window.create_error, None);
    }

    #[test]
    fn save_clicked_without_selection_opens_create_window() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A"]);
        let mut window = WindowPreset::new();

        window.on_save_clicked(
            &mut handler,
            &three_points(),
            SplineMode::Bezier,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );

        assert!(window.new_preset_is_open);
        assert_eq!(window.save_error, None);
        assert_eq!(handler.preset_selected_index, None);
        assert!(std::fs::read_dir(tmp.path()).unwrap().next().is_none());
    }

    #[test]
    fn save_clicked_with_selection_saves() {
        let tmp = TempDir::new().unwrap();
        let mut handler = handler_with_presets(&tmp, &["A"]);
        handler.preset_selected_index = Some(0);
        let mut window = WindowPreset::new();

        window.on_save_clicked(
            &mut handler,
            &three_points(),
            SplineMode::Bezier,
            PickerSpace::Hsv,
            BlendSpace::Picker,
        );

        let saved: AppDataCtx =
            serde_json::from_str(&std::fs::read_to_string(tmp.path().join("A.json")).unwrap())
                .unwrap();
        assert_eq!(saved.control_points, three_points());
        assert_eq!(saved.spline_mode, SplineMode::Bezier);
        assert!(!window.new_preset_is_open);
        assert_eq!(window.save_error, None);
    }

    fn button_actions(buttons: &[(&str, PendingSwitchAction)]) -> Vec<PendingSwitchAction> {
        buttons.iter().map(|&(_, action)| action).collect()
    }

    #[test]
    fn re_picking_the_selected_preset_offers_save_and_revert() {
        let buttons = pending_switch_buttons(true, true);

        assert_eq!(
            button_actions(buttons),
            [
                PendingSwitchAction::SaveAndSwitch,
                PendingSwitchAction::Discard,
                PendingSwitchAction::Cancel
            ]
        );
        assert!(buttons.iter().any(|(label, _)| label.contains("Revert")));
        assert!(buttons
            .iter()
            .all(|(label, _)| !label.to_lowercase().contains("switch")));
    }

    #[test]
    fn picking_another_preset_offers_save_and_switch() {
        let buttons = pending_switch_buttons(true, false);

        assert!(buttons
            .iter()
            .any(|(label, _)| label.contains("Save and switch")));
    }

    #[test]
    fn pending_switch_without_selection_offers_discard_and_cancel() {
        assert_eq!(
            button_actions(pending_switch_buttons(false, false)),
            [PendingSwitchAction::Discard, PendingSwitchAction::Cancel]
        );
    }

    #[test]
    fn re_picking_the_dirty_selected_preset_then_discard_reverts_to_saved() {
        let tmp = TempDir::new().unwrap();
        let (mut handler, mut points) = handler_with_edited_a_and_distinct_b(&tmp);
        let saved = handler.presets[0].data.clone();
        let mut window = WindowPreset::new();
        let mut mode = SplineMode::Linear;
        assert!(has_unsaved_changes(
            &handler,
            &points,
            mode,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));

        assert!(window
            .choose_preset(
                0,
                &mut handler,
                &mut points,
                &mut mode,
                &mut PickerSpace::Hsv,
                &mut BlendSpace::Picker
            )
            .is_none());
        assert_eq!(window.pending_switch, Some(0));

        let applied = window.resolve_pending_switch(
            PendingSwitchAction::Discard,
            &mut handler,
            &mut points,
            &mut mode,
            &mut PickerSpace::Hsv,
            &mut BlendSpace::Picker,
        );

        assert_eq!(applied.map(|p| p.name), Some("A".to_string()));
        assert_eq!(points, saved.control_points);
        assert_eq!(mode, saved.spline_mode);
        assert_eq!(handler.preset_selected_index, Some(0));
        assert!(!has_unsaved_changes(
            &handler,
            &points,
            mode,
            PickerSpace::Hsv,
            BlendSpace::Picker
        ));
    }
}
