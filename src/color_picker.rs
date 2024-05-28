use crate::{
    control_point::{
        create_tangent_for_control_point, ControlPoint, ControlPointStorage, ControlPointTangent,
        ControlPointType,
    },
    curves::Bezier,
    error::{Result, ZError},
    hsv_key_value::HsvKeyValue,
};
use eframe::{
    egui::{
        self,
        color_picker::{show_color, Alpha},
        remap_clamp, Layout, PointerButton, Pos2, Response, Ui,
    },
    epaint::{vec2, Color32, HsvaGamma, Vec2},
};
use serde::{Deserialize, Serialize};

use crate::{
    curves::{ui_ordered_control_points, ui_ordered_spline_gradient},
    math::hue_lerp,
    preset::{
        delete_preset_from_disk, get_preset_save_path, load_presets, save_preset_to_disk, Preset,
        PresetData,
    },
    ui_common::{
        color_slider_1d, color_slider_2d, color_text_ui, response_copy_color_on_click,
        ui_hue_control_points_overlay,
    },
};

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ColorStringCopy {
    HEX,
    HEXNOA,
    SRGBHEX,
    HSV,
    HSVA,
    INT,
    FLOAT,
    RGB,
    SRGB,
    RGBA,
    SRGBA,
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum SplineMode {
    Linear,
    Bezier,
    HermiteBezier,
    Polynomial,
}

pub struct ZColorPicker {
    pub control_points: Vec<ControlPoint>,
    pub last_modifying_point_index: Option<usize>,
    pub is_curve_locked: bool,
    pub is_hue_middle_interpolated: bool,
    pub is_insert_right: bool,
    pub is_window_lock: bool,
    pub spline_mode: SplineMode,
    pub presets: Vec<Preset>,
    pub preset_selected_index: Option<usize>,
    pub new_preset_window_open: bool,
    pub new_preset_window_text: String,
    pub dragging_bezier_index: Option<usize>,
    pub control_point_right_clicked: Option<usize>,
}

impl ZColorPicker {
    pub fn new() -> Self {
        let mut new_color_picker = Self {
            control_points: Vec::with_capacity(4),
            last_modifying_point_index: None,
            is_curve_locked: false,
            is_hue_middle_interpolated: true,
            is_insert_right: true,
            is_window_lock: true,
            spline_mode: SplineMode::HermiteBezier,
            presets: Vec::new(),
            preset_selected_index: None,
            new_preset_window_open: false,
            new_preset_window_text: String::new(),
            dragging_bezier_index: None,
            control_point_right_clicked: None,
        };

        const LAZY_TANGENT_DELTA: f32 = 0.01;
        const DEFAULT_STARTUP_CONTROL_POINTS: [ControlPoint; 4] = [
            ControlPoint::ControlPointSimple(ControlPointStorage {
                val: ControlPointType {
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
                val: ControlPointType {
                    val: [0.44, 0.38, 0.1],
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
            ControlPoint::ControlPointSimple(ControlPointStorage {
                val: ControlPointType {
                    val: [0.8, 0.6, 0.1],
                },
                t: 2.0,
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
                val: ControlPointType {
                    val: [0.9, 0.8, 0.2],
                },
                t: 3.0,
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

        let r = load_presets(&mut new_color_picker.presets);
        if let Err(e) = r {
            println!("{}", e);
        }

        // Use first as default if exists
        if new_color_picker.presets.len() >= 1 {
            new_color_picker.preset_selected_index = Some(0);
            new_color_picker.apply_selected_preset();
        } else {
            for control_point in &DEFAULT_STARTUP_CONTROL_POINTS {
                new_color_picker.spawn_control_point(*control_point.val(), *control_point.t());
            }
        }

        new_color_picker
    }

    fn remove_all_control_points(&mut self) {
        for i in (0..self.control_points.len()).rev() {
            self.remove_control_point(i);
        }
        self.last_modifying_point_index = None;
        self.dragging_bezier_index = None;
    }

    fn apply_preset(&mut self, preset: Preset) {
        self.remove_all_control_points();
        for preset_control_point in preset.data.control_points {
            self.spawn_control_point(*preset_control_point.val(), *preset_control_point.t());
        }
        self.spline_mode = preset.data.spline_mode;
    }

    pub fn apply_selected_preset(&mut self) {
        if let Some(s) = self.preset_selected_index {
            if s < self.presets.len() {
                let preset_to_apply = self.presets[s].clone();
                self.apply_preset(preset_to_apply);
            }
        }
    }

    pub fn save_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.preset_selected_index {
            let preset = &mut self.presets[s];
            preset.data = PresetData {
                spline_mode: self.spline_mode,
                control_points: self.control_points.clone(),
            };
            save_preset_to_disk(&preset.clone())?;

            return Ok(());
        }

        Err(ZError::Message(
            "Preset Save failed, No preset selected".to_string(),
        ))
    }

    pub fn preset_data_from_current_state(&self) -> PresetData {
        PresetData {
            spline_mode: self.spline_mode,
            control_points: self.control_points.clone(),
        }
    }

    pub fn create_preset(&mut self, name: &String) -> Result<()> {
        let preset = Preset::new(name, self.preset_data_from_current_state());
        let index = self.presets.len();
        self.presets.push(preset);

        self.preset_selected_index = Some(index);
        self.save_selected_preset()?;

        Ok(())
    }

    pub fn delete_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.preset_selected_index {
            let preset_to_remove = self.presets.remove(s);
            delete_preset_from_disk(&get_preset_save_path(&preset_to_remove))?;
            self.preset_selected_index = None;

            return Ok(());
        }

        Err(ZError::Message(
            "Selected Preset Delete failed, No preset selected".to_string(),
        ))
    }

    pub fn remove_control_point(&mut self, index: usize) {
        self.control_points.remove(index);
        println!(
            "CP {} removed, new len {}",
            index,
            self.control_points.len()
        );
        if self.control_points.len() > 1 {
            self.last_modifying_point_index = Some(index.max(1) - 1);
            self.dragging_bezier_index = Some(index.max(1) - 1);
        } else {
            self.dragging_bezier_index = None;
            self.last_modifying_point_index = None;
        }
    }

    pub fn spawn_control_point(&mut self, color: ControlPointType, t: f32) {
        let control_point_pivot = self.last_modifying_point_index;

        let new_index = match control_point_pivot {
            Some(index) => {
                if self.is_insert_right {
                    index + 1
                } else {
                    index
                }
            }
            None => {
                if self.control_points.len() <= 0 {
                    0
                } else {
                    if self.is_insert_right {
                        self.control_points.len()
                    } else {
                        0
                    }
                }
            }
        };

        self.dragging_bezier_index = None;
        let mut new_cp = ControlPoint::new_simple(color, t);
        self.control_points.insert(new_index, new_cp);
        // Adding keys messes with the indicies
        self.last_modifying_point_index = Some(new_index);

        println!(
            "ControlPoint#{} spawned @[{}]{},{},{}",
            self.control_points.len(),
            t,
            color.val[0],
            color.val[1],
            color.val[2],
        );
    }

    pub fn get_control_points_sdf_2d(&self, xy: Pos2) -> Option<(&ControlPoint, f32)> {
        let mut closest_dist: Option<f32> = None;
        let mut closest_cp: Option<&ControlPoint> = None;
        for cp in self.control_points.iter() {
            let pos_2d = Pos2::new(
                cp.val()[0].clamp(0.0, 1.0),
                1.0 - cp.val()[1].clamp(0.0, 1.0),
            );
            let distance_2d = pos_2d.distance(xy);

            match closest_dist {
                Some(closest_dist_2d) => {
                    if distance_2d < closest_dist_2d {
                        closest_cp = Some(cp);
                        closest_dist = Some(distance_2d);
                    }
                }
                None => {
                    closest_cp = Some(cp);
                    closest_dist = Some(distance_2d);
                }
            };
        }

        match closest_dist {
            Some(closest_dist_2d) => {
                let dist = closest_dist_2d;
                println!("Closest Dist: {}", dist);
                Some((closest_cp.unwrap(), dist))
            }
            None => {
                println!("Did not find closest dist");
                None
            }
        }
    }

    pub fn pre_draw_update(&mut self) {
        if self.spline_mode == SplineMode::Bezier {
            // Force init tangents
            for control_point in &mut self.control_points {
                let clone = control_point.clone();
                for tang in &mut control_point.tangents_mut().iter_mut() {
                    if tang.is_none() {
                        *tang = Some(create_tangent_for_control_point(&clone));
                    }
                }
            }
        }
    }

    pub fn post_update_control_points(&mut self) {
        if self.is_hue_middle_interpolated {
            let num_points = self.control_points.len();
            if num_points >= 2 {
                let points = &mut self.control_points[..];

                let first_index = 0;
                let last_index = points.len() - 1;
                let first_hue = points[first_index].val()[2];
                let last_hue: f32 = points[last_index].val()[2];

                for i in 1..last_index {
                    let t = (i as f32) / (points.len() - 1) as f32;
                    let hue = hue_lerp(first_hue, last_hue, t);
                    points[i].val_mut()[2] = hue;
                }
            }
        }

        if self.is_window_lock {
            for i in 0..self.control_points.len() {
                let cp = &mut self.control_points[i];
                cp.val_mut()[0] = cp.val()[0].clamp(0.0, 1.0);
                cp.val_mut()[1] = cp.val()[1].clamp(0.0, 1.0);
                cp.val_mut()[2] = cp.val()[2].clamp(0.0, 1.0);
            }
        }

        match self.control_point_right_clicked {
            Some(index) => {
                self.remove_control_point(index);
            }
            _ => {}
        }
    }

    pub fn draw_ui(
        &mut self,
        ui: &mut Ui,
        mut color_copy_format: &mut ColorStringCopy,
    ) -> Response {
        let inner_response = ui.vertical(|ui| {
            self.pre_draw_update();

            let response = main_color_picker(
                ui,
                &mut self.control_points[..],
                self.spline_mode,
                *color_copy_format,
                &mut self.last_modifying_point_index,
                &mut self.dragging_bezier_index,
                &mut self.control_point_right_clicked,
                self.is_hue_middle_interpolated,
                self.is_curve_locked,
            );

            self.post_update_control_points();

            self.draw_ui_main_options(ui, &mut color_copy_format);

            response
        });

        inner_response.inner
    }

    pub fn draw_ui_main_options(&mut self, ui: &mut Ui, color_copy_format: &mut ColorStringCopy) {
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.is_curve_locked, "🔒")
                .on_hover_text("Apply changes to all control points");
            ui.checkbox(&mut self.is_hue_middle_interpolated, "🎨")
                .on_hover_text("Only modify first/last control points");
            const INSERT_RIGHT_UNICODE: &str = "👉";
            const INSERT_LEFT_UNICODE: &str = "👈";
            let insert_mode_unicode = if self.is_insert_right {
                INSERT_RIGHT_UNICODE
            } else {
                INSERT_LEFT_UNICODE
            };
            ui.checkbox(&mut self.is_insert_right, insert_mode_unicode)
                .on_hover_text(format!(
                    "Insert new points in {} direction",
                    insert_mode_unicode
                ));
            ui.checkbox(&mut self.is_window_lock, "🆘")
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
                .selected_text(format!("{:?}", self.spline_mode))
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);
                    ui.selectable_value(&mut self.spline_mode, SplineMode::Linear, "Linear");
                    ui.selectable_value(
                        &mut self.spline_mode,
                        SplineMode::Bezier,
                        "Bezier(Bugged)",
                    );
                    ui.selectable_value(
                        &mut self.spline_mode,
                        SplineMode::HermiteBezier,
                        "Hermite(NYI)",
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
                self.control_points.reverse();
            }
        });

        ui.horizontal(|ui| {
            let combobox_selected_text_to_show = match self.preset_selected_index {
                Some(i) => self.presets[i].name.to_string(),
                None => "".to_string(),
            };

            let mut combobox_selected_index = 0;
            let mut combobox_has_selected = false;
            let _combobox_response = egui::ComboBox::new(1232313, "")
                .selected_text(combobox_selected_text_to_show)
                .show_ui(ui, |ui| {
                    ui.set_min_width(60.0);

                    for (i, preset) in &mut self.presets.iter().enumerate() {
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
                        self.preset_selected_index = None;
                    }
                })
                .response
                .on_hover_text("Presets");

            if combobox_has_selected {
                if combobox_selected_index == 0 {
                    self.new_preset_window_open = true;
                    self.new_preset_window_text.clear();
                    println!("New Preset");
                } else {
                    self.preset_selected_index = Some(combobox_selected_index - 1);
                    self.apply_selected_preset();
                    println!("Selected Preset {:?}", combobox_selected_index - 1);
                }
            };

            if ui.button("Save").clicked_by(PointerButton::Primary) {
                if let Some(_s) = self.preset_selected_index {
                    let r = self.save_selected_preset();
                    match r {
                        Ok(_) => println!("Sucessfully Saved"),
                        Err(e) => println!("{}", e),
                    }
                } else {
                    println!("Could not save, no preset selected");
                }
            }
            if ui.button("Delete").clicked_by(PointerButton::Primary) {
                let r = self.delete_selected_preset();
                match r {
                    Ok(_) => println!("Sucessfully Deleted"),
                    Err(e) => println!("{}", e),
                }
            }
        });

        let mut create_preset_open = self.new_preset_window_open;
        let mut create_preset_create_clicked = false;
        if self.new_preset_window_open {
            egui::Window::new("Create Preset")
                .open(&mut create_preset_open)
                .show(ui.ctx(), |ui| {
                    let _text_response = ui.text_edit_singleline(&mut self.new_preset_window_text);

                    if ui.button("Create").clicked() {
                        self.new_preset_window_open = false;
                        create_preset_create_clicked = true;

                        let r = self.create_preset(&self.new_preset_window_text.clone());
                        match r {
                            Ok(_) => println!("Sucessfully Created"),
                            Err(e) => println!("{}", e),
                        }
                    }
                });
            self.new_preset_window_open = create_preset_open;
            if create_preset_create_clicked {
                self.new_preset_window_open = false;
            }
        }
    }
}

impl Default for ZColorPicker {
    fn default() -> Self {
        Self::new()
    }
}

pub fn format_color_as(
    color: Color32,
    format_type: ColorStringCopy,
    no_alpha: Option<bool>,
) -> String {
    let formatted = match format_type {
        ColorStringCopy::HEX => match no_alpha {
            Some(no_alpha) => {
                if no_alpha {
                    format!("{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
                } else {
                    format!(
                        "{:02x}{:02x}{:02x}{:02x}",
                        color.a(),
                        color.r(),
                        color.g(),
                        color.b()
                    )
                }
            }
            _ => {
                format!(
                    "{:02x}{:02x}{:02x}{:02x}",
                    color.a(),
                    color.r(),
                    color.g(),
                    color.b()
                )
            }
        },
        ColorStringCopy::HEXNOA => {
            format!("{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
        }
        _ => {
            println!("Not Implemented {:?}", format_type);
            format!("rgb({}, {}, {})", color.r(), color.g(), color.b())
        }
    };
    formatted.to_uppercase()
}

pub fn main_color_picker(
    ui: &mut Ui,
    control_points: &mut [ControlPoint],
    spline_mode: SplineMode,
    color_copy_format: ColorStringCopy,
    last_modifying_point_index: &mut Option<usize>,
    dragging_bezier_index: &mut Option<usize>,
    control_point_right_clicked: &mut Option<usize>,
    is_hue_middle_interpolated: bool,
    is_curve_locked: bool,
) -> Response {
    let num_control_points = control_points.len();
    if let Some(last_modified_index) = *last_modifying_point_index {
        if num_control_points == 0 {
            *last_modifying_point_index = None;
        } else {
            *last_modifying_point_index =
                Some(last_modified_index.clamp(0, num_control_points - 1));
        }
    }

    let main_color_picker_response = ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let desired_size_slider_2d = Vec2::splat(ui.spacing().slider_width);

        let mut is_modifying_index: Option<usize> =
            dragging_bezier_index.or(*last_modifying_point_index);

        let modifying_control_point = match is_modifying_index {
            Some(index) => control_points.get_mut(index),
            None => None,
        };

        let dummy_color = HsvaGamma {
            h: 0.0,
            s: 0.0,
            v: 0.0,
            a: 1.0,
        };
        let mut color_to_show = match modifying_control_point.as_ref() {
            Some(cp) => cp.val().hsv(),
            None => dummy_color,
        };

        let current_color_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
        let response: Response =
            show_color(ui, color_to_show, current_color_size).on_hover_text("Selected color");
        response_copy_color_on_click(
            ui,
            &response,
            color_to_show,
            color_copy_format,
            PointerButton::Middle,
        );

        let alpha = Alpha::Opaque;
        color_text_ui(ui, color_to_show, alpha, color_copy_format);

        if alpha == Alpha::BlendOrAdditive {
            // We signal additive blending by storing a negative alpha (a bit ironic).
            let a = &mut color_to_show.a;
            let mut additive = *a < 0.0;
            ui.horizontal(|ui| {
                ui.label("Blending:");
                ui.radio_value(&mut additive, false, "Normal");
                ui.radio_value(&mut additive, true, "Additive");

                if additive {
                    *a = -a.abs();
                }

                if !additive {
                    *a = a.abs();
                }
            });
        }

        let additive = color_to_show.a < 0.0;

        let opaque = HsvaGamma {
            a: 1.0,
            ..color_to_show
        };

        if alpha == Alpha::Opaque {
            color_to_show.a = 1.0;
        } else {
            let a = &mut color_to_show.a;

            if alpha == Alpha::OnlyBlend {
                if *a < 0.0 {
                    *a = 0.5; // was additive, but isn't allowed to be
                }
                color_slider_1d(ui, Some(a), |a| HsvaGamma { a, ..opaque }.into())
                    .on_hover_text("Alpha");
            } else if !additive {
                color_slider_1d(ui, Some(a), |a| HsvaGamma { a, ..opaque }.into())
                    .on_hover_text("Alpha");
            }
        }

        let prev_hue = color_to_show.h;
        let mut delta_hue = None;
        let hue_response = color_slider_1d(ui, Some(&mut color_to_show.h), |h| {
            HsvaGamma {
                h,
                s: 1.0,
                v: 1.0,
                a: 1.0,
            }
            .into()
        })
        .on_hover_text("Hue");
        if hue_response.clicked_by(PointerButton::Primary) {
            match modifying_control_point {
                Some(cp) => {
                    cp.val_mut()[2] = color_to_show.h;
                }
                None => {}
            }
            delta_hue = Some(color_to_show.h - prev_hue);
        }

        let (_control_points_hue_response, hue_selected_index) =
            ui_hue_control_points_overlay(ui, &hue_response, control_points, is_modifying_index);

        if let Some(new_selected_index) = hue_selected_index {
            is_modifying_index = Some(new_selected_index);
        }

        if let Some(h) = delta_hue {
            if is_curve_locked {
                // Move all other points
                for i in 0..num_control_points {
                    if is_modifying_index.is_some() {
                        if i == is_modifying_index.unwrap() {
                            continue;
                        }
                    }
                    let hue_ref = &mut control_points[i].val_mut()[2];
                    *hue_ref = (*hue_ref + h).rem_euclid(1.0);
                }
            }
        }

        let slider_2d_reponse: Response = color_slider_2d(
            ui,
            desired_size_slider_2d,
            &mut color_to_show.s,
            &mut color_to_show.v,
            main_color_picker_color_at_function(color_to_show.h, 1.0),
        );

        if let Some(mut modifying_index) = is_modifying_index {
            let valid_index = modifying_index <= control_points.len() - 1;
            assert_eq!(
                valid_index,
                true,
                "modifying index is invalid, ({:?}|{:?})",
                modifying_index,
                control_points.len()
            );
            is_modifying_index = Some(modifying_index.clamp(0, control_points.len() - 1));
            modifying_index = is_modifying_index.unwrap();

            let control_point_val = control_points[modifying_index].val_mut();
            control_point_val.val[2] = color_to_show.h;
        }

        if dragging_bezier_index.is_some() {
            let control_point = match is_modifying_index {
                Some(a) => Some(control_points[a].val_mut()),
                _ => None,
            };
            let unwrapped = &mut control_point.unwrap();
            unwrapped.val[0] = color_to_show.s;
            unwrapped.val[1] = color_to_show.v;
        }

        // let (dragged_points_response, selected_index, hovering_control_point) =
        //     PaintCurve::default().ui_content(
        //         ui,
        //         control_points,
        //         spline_mode,
        //         is_hue_middle_interpolated,
        //         &slider_2d_reponse,
        //     );

        let _spline_gradient_repsonse =
            ui_ordered_spline_gradient(ui, control_points, spline_mode, &slider_2d_reponse);

        let (
            dragged_points_response,
            selected_index,
            hovering_control_point,
            selected_tangent_index,
            dragged_tangent_response,
        ) = ui_ordered_control_points(
            ui,
            control_points,
            &is_modifying_index,
            is_hue_middle_interpolated,
            &slider_2d_reponse,
            spline_mode == SplineMode::Bezier,
        );

        *control_point_right_clicked = match hovering_control_point {
            Some(a) => {
                if a.0.clicked_by(PointerButton::Secondary) {
                    Some(a.1)
                } else {
                    None
                }
            }
            _ => None,
        };

        if dragged_points_response.is_none() {
            *dragging_bezier_index = None;
        }

        match selected_index {
            Some(index) => *last_modifying_point_index = Some(index),
            _ => {}
        }

        match dragged_points_response {
            Some(r) => {
                if r.dragged_by(PointerButton::Primary) {
                    *dragging_bezier_index = selected_index;
                    match is_modifying_index {
                        Some(index) => {
                            {
                                let point_x_ref = &mut control_points[index].val_mut()[0];
                                *point_x_ref += r.drag_delta().x / slider_2d_reponse.rect.size().x;
                            }
                            {
                                let point_y_ref = &mut control_points[index].val_mut()[1];
                                *point_y_ref -= r.drag_delta().y / slider_2d_reponse.rect.size().y;
                            }
                        }
                        _ => {}
                    }

                    if is_curve_locked {
                        // Move all other points
                        for i in 0..num_control_points {
                            if i == is_modifying_index.unwrap_or(0) {
                                continue;
                            }

                            {
                                let point_x_ref = &mut control_points[i].val_mut()[0];
                                *point_x_ref += r.drag_delta().x / slider_2d_reponse.rect.size().x;
                            }
                            {
                                let point_y_ref = &mut control_points[i].val_mut()[1];
                                *point_y_ref -= r.drag_delta().y / slider_2d_reponse.rect.size().y;
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        match dragged_tangent_response {
            Some(r) => {
                if r.dragged_by(PointerButton::Primary) {
                    match *last_modifying_point_index {
                        Some(index) => {
                            if let Some(tang) = &mut control_points[index].tangents_mut()
                                [selected_tangent_index.unwrap()]
                            {
                                {
                                    let point_x_ref = &mut tang[0];
                                    *point_x_ref +=
                                        r.drag_delta().x / slider_2d_reponse.rect.size().x;
                                }

                                {
                                    let point_y_ref = &mut tang[1];
                                    *point_y_ref -=
                                        r.drag_delta().y / slider_2d_reponse.rect.size().y;
                                }
                            }
                        }
                        _ => {}
                    }

                    // if is_curve_locked {
                    //     // Move all other points
                    //     for i in 0..num_control_points {
                    //         if i == is_modifying_index.unwrap_or(0) {
                    //             continue;
                    //         }

                    //         {
                    //             let point_x_ref = &mut control_points[i].val[0];
                    //             *point_x_ref += r.drag_delta().x / slider_2d_reponse.rect.size().x;
                    //         }
                    //         {
                    //             let point_y_ref = &mut control_points[i].val[1];
                    //             *point_y_ref -= r.drag_delta().y / slider_2d_reponse.rect.size().y;
                    //         }
                    //     }
                    // }
                }
            }
            _ => {}
        }

        slider_2d_reponse
    });

    return main_color_picker_response.inner;
}

fn main_color_picker_color_at_function(hue: f32, alpha: f32) -> impl Fn(f32, f32) -> Color32 {
    let color = HsvaGamma {
        h: hue,
        s: 0.0,
        v: 0.0,
        a: alpha,
    };

    return move |s, v| HsvaGamma { s, v, ..color }.into();
}
