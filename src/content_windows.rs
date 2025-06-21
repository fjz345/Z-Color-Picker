use eframe::egui;

use crate::app::ZColorPickerOptions;
use crate::common::ColorStringCopy;
use crate::common::SplineMode;
use crate::control_point::ControlPoint;
use crate::egui::InnerResponse;
use crate::egui::PointerButton;
use crate::egui::Ui;
use crate::egui::Window;
use crate::preset::Preset;
use crate::preset::PresetData;
use crate::{egui::Pos2, ui_common::ContentWindow};

pub struct WindowZColorPickerOptionsDrawResult {
    pub preset_result: PresetDrawResult,
}

impl Default for WindowZColorPickerOptionsDrawResult {
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

pub struct PresetDrawResult {
    pub should_apply: Option<Preset>,
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
    pub position: Pos2,
    pub new_preset_is_open: bool,
    pub new_preset_window_text: String,
}

impl WindowZColorPickerOptions {
    pub fn new(window_position: Pos2) -> Self {
        Self {
            open: false,
            position: window_position,
            new_preset_window_text: String::new(),
            new_preset_is_open: false,
        }
    }

    pub fn update(&mut self) {}

    pub fn draw_content(
        &mut self,
        ui: &mut Ui,
        options: &mut ZColorPickerOptions,
        control_points: &mut Vec<ControlPoint>,
        color_copy_format: &mut ColorStringCopy,
    ) -> WindowZColorPickerOptionsDrawResult {
        let mut draw_result = WindowZColorPickerOptionsDrawResult::default();

        ui.horizontal(|ui| {
            ui.checkbox(&mut options.is_curve_locked, "🔒")
                .on_hover_text("Apply changes to all control points");
            ui.checkbox(&mut options.is_hue_middle_interpolated, "🎨")
                .on_hover_text("Only modify first/last control points");
            const INSERT_RIGHT_UNICODE: &str = "👉";
            const INSERT_LEFT_UNICODE: &str = "👈";
            let insert_mode_unicode = if options.is_insert_right {
                INSERT_RIGHT_UNICODE
            } else {
                INSERT_LEFT_UNICODE
            };
            ui.checkbox(&mut options.is_insert_right, insert_mode_unicode)
                .on_hover_text(format!(
                    "Insert new points in {} direction",
                    insert_mode_unicode
                ));
            ui.checkbox(&mut options.is_window_lock, "🆘")
                .on_hover_text("Clamps the control points so they are contained");
        });

        ui.horizontal(|ui| {
            egui::ComboBox::new(12312312, "")
                .selected_text(format!("{:?}", *color_copy_format))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    ui.selectable_value(color_copy_format, ColorStringCopy::HEX, "Hex");
                    ui.selectable_value(color_copy_format, ColorStringCopy::HEXNOA, "Hex(no A)");
                })
                .response
                .on_hover_text("Color Copy Format");

            egui::ComboBox::new(12312313, "")
                .selected_text(format!("{:?}", options.spline_mode))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    ui.selectable_value(&mut options.spline_mode, SplineMode::Linear, "Linear");
                    ui.selectable_value(&mut options.spline_mode, SplineMode::Bezier, "Bezier");
                    ui.selectable_value(
                        &mut options.spline_mode,
                        SplineMode::HermiteBezier,
                        "Hermite",
                    );
                    // TODO: enable Polynomial combo box
                    // ui.selectable_value(
                    //     &mut self.spline_mode,
                    //     SplineMode::Polynomial,
                    //     "Polynomial(Crash)",
                    // );
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

        ui.horizontal(|ui| {
            let combobox_selected_text_to_show = match options.preset_selected_index {
                Some(i) => options.presets[i.clamp(0, options.presets.len() - 1)]
                    .name
                    .to_string(),
                None => "".to_string(),
            };

            let mut combobox_selected_index = 0;
            let mut combobox_has_selected = false;
            let _combobox_response = egui::ComboBox::new(1232313, "")
                .selected_text(combobox_selected_text_to_show)
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);

                    for (i, preset) in &mut options.presets.iter().enumerate() {
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
                        options.preset_selected_index = None;
                    }
                })
                .response
                .on_hover_text("Presets");

            if combobox_has_selected {
                if combobox_selected_index == 0 {
                    self.new_preset_is_open = true;
                    self.new_preset_window_text.clear();
                    log::info!("Selected New Preset");
                } else {
                    options.preset_selected_index = Some(combobox_selected_index - 1);
                    if let Some(s) = options.preset_selected_index {
                        draw_result.preset_result.should_apply = Some(options.presets[s].clone());
                        log::info!("Selected Preset {:?}", combobox_selected_index - 1);
                    }
                }
            };

            if ui.button("Save").clicked_by(PointerButton::Primary) {
                if let Some(s) = options.preset_selected_index {
                    options.presets[s].data.spline_mode = options.spline_mode;
                    options.presets[s].data.control_points = control_points.to_vec();
                    log::info!("Saved preset [{}]", options.presets[s].name);
                } else {
                    log::info!("Could not save, no preset selected");
                }
            }
            if ui.button("Delete").clicked_by(PointerButton::Primary) {
                if let Some(s) = options.preset_selected_index {
                    options.presets.remove(s);
                    options.preset_selected_index = None;
                }
            }
        });

        let mut create_preset_open = self.new_preset_is_open;
        let mut create_preset_create_clicked = false;
        if self.new_preset_is_open {
            egui::Window::new("Create Preset")
                .open(&mut create_preset_open)
                .show(ui.ctx(), |ui| {
                    let _text_response = ui.text_edit_singleline(&mut self.new_preset_window_text);

                    if ui.button("Create").clicked() {
                        self.new_preset_is_open = false;
                        create_preset_create_clicked = true;

                        let new_preset: Preset = Preset {
                            name: self.new_preset_window_text.clone(),
                            data: PresetData {
                                spline_mode: options.spline_mode,
                                control_points: control_points.to_vec(),
                            },
                        };
                        options.presets.push(new_preset);
                    }
                });

            if create_preset_create_clicked {
                create_preset_open = false;
            }
            self.new_preset_is_open = create_preset_open;
        }
        draw_result
    }

    fn draw_ui(
        &mut self,
        ui: &mut Ui,
        options: &mut ZColorPickerOptions,
        control_points: &mut Vec<ControlPoint>,
        color_copy_format: &mut ColorStringCopy,
    ) -> Option<InnerResponse<Option<WindowZColorPickerOptionsDrawResult>>> {
        let prev_visuals = ui.visuals_mut().clone();

        let mut open = self.is_open();
        let response = Window::new(self.title())
            .resizable(false)
            .title_bar(false)
            .open(&mut open)
            .auto_sized()
            .show(ui.ctx(), |ui: &mut Ui| {
                self.draw_content(ui, options, control_points, color_copy_format)
            });

        if open {
            self.open();
        } else {
            self.close();
        }

        ui.ctx().set_visuals(prev_visuals);

        response
    }
}
