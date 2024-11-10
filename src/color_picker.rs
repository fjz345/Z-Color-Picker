use crate::{
    app::ZColorPickerOptions,
    common::{ColorStringCopy, SplineMode},
    control_point::{
        create_tangent_for_control_point, ControlPoint, ControlPointStorage, ControlPointTangent,
        ControlPointType,
    },
    error::{Result, ZError},
    preset::PRESETS_FOLDER_NAME,
    ui_common::ContentWindow,
};
use eframe::{
    egui::{
        self,
        color_picker::{show_color, Alpha},
        InnerResponse, Layout, PointerButton, Pos2, Response, Sense, Ui, Widget, WidgetInfo,
        Window,
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

pub struct ZColorPicker<'a> {
    pub control_points: &'a mut [ControlPoint],
    pub spline_mode: SplineMode,
    pub last_modifying_point_index: Option<usize>,
    pub dragging_bezier_index: Option<usize>,
    pub control_point_right_clicked: Option<usize>,
    pub color_copy_format: ColorStringCopy,
    pub is_hue_middle_interpolated: bool,
    pub is_curve_locked: bool,
}

impl<'a> Widget for ZColorPicker<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let inner_response = ui.vertical(|ui| self.add_contents(ui));

        inner_response.inner | inner_response.response
    }
}

impl<'a> ZColorPicker<'a> {
    pub fn new(control_points: &'a mut [ControlPoint]) -> Self {
        Self {
            control_points,
            spline_mode: SplineMode::HermiteBezier,
            last_modifying_point_index: None,
            dragging_bezier_index: None,
            control_point_right_clicked: None,
            color_copy_format: ColorStringCopy::default(),
            is_hue_middle_interpolated: true,
            is_curve_locked: false,
            // MainColorPickerCtx
        }
    }

    fn picker_ui(&mut self, ui: &mut Ui, response: &Response) {
        let num_control_points = self.control_points.len();
        if let Some(last_modified_index) = self.last_modifying_point_index {
            if num_control_points == 0 {
                self.last_modifying_point_index = None;
            } else {
                self.last_modifying_point_index =
                    Some(last_modified_index.clamp(0, num_control_points - 1));
            }
        }

        let desired_size = ui.max_rect().size();

        let main_color_picker_response = ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
            let scale_factor = desired_size.x / ui.spacing().slider_width;
            let desired_size_slider_2d = scale_factor * Vec2::splat(ui.spacing().slider_width);

            let mut is_modifying_index: Option<usize> = self
                .dragging_bezier_index
                .or(self.last_modifying_point_index);

            let modifying_control_point = match is_modifying_index {
                Some(index) => self.control_points.get_mut(index),
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

            let current_color_size =
                scale_factor * vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
            let response: Response =
                show_color(ui, color_to_show, current_color_size).on_hover_text("Selected color");
            response_copy_color_on_click(
                ui,
                &response,
                color_to_show,
                self.color_copy_format,
                PointerButton::Middle,
            );

            let alpha = Alpha::Opaque;
            color_text_ui(ui, color_to_show, alpha, self.color_copy_format);

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

            let hue_slider_desired_size =
                scale_factor * vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);

            if alpha == Alpha::Opaque {
                color_to_show.a = 1.0;
            } else {
                let a = &mut color_to_show.a;

                if alpha == Alpha::OnlyBlend {
                    if *a < 0.0 {
                        *a = 0.5; // was additive, but isn't allowed to be
                    }
                    color_slider_1d(ui, hue_slider_desired_size, Some(a), |a| {
                        HsvaGamma { a, ..opaque }.into()
                    })
                    .on_hover_text("Alpha");
                } else if !additive {
                    color_slider_1d(ui, hue_slider_desired_size, Some(a), |a| {
                        HsvaGamma { a, ..opaque }.into()
                    })
                    .on_hover_text("Alpha");
                }
            }

            let _prev_hue = color_to_show.h;
            let mut delta_hue = None;
            let mut pick_hue_unused = 0.0_f32;
            let pick_hue = Some(&mut pick_hue_unused);
            let hue_response = color_slider_1d(ui, hue_slider_desired_size, pick_hue, |h| {
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
                delta_hue = Some(color_to_show.h - pick_hue_unused);
            } else if hue_response.dragged_by(PointerButton::Primary) {
                delta_hue = Some(color_to_show.h - pick_hue_unused);
            }

            let (_control_points_hue_response, hue_selected_index) = ui_hue_control_points_overlay(
                ui,
                &hue_response,
                self.control_points,
                is_modifying_index,
                self.is_hue_middle_interpolated,
            );

            if let Some(new_selected_index) = hue_selected_index {
                is_modifying_index = Some(new_selected_index);
            }

            if let Some(h) = delta_hue {
                if let Some(index) = is_modifying_index {
                    if self.is_curve_locked {
                        // Move all points
                        for i in 0..num_control_points {
                            let val_mut_ref = self.control_points[i].val_mut();
                            let clamped_new_h = (val_mut_ref.h() - h).rem_euclid(1.0);
                            val_mut_ref.val[2] = clamped_new_h;
                        }
                    } else {
                        const MOVE_EVEN_IF_NOT_DRAG: bool = false;
                        if MOVE_EVEN_IF_NOT_DRAG {
                            let val_mut_ref = self.control_points[index].val_mut();
                            // Prevent wrapping from 1.0 -> 0.0, then wrap around [0,1.0]
                            let clamped_new_h =
                                (val_mut_ref.h() - h).clamp(0.0, 0.999).rem_euclid(1.0);
                            val_mut_ref.val[2] = clamped_new_h;
                        }
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

            // Do not see what this is doing, probably unuseful old code
            // if modifying_control_point.is_some() {
            //     let control_point = match modifying_control_point {
            //         Some(a) => Some(ctx.control_points[a].val_mut()),
            //         _ => None,
            //     };
            //     let unwrapped = &mut control_point.unwrap();
            //     unwrapped.val[0] = color_to_show.s;
            //     unwrapped.val[1] = color_to_show.v;
            // }

            let _spline_gradient_repsonse = ui_ordered_spline_gradient(
                ui,
                self.control_points,
                self.spline_mode,
                &slider_2d_reponse,
            );

            let (
                dragged_points_response,
                selected_index,
                hovering_control_point,
                selected_tangent_index,
                dragged_tangent_response,
            ) = ui_ordered_control_points(
                ui,
                self.control_points,
                &is_modifying_index,
                self.is_hue_middle_interpolated,
                &slider_2d_reponse,
                self.spline_mode == SplineMode::Bezier,
            );

            self.control_point_right_clicked = match hovering_control_point {
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
                self.dragging_bezier_index = None;
            }

            match selected_index {
                Some(index) => self.last_modifying_point_index = Some(index),
                _ => {}
            }

            match dragged_points_response {
                Some(r) => {
                    if r.dragged_by(PointerButton::Primary) {
                        self.dragging_bezier_index = selected_index;
                        match is_modifying_index {
                            Some(index) => {
                                {
                                    let point_x_ref = &mut self.control_points[index].val_mut()[0];
                                    *point_x_ref +=
                                        r.drag_delta().x / slider_2d_reponse.rect.size().x;
                                }
                                {
                                    let point_y_ref = &mut self.control_points[index].val_mut()[1];
                                    *point_y_ref -=
                                        r.drag_delta().y / slider_2d_reponse.rect.size().y;
                                }
                            }
                            _ => {}
                        }

                        if self.is_curve_locked {
                            // Move all other points
                            for i in 0..num_control_points {
                                if i == is_modifying_index.unwrap_or(0) {
                                    continue;
                                }

                                {
                                    let point_x_ref = &mut self.control_points[i].val_mut()[0];
                                    *point_x_ref +=
                                        r.drag_delta().x / slider_2d_reponse.rect.size().x;
                                }
                                {
                                    let point_y_ref = &mut self.control_points[i].val_mut()[1];
                                    *point_y_ref -=
                                        r.drag_delta().y / slider_2d_reponse.rect.size().y;
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
                        match self.last_modifying_point_index {
                            Some(index) => {
                                if let Some(tang) = &mut self.control_points[index].tangents_mut()
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

        // return main_color_picker_response.inner;
    }

    /// Just the slider, no text
    fn allocate_space(&self, ui: &mut Ui, vec2: Option<Vec2>) -> Response {
        let desired_size = if let Some(v) = vec2 {
            v
        } else {
            Vec2::new(200.0, 500.0)
        };
        ui.allocate_response(desired_size, Sense::click_and_drag())
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let mut old_value = self.control_points.to_vec();

        let mut response = self.allocate_space(ui, None);
        self.picker_ui(ui, &response);

        let value = self.control_points.to_vec();

        response.changed = value != old_value;
        response.widget_info(|| WidgetInfo::new(egui::WidgetType::Other));

        #[cfg(feature = "accesskit")]
        if let Some(mut node) = ui.ctx().accesskit_node(response.id) {
            use accesskit::Action;
            node.min_numeric_value = Some(*self.range.start());
            node.max_numeric_value = Some(*self.range.end());
            node.numeric_value_step = self.step;
            node.actions |= Action::SetValue;
            let clamp_range = self.clamp_range();
            if value < *clamp_range.end() {
                node.actions |= Action::Increment;
            }
            if value > *clamp_range.start() {
                node.actions |= Action::Decrement;
            }
        }

        let slider_response = response.clone();

        // let value_response = if self.show_value {
        //     let position_range = self.position_range(&response.rect);
        //     let value_response = self.value_ui(ui, position_range);
        //     if value_response.gained_focus()
        //         || value_response.has_focus()
        //         || value_response.lost_focus()
        //     {
        //         // Use the [`DragValue`] id as the id of the whole widget,
        //         // so that the focus events work as expected.
        //         response = value_response.union(response);
        //     } else {
        //         // Use the slider id as the id for the whole widget
        //         response = response.union(value_response.clone());
        //     }
        //     Some(value_response)
        // } else {
        //     None
        // };

        // if !self.text.is_empty() {
        //     let label_response = ui.add(Label::new(self.text.clone()).wrap(false));
        //     // The slider already has an accessibility label via widget info,
        //     // but sometimes it's useful for a screen reader to know
        //     // that a piece of text is a label for another widget,
        //     // e.g. so the text itself can be excluded from navigation.
        //     slider_response.labelled_by(label_response.id);
        //     if let Some(value_response) = value_response {
        //         value_response.labelled_by(label_response.id);
        //     }
        // }

        response
    }
}

pub struct ZColorPickerWrapper {
    pub control_points: Vec<ControlPoint>,
    pub last_modifying_point_index: Option<usize>,
    pub dragging_bezier_index: Option<usize>,
    pub control_point_right_clicked: Option<usize>,
    pub options: ZColorPickerOptions,
}

impl ZColorPickerWrapper {
    pub fn new() -> Self {
        let mut new_color_picker = Self {
            control_points: Vec::with_capacity(4),
            last_modifying_point_index: None,
            dragging_bezier_index: None,
            control_point_right_clicked: None,
            options: ZColorPickerOptions::default(),
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

        let cur_dir = std::env::current_dir().unwrap();
        let presets_path_buf = cur_dir.join(PRESETS_FOLDER_NAME);
        let presets_path = presets_path_buf.as_path();
        println!("Loading presets from: {}", presets_path.to_str().unwrap());
        let r = load_presets(presets_path, &mut new_color_picker.options.presets);
        if let Err(e) = r {
            dbg!(e);
        }

        // Use first as default if exists
        if new_color_picker.options.presets.len() >= 1 {
            new_color_picker.options.preset_selected_index = Some(0);
            new_color_picker.apply_selected_preset();
        } else {
            for control_point in &DEFAULT_STARTUP_CONTROL_POINTS {
                new_color_picker.spawn_control_point(control_point.clone());
            }
        }

        // new_color_picker.main_color_picker_window.open();
        // new_color_picker.options_window.open();

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
            self.spawn_control_point(preset_control_point);
        }
        self.options.spline_mode = preset.data.spline_mode;
    }

    pub fn apply_selected_preset(&mut self) {
        if let Some(s) = self.options.preset_selected_index {
            if s < self.options.presets.len() {
                let preset_to_apply = self.options.presets[s].clone();
                self.apply_preset(preset_to_apply);
            }
        }
    }

    pub fn save_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.options.preset_selected_index {
            let preset = &mut self.options.presets[s];
            preset.data = PresetData {
                spline_mode: self.options.spline_mode,
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
            spline_mode: self.options.spline_mode,
            control_points: self.control_points.clone(),
        }
    }

    pub fn create_preset(&mut self, name: &String) -> Result<()> {
        for i in self.options.presets.iter() {
            if &i.name == name {
                return Err(ZError::Message(
                    "Preset already exists with that name".to_string(),
                ));
            }
        }

        let preset = Preset::new(name, self.preset_data_from_current_state());
        let index = self.options.presets.len();
        self.options.presets.push(preset);

        self.options.preset_selected_index = Some(index);
        self.save_selected_preset()?;

        Ok(())
    }

    pub fn delete_selected_preset(&mut self) -> Result<()> {
        if let Some(s) = self.options.preset_selected_index {
            let preset_to_remove = self.options.presets.remove(s);
            delete_preset_from_disk(&get_preset_save_path(&preset_to_remove))?;
            self.options.preset_selected_index = None;

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

    pub fn spawn_control_point(&mut self, cp: ControlPoint) {
        let control_point_pivot = self.last_modifying_point_index;

        let new_index = match control_point_pivot {
            Some(index) => {
                if self.options.is_insert_right {
                    index + 1
                } else {
                    index
                }
            }
            None => {
                if self.control_points.len() <= 0 {
                    0
                } else {
                    if self.options.is_insert_right {
                        self.control_points.len()
                    } else {
                        0
                    }
                }
            }
        };

        self.dragging_bezier_index = None;

        println!(
            "ControlPoint#{} spawned @[{}]{},{},{}",
            self.control_points.len(),
            cp.t(),
            cp.val()[0],
            cp.val()[1],
            cp.val()[2],
        );
        self.control_points.insert(new_index, cp);
        // Adding keys messes with the indicies
        self.last_modifying_point_index = Some(new_index);
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
        if self.options.spline_mode == SplineMode::Bezier {
            // Force init tangents
            for control_point in &mut self.control_points {
                let _clone = control_point.clone();
                for tang in &mut control_point.tangents_mut().iter_mut() {
                    if tang.is_none() {
                        *tang = Some(create_tangent_for_control_point());
                    }
                }
            }
        }
    }

    pub fn post_update_control_points(&mut self) {
        if self.options.is_hue_middle_interpolated {
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

        if self.options.is_window_lock {
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

        // self.options_window.update();
    }

    pub fn draw_ui(&mut self, ui: &mut Ui, color_copy_format: &mut ColorStringCopy) -> Response {
        let inner_response = ui.vertical(|ui| {
            self.pre_draw_update();

            self.post_update_control_points();

            let ctx = MainColorPickerCtx {
                control_points: &mut self.control_points[..],
                spline_mode: self.options.spline_mode,
                color_copy_format: *color_copy_format,
                last_modifying_point_index: &mut self.last_modifying_point_index,
                dragging_index: &mut self.dragging_bezier_index,
                control_point_right_clicked: &mut self.control_point_right_clicked,
                is_hue_middle_interpolated: self.options.is_hue_middle_interpolated,
                is_curve_locked: self.options.is_curve_locked,
            };

            // let color_picker = ZColorPicker::new(&mut self.control_points);
            // ui.add(color_picker);

            let main_color_picker_response =
                main_color_picker(ui, Vec2 { x: 200.0, y: 500.0 }, ctx);
            // let options_draw_result = self.options_window.draw_ui(
            //     ui,
            //     &mut self.options,
            //     &mut self.control_points,
            //     color_copy_format,
            // );

            // if let Some(inner_response) = options_draw_result {
            //     if let Some(draw_result) = inner_response.inner {
            //         if let Some(should_create) = draw_result.preset_result.should_create {
            //             let r = self.create_preset(&should_create);
            //             match r {
            //                 Ok(_) => println!("Sucessfully Created"),
            //                 Err(e) => println!("Could not create preset: {}", e),
            //             }
            //         }
            //         if let Some(_) = draw_result.preset_result.should_apply {
            //             self.apply_selected_preset();
            //         }
            //         if let Some(_) = draw_result.preset_result.should_delete {
            //             let r = self.delete_selected_preset();
            //             match r {
            //                 Ok(_) => println!("Sucessfully Deleted"),
            //                 Err(e) => println!("{}", e),
            //             }
            //         }
            //         if let Some(_) = draw_result.preset_result.should_save {
            //             let r = self.save_selected_preset();
            //             match r {
            //                 Ok(_) => println!("Sucessfully Saved"),
            //                 Err(e) => println!("{}", e),
            //             }
            //         }
            //     }
            // }

            main_color_picker_response
        });

        inner_response.inner
    }
}

impl Default for ZColorPickerWrapper {
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

pub struct MainColorPickerCtx<'a> {
    pub control_points: &'a mut [ControlPoint],
    pub spline_mode: SplineMode,
    pub color_copy_format: ColorStringCopy,
    pub last_modifying_point_index: &'a mut Option<usize>,
    pub dragging_index: &'a mut Option<usize>,
    pub control_point_right_clicked: &'a mut Option<usize>,
    pub is_hue_middle_interpolated: bool,
    pub is_curve_locked: bool,
}

pub fn main_color_picker(ui: &mut Ui, desired_size: Vec2, ctx: MainColorPickerCtx) -> Response {
    let num_control_points = ctx.control_points.len();
    if let Some(last_modified_index) = *ctx.last_modifying_point_index {
        if num_control_points == 0 {
            *ctx.last_modifying_point_index = None;
        } else {
            *ctx.last_modifying_point_index =
                Some(last_modified_index.clamp(0, num_control_points - 1));
        }
    }

    let main_color_picker_response = ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let scale_factor = desired_size.x / ui.spacing().slider_width;
        let desired_size_slider_2d = scale_factor * Vec2::splat(ui.spacing().slider_width);

        let mut is_modifying_index: Option<usize> =
            ctx.dragging_index.or(*ctx.last_modifying_point_index);

        let modifying_control_point = match is_modifying_index {
            Some(index) => ctx.control_points.get_mut(index),
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

        let current_color_size =
            scale_factor * vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
        let response: Response =
            show_color(ui, color_to_show, current_color_size).on_hover_text("Selected color");
        response_copy_color_on_click(
            ui,
            &response,
            color_to_show,
            ctx.color_copy_format,
            PointerButton::Middle,
        );

        let alpha = Alpha::Opaque;
        color_text_ui(ui, color_to_show, alpha, ctx.color_copy_format);

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

        let hue_slider_desired_size =
            scale_factor * vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);

        if alpha == Alpha::Opaque {
            color_to_show.a = 1.0;
        } else {
            let a = &mut color_to_show.a;

            if alpha == Alpha::OnlyBlend {
                if *a < 0.0 {
                    *a = 0.5; // was additive, but isn't allowed to be
                }
                color_slider_1d(ui, hue_slider_desired_size, Some(a), |a| {
                    HsvaGamma { a, ..opaque }.into()
                })
                .on_hover_text("Alpha");
            } else if !additive {
                color_slider_1d(ui, hue_slider_desired_size, Some(a), |a| {
                    HsvaGamma { a, ..opaque }.into()
                })
                .on_hover_text("Alpha");
            }
        }

        let _prev_hue = color_to_show.h;
        let mut delta_hue = None;
        let mut pick_hue_unused = 0.0_f32;
        let pick_hue = Some(&mut pick_hue_unused);
        let hue_response = color_slider_1d(ui, hue_slider_desired_size, pick_hue, |h| {
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
            delta_hue = Some(color_to_show.h - pick_hue_unused);
        } else if hue_response.dragged_by(PointerButton::Primary) {
            delta_hue = Some(color_to_show.h - pick_hue_unused);
        }

        let (_control_points_hue_response, hue_selected_index) = ui_hue_control_points_overlay(
            ui,
            &hue_response,
            ctx.control_points,
            is_modifying_index,
            ctx.is_hue_middle_interpolated,
        );

        if let Some(new_selected_index) = hue_selected_index {
            is_modifying_index = Some(new_selected_index);
        }

        if let Some(h) = delta_hue {
            if let Some(index) = is_modifying_index {
                if ctx.is_curve_locked {
                    // Move all points
                    for i in 0..num_control_points {
                        let val_mut_ref = ctx.control_points[i].val_mut();
                        let clamped_new_h = (val_mut_ref.h() - h).rem_euclid(1.0);
                        val_mut_ref.val[2] = clamped_new_h;
                    }
                } else {
                    const MOVE_EVEN_IF_NOT_DRAG: bool = false;
                    if MOVE_EVEN_IF_NOT_DRAG {
                        let val_mut_ref = ctx.control_points[index].val_mut();
                        // Prevent wrapping from 1.0 -> 0.0, then wrap around [0,1.0]
                        let clamped_new_h = (val_mut_ref.h() - h).clamp(0.0, 0.999).rem_euclid(1.0);
                        val_mut_ref.val[2] = clamped_new_h;
                    }
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

        // Do not see what this is doing, probably unuseful old code
        // if modifying_control_point.is_some() {
        //     let control_point = match modifying_control_point {
        //         Some(a) => Some(ctx.control_points[a].val_mut()),
        //         _ => None,
        //     };
        //     let unwrapped = &mut control_point.unwrap();
        //     unwrapped.val[0] = color_to_show.s;
        //     unwrapped.val[1] = color_to_show.v;
        // }

        let _spline_gradient_repsonse =
            ui_ordered_spline_gradient(ui, ctx.control_points, ctx.spline_mode, &slider_2d_reponse);

        let (
            dragged_points_response,
            selected_index,
            hovering_control_point,
            selected_tangent_index,
            dragged_tangent_response,
        ) = ui_ordered_control_points(
            ui,
            ctx.control_points,
            &is_modifying_index,
            ctx.is_hue_middle_interpolated,
            &slider_2d_reponse,
            ctx.spline_mode == SplineMode::Bezier,
        );

        *ctx.control_point_right_clicked = match hovering_control_point {
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
            *ctx.dragging_index = None;
        }

        match selected_index {
            Some(index) => *ctx.last_modifying_point_index = Some(index),
            _ => {}
        }

        match dragged_points_response {
            Some(r) => {
                if r.dragged_by(PointerButton::Primary) {
                    *ctx.dragging_index = selected_index;
                    match is_modifying_index {
                        Some(index) => {
                            {
                                let point_x_ref = &mut ctx.control_points[index].val_mut()[0];
                                *point_x_ref += r.drag_delta().x / slider_2d_reponse.rect.size().x;
                            }
                            {
                                let point_y_ref = &mut ctx.control_points[index].val_mut()[1];
                                *point_y_ref -= r.drag_delta().y / slider_2d_reponse.rect.size().y;
                            }
                        }
                        _ => {}
                    }

                    if ctx.is_curve_locked {
                        // Move all other points
                        for i in 0..num_control_points {
                            if i == is_modifying_index.unwrap_or(0) {
                                continue;
                            }

                            {
                                let point_x_ref = &mut ctx.control_points[i].val_mut()[0];
                                *point_x_ref += r.drag_delta().x / slider_2d_reponse.rect.size().x;
                            }
                            {
                                let point_y_ref = &mut ctx.control_points[i].val_mut()[1];
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
                    match *ctx.last_modifying_point_index {
                        Some(index) => {
                            if let Some(tang) = &mut ctx.control_points[index].tangents_mut()
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
