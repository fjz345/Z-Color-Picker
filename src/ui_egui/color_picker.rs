use crate::{
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::{
        create_tangent_for_control_point, ControlPoint, ControlPointStorage, ControlPointTangent,
        ControlPointValue,
    },
    error::{Result, ZError},
    preset::get_presets_path,
    ui_egui::{
        app::ZColorPickerOptions,
        control_points::{ui_ordered_control_points, ControlPointUiResult},
        ui_common::contrast_color,
    },
};
use eframe::{
    egui::{
        self,
        color_picker::{show_color, Alpha},
        lerp, pos2, Layout, NumExt, PointerButton, Pos2, Rect, Response, Sense, Shape, Stroke,
        TextStyle, Ui, Widget, WidgetInfo,
    },
    epaint::{vec2, Color32, HsvaGamma, Vec2},
};
use serde::{Deserialize, Serialize};

use crate::{
    math::hue_lerp,
    preset::{delete_preset_from_disk, load_presets, save_preset_to_disk, Preset, PresetData},
    ui_egui::curves::ui_ordered_spline_gradient,
    ui_egui::ui_common::{color_slider_1d, color_slider_2d, color_text_ui},
};

pub struct MainColorPickerCtx<'a> {
    pub control_points: &'a mut Vec<ControlPoint>,
    pub spline_mode: SplineMode,
    pub color_copy_format: ColorStringCopy,
    pub last_modifying_point_index: &'a mut Option<usize>,
    pub dragging_index: &'a mut Option<usize>,
    pub control_point_right_clicked: &'a mut Option<usize>,
    pub is_hue_middle_interpolated: bool,
    pub is_curve_locked: bool,
}

pub struct ZColorPicker<'a> {
    pub ctx: &'a mut MainColorPickerCtx<'a>,
}

impl<'a> Widget for ZColorPicker<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        self.add_contents(ui)
    }
}

impl<'a> ZColorPicker<'a> {
    pub fn new(ctx: &'a mut MainColorPickerCtx<'a>) -> Self {
        Self { ctx }
    }

    fn picker_ui(&mut self, ui: &mut Ui) -> Response {
        let desired_size = vec2(200.0, 500.0);
        if let Some(last_modified_index) = *self.ctx.last_modifying_point_index {
            if self.ctx.control_points.len() == 0 {
                *self.ctx.last_modifying_point_index = None;
            } else {
                *self.ctx.last_modifying_point_index =
                    Some(last_modified_index.clamp(0, self.ctx.control_points.len() - 1));
            }
        }

        return main_color_picker(ui, desired_size, self.ctx);
    }

    /// Just the slider, no text
    fn allocate_space(&self, ui: &mut Ui) -> Response {
        let thickness = ui
            .text_style_height(&TextStyle::Body)
            .at_least(ui.spacing().interact_size.y);

        let desired_size = Vec2::new(ui.spacing().slider_width, thickness);
        ui.allocate_response(desired_size, Sense::click_and_drag())
    }

    fn add_contents(&mut self, ui: &mut Ui) -> Response {
        let old_value = self.ctx.control_points.to_vec();

        let mut response = self.picker_ui(ui);

        let value = self.ctx.control_points.to_vec();

        if value != old_value {
            response.mark_changed();
        }
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

        response
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZColorPickerWrapper {
    pub control_points: Vec<ControlPoint>,
    pub last_modifying_point_index: Option<usize>,
    pub dragging_index: Option<usize>,
    pub control_point_right_clicked: Option<usize>,
    pub options: ZColorPickerOptions,
}

impl Default for ZColorPickerWrapper {
    fn default() -> Self {
        let new_color_picker = Self {
            control_points: Self::DEFAULT_STARTUP_CONTROL_POINTS.to_vec(),
            last_modifying_point_index: None,
            dragging_index: None,
            control_point_right_clicked: None,
            options: ZColorPickerOptions::default(),
        };

        new_color_picker
    }
}
const LAZY_TANGENT_DELTA: f32 = 0.01;
impl ZColorPickerWrapper {
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
            val: ControlPointValue {
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
            val: ControlPointValue {
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

    pub fn load_presets(&mut self) {
        let path_buf = get_presets_path();
        let presets_path = path_buf.as_path();
        log::info!("Loading presets from: {}", presets_path.to_str().unwrap());
        let r = load_presets(&presets_path, &mut self.options.presets);
        if let Err(e) = r {
            dbg!(e);
        }

        // Use first as default if exists
        if self.options.presets.len() >= 1 {
            self.options.preset_selected_index = Some(0);
            match self.apply_selected_preset() {
                Ok(_) => log::info!("Preset Applied!"),
                Err(e) => log::info!("{e}"),
            }
        }
    }

    pub fn apply_preset(&mut self, preset: &Preset) -> Result<()> {
        self.control_points.clear();
        for preset_control_point in &preset.data.control_points {
            self.control_points.push(preset_control_point.clone());
        }
        self.options.spline_mode = preset.data.spline_mode;
        Ok(())
    }

    pub fn apply_selected_preset(&mut self) -> Result<Preset> {
        if let Some(s) = self.options.preset_selected_index {
            if s < self.options.presets.len() {
                let preset_to_apply = self.options.presets[s].clone();
                match self.apply_preset(&preset_to_apply) {
                    Ok(_) => return Ok(preset_to_apply),
                    Err(_) => {
                        return Err(ZError::Message(
                            "Apply preset failed. Could not apply preset".to_string(),
                        ))
                    }
                }
            }
        }
        Err(ZError::Message(
            "Apply preset failed. Could not find preset".to_string(),
        ))
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
            delete_preset_from_disk(&preset_to_remove)?;
            self.options.preset_selected_index = None;

            return Ok(());
        }

        Err(ZError::Message(
            "Selected Preset Delete failed, No preset selected".to_string(),
        ))
    }

    pub fn pre_draw_update(&mut self) {
        if self.options.spline_mode == SplineMode::Bezier {
            // Force init tangents
            for control_point in &mut self.control_points {
                for tang in control_point.tangents_mut().iter_mut() {
                    if tang.is_none() {
                        *tang = Some(create_tangent_for_control_point());
                    }
                }
            }
        }
    }

    pub fn draw_ui(&mut self, ui: &mut Ui, color_copy_format: &ColorStringCopy) -> Response {
        let inner_response = ui.vertical(|ui| {
            self.pre_draw_update();

            let mut ctx = MainColorPickerCtx {
                control_points: &mut self.control_points,
                spline_mode: self.options.spline_mode,
                color_copy_format: *color_copy_format,
                last_modifying_point_index: &mut self.last_modifying_point_index,
                dragging_index: &mut self.dragging_index,
                control_point_right_clicked: &mut self.control_point_right_clicked,
                is_hue_middle_interpolated: self.options.is_hue_middle_interpolated,
                is_curve_locked: self.options.is_curve_locked,
            };

            let color_picker_widget: ZColorPicker<'_> = ZColorPicker::new(&mut ctx);
            let main_color_picker_response = ui.add(color_picker_widget);

            self.post_draw(&main_color_picker_response);

            main_color_picker_response
        });

        inner_response.inner
    }

    pub fn remove_control_point(&mut self, index: usize) {
        self.control_points.remove(index);
        log::info!(
            "CP {} removed, new len {}",
            index,
            self.control_points.len()
        );
        if self.control_points.len() > 1 {
            self.last_modifying_point_index = Some(index.max(1) - 1);
            self.dragging_index = Some(index.max(1) - 1);
        } else {
            self.dragging_index = None;
            self.last_modifying_point_index = None;
        }
    }

    fn remove_all_control_points(&mut self) {
        for i in (0..self.control_points.len()).rev() {
            self.remove_control_point(i);
        }
        self.last_modifying_point_index = None;
        self.dragging_index = None;
    }

    pub fn spawn_control_point(&mut self, cp: ControlPoint) {
        let new_index = self.last_modifying_point_index.map_or_else(
            || {
                if self.options.is_insert_right {
                    self.control_points.len()
                } else {
                    0
                }
            },
            |idx| {
                if self.options.is_insert_right {
                    idx + 1
                } else {
                    idx
                }
            },
        );

        self.dragging_index = None;

        log::info!(
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
                log::debug!("Closest Dist: {}", dist);
                Some((closest_cp.unwrap(), dist))
            }
            None => {
                log::debug!("Did not find closest dist");
                None
            }
        }
    }

    fn interpolate_hue_middle(&mut self) {
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
    fn clamp_control_points(&mut self) {
        for cp in self.control_points.iter_mut() {
            cp.val_mut()[0] = cp.val()[0].clamp(0.0, 1.0);
            cp.val_mut()[1] = cp.val()[1].clamp(0.0, 1.0);
            cp.val_mut()[2] = cp.val()[2].clamp(0.0, 1.0);
        }
    }

    pub fn apply_control_point_constraints(&mut self) {
        if self.options.is_hue_middle_interpolated {
            self.interpolate_hue_middle();
        }
        if self.options.is_window_lock {
            self.clamp_control_points();
        }
    }

    fn post_draw(&mut self, z_color_picker_response: &Response) {
        self.apply_control_point_constraints();

        match self.control_point_right_clicked {
            Some(index) => {
                self.remove_control_point(index);
            }
            _ => {}
        }
        self.handle_doubleclick_event(z_color_picker_response);
    }

    pub fn handle_doubleclick_event(&mut self, z_color_picker_response: &Response) -> bool {
        if z_color_picker_response.double_clicked_by(PointerButton::Primary) {
            match z_color_picker_response.interact_pointer_pos() {
                Some(pos) => {
                    if z_color_picker_response.rect.contains(pos) {
                        let z_color_picker_response_xy = pos - z_color_picker_response.rect.min;
                        let normalized_xy =
                            z_color_picker_response_xy / z_color_picker_response.rect.size();

                        let closest = self.get_control_points_sdf_2d(normalized_xy.to_pos2());
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
                                    let new_cp: ControlPoint =
                                        ControlPoint::new_simple(color.into(), *cp.t());
                                    self.spawn_control_point(new_cp);
                                }
                            }
                            _ => {
                                let color: [f32; 3] = [color_xy[0], color_xy[1], 0.0];
                                let new_cp = ControlPoint::new_simple(color.into(), 0.0);
                                self.spawn_control_point(new_cp);
                            }
                        };
                        self.apply_control_point_constraints();
                    }
                }
                _ => {}
            }

            return true;
        }

        false
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
            log::info!("Not Implemented {:?}", format_type);
            format!("rgb({}, {}, {})", color.r(), color.g(), color.b())
        }
    };
    formatted.to_uppercase()
}

pub fn main_color_picker(
    ui: &mut Ui,
    desired_size: Vec2,
    ctx: &mut MainColorPickerCtx,
) -> Response {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let scale_factor = desired_size.x / ui.spacing().slider_width;
        let desired_size_slider_2d = scale_factor * Vec2::splat(ui.spacing().slider_width);
        let slider_2d_rect_size =
            Vec2::new(ui.spacing().slider_width, ui.spacing().interact_size.y) * scale_factor;

        let mut is_modifying_index = ctx.dragging_index.or(*ctx.last_modifying_point_index);
        let modifying_control_point =
            is_modifying_index.and_then(|i| ctx.control_points.get_mut(i));

        let mut color_to_show = modifying_control_point
            .as_ref()
            .map(|cp| cp.val().hsv())
            .unwrap_or(HsvaGamma {
                h: 0.0,
                s: 0.0,
                v: 0.0,
                a: 1.0,
            });

        show_color(ui, color_to_show, slider_2d_rect_size).on_hover_text("Selected color");
        let alpha = Alpha::Opaque;
        color_text_ui(ui, color_to_show, alpha, ctx.color_copy_format);
        handle_additive_blending(ui, &mut color_to_show, alpha);

        let additive = color_to_show.a < 0.0;
        let opaque = HsvaGamma {
            a: 1.0,
            ..color_to_show
        };

        handle_alpha_slider(
            ui,
            alpha,
            &mut color_to_show,
            &opaque,
            additive,
            slider_2d_rect_size,
        );

        let delta_hue = handle_hue_slider(
            ui,
            &mut color_to_show,
            slider_2d_rect_size,
            ctx,
            &mut is_modifying_index,
        );
        if let Some(h) = delta_hue {
            if let Some(_index) = is_modifying_index {
                for cp in ctx.control_points.iter_mut() {
                    let val_mut_ref = cp.val_mut();
                    val_mut_ref.val[2] = (val_mut_ref.h() - h).rem_euclid(1.0);
                }
            }
        }

        let t_to_show = {
            if let Some(cp) = is_modifying_index.and_then(|i| ctx.control_points.get(i)) {
                *cp.t()
            } else {
                0.0
            }
        };
        let mut t_to_show = t_to_show;
        let delta_t = handle_t_slider(
            ui,
            &mut t_to_show,
            slider_2d_rect_size,
            ctx,
            &mut is_modifying_index,
        );
        if let Some(t) = delta_t {
            if let Some(_index) = is_modifying_index {
                for cp in ctx.control_points.iter_mut() {
                    let val_mut_ref = cp.t_mut();
                    *val_mut_ref = (*val_mut_ref - t).rem_euclid(1.0);
                }
            }
        }

        let slider_2d_response = color_slider_2d(
            ui,
            desired_size_slider_2d,
            &mut color_to_show.s,
            &mut color_to_show.v,
            main_color_picker_color_at_function(color_to_show.h, 1.0),
        );

        let _ = ui_ordered_spline_gradient(
            ui,
            ctx.control_points,
            ctx.spline_mode,
            &slider_2d_response,
        );

        handle_control_points_ui(ui, ctx, &slider_2d_response, &mut is_modifying_index);

        slider_2d_response
    })
    .inner
}

// -------------------- Helper Functions --------------------
fn handle_additive_blending(ui: &mut Ui, color: &mut HsvaGamma, alpha: Alpha) {
    if alpha == Alpha::BlendOrAdditive {
        let a = &mut color.a;
        let mut additive = *a < 0.0;
        ui.horizontal(|ui| {
            ui.label("Blending:");
            ui.radio_value(&mut additive, false, "Normal");
            ui.radio_value(&mut additive, true, "Additive");

            *a = if additive { -a.abs() } else { a.abs() };
        });
    }
}

fn handle_alpha_slider(
    ui: &mut Ui,
    alpha: Alpha,
    color: &mut HsvaGamma,
    opaque: &HsvaGamma,
    additive: bool,
    hue_slider_size: Vec2,
) {
    if alpha == Alpha::Opaque {
        color.a = 1.0;
    } else {
        let a = &mut color.a;
        if alpha == Alpha::OnlyBlend && *a < 0.0 {
            *a = 0.5;
        }
        if alpha == Alpha::OnlyBlend || !additive {
            color_slider_1d(ui, hue_slider_size, Some(a), |a| {
                HsvaGamma { a, ..*opaque }.into()
            })
            .on_hover_text("Alpha");
        }
    }
}

fn handle_hue_slider(
    ui: &mut Ui,
    color: &mut HsvaGamma,
    slider_size: Vec2,
    ctx: &mut MainColorPickerCtx,
    is_modifying_index: &mut Option<usize>,
) -> Option<f32> {
    let mut delta_hue = None;
    let mut pick_hue_unused = 0.0;
    let pick_hue = Some(&mut pick_hue_unused);

    let hue_response = color_slider_1d(ui, slider_size, pick_hue, |h| {
        HsvaGamma {
            h,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        }
        .into()
    })
    .on_hover_text("Hue");

    if hue_response.clicked_by(PointerButton::Primary)
        || hue_response.dragged_by(PointerButton::Primary)
    {
        delta_hue = Some(color.h - pick_hue_unused);
    }

    let (_hue_response_cp, hue_selected_index) = ui_hue_control_points_overlay(
        ui,
        &hue_response,
        ctx.control_points,
        *is_modifying_index,
        ctx.is_hue_middle_interpolated,
    );

    if let Some(new_index) = hue_selected_index {
        *is_modifying_index = Some(new_index);
    }

    delta_hue
}

fn handle_t_slider(
    ui: &mut Ui,
    t_to_show: &mut f32,
    slider_size: Vec2,
    ctx: &mut MainColorPickerCtx,
    is_modifying_index: &mut Option<usize>,
) -> Option<f32> {
    let mut t_hue = None;
    let mut pick_hue_unused = 0.0;
    let pick_hue = Some(&mut pick_hue_unused);

    let t_response = color_slider_1d(ui, slider_size, pick_hue, |t| {
        HsvaGamma {
            h: 0.0,
            s: 0.0,
            v: t,
            a: 1.0,
        }
        .into()
    })
    .on_hover_text("T");

    if t_response.clicked_by(PointerButton::Primary)
        || t_response.dragged_by(PointerButton::Primary)
    {
        t_hue = Some(*t_to_show - pick_hue_unused);
    }

    let (_t_response_cp, t_selected_index) =
        ui_t_control_points_overlay(ui, &t_response, ctx.control_points, *is_modifying_index);

    if let Some(new_index) = t_selected_index {
        *is_modifying_index = Some(new_index);
    }

    t_hue
}

fn apply_drag_delta(point: &mut [f32; 2], delta: Vec2, rect_size: Vec2) {
    point[0] += delta.x / rect_size.x;
    point[1] -= delta.y / rect_size.y;
}

fn apply_drag_delta_3(point: &mut [f32; 3], delta: Vec2, rect_size: Vec2) {
    point[0] += delta.x / rect_size.x;
    point[1] -= delta.y / rect_size.y;
}

fn handle_control_points_ui(
    ui: &mut Ui,
    ctx: &mut MainColorPickerCtx,
    slider_2d_response: &Response,
    is_modifying_index: &mut Option<usize>,
) {
    let control_point_ui_result = ui_ordered_control_points(
        ui,
        ctx.control_points,
        *is_modifying_index,
        ctx.is_hue_middle_interpolated,
        &slider_2d_response,
        ctx.spline_mode == SplineMode::Bezier,
    );

    let ControlPointUiResult {
        selected_index,
        hovering_control_point,
        dragged_point,
        selected_tangent,
        dragged_tangent,
    } = control_point_ui_result;

    // Right click
    *ctx.control_point_right_clicked = hovering_control_point.and_then(|(r, idx)| {
        if r.clicked_by(PointerButton::Secondary) {
            Some(idx)
        } else {
            None
        }
    });

    if dragged_point.is_none() {
        *ctx.dragging_index = None;
    }

    if let Some(index) = selected_index {
        *ctx.last_modifying_point_index = Some(index);
    }

    // Drag control point
    if let Some(r) = dragged_point {
        if r.dragged_by(PointerButton::Primary) {
            *ctx.dragging_index = selected_index;
            if let Some(index) = *is_modifying_index {
                let cp = &mut ctx.control_points[index];
                apply_drag_delta_3(
                    &mut cp.val_mut().val,
                    r.drag_delta(),
                    slider_2d_response.rect.size(),
                );
            }

            if ctx.is_curve_locked {
                for (i, cp) in ctx.control_points.iter_mut().enumerate() {
                    if i == is_modifying_index.unwrap_or(0) {
                        continue;
                    }
                    apply_drag_delta_3(
                        &mut cp.val_mut().val,
                        r.drag_delta(),
                        slider_2d_response.rect.size(),
                    );
                }
            }
        }
    }

    // Drag tangent
    if let Some(r) = dragged_tangent {
        if r.dragged_by(PointerButton::Primary) {
            if let Some(index) = *ctx.last_modifying_point_index {
                if let Some(tang_idx) = selected_tangent {
                    if let Some(tang) = &mut ctx.control_points[index].tangents_mut()[tang_idx] {
                        apply_drag_delta_3(
                            &mut tang.val,
                            r.drag_delta(),
                            slider_2d_response.rect.size(),
                        );
                    }
                }
            }
        }
    }
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

pub fn ui_hue_control_points_overlay(
    ui: &mut Ui,
    parent_response: &Response,
    control_points: &mut [ControlPoint],
    modifying_control_point_index: Option<usize>,
    is_hue_middle_interpolated: bool,
) -> (Response, Option<usize>) {
    let container_response =
        ui.allocate_rect(parent_response.rect, Sense::focusable_noninteractive());
    const Y_OFFSET: f32 = 5.0;
    const Y_OFFSET_SELECTED: f32 = -14.0;
    ui.add_space(8.0);
    let visuals = ui.style().interact(&parent_response);

    let r = container_response.rect.height() / 4.0;

    let mut selected_key_frame = None;
    for i in 0..control_points.len() {
        if is_hue_middle_interpolated {
            if i != 0 && i != control_points.len() - 1 {
                continue;
            }
        }

        let val = control_points[i].val().h();
        let picked_color = control_points[i].val().color();
        // Show where the slider is at:
        let x = lerp(
            container_response.rect.left()..=container_response.rect.right(),
            val,
        );

        let y_offset_to_use = if let Some(index) = modifying_control_point_index {
            if i == index {
                Y_OFFSET_SELECTED
            } else {
                Y_OFFSET
            }
        } else {
            Y_OFFSET
        };

        let gizmo_rect: Vec<Pos2> = if i == 0 {
            // First
            vec![
                pos2(
                    x + r,
                    y_offset_to_use + container_response.rect.center().y + r,
                ),
                pos2(
                    x - r,
                    y_offset_to_use + container_response.rect.bottom() - r * 2.0,
                ),
                pos2(x - r, y_offset_to_use + container_response.rect.bottom()),
            ]
        } else if i == (control_points.len() - 1) {
            // Last
            vec![
                pos2(
                    x - r,
                    y_offset_to_use + container_response.rect.center().y + r,
                ),
                pos2(
                    x + r,
                    y_offset_to_use + container_response.rect.bottom() - r * 2.0,
                ),
                pos2(x + r, y_offset_to_use + container_response.rect.bottom()),
            ]
        } else {
            // Other
            vec![
                pos2(x + r, y_offset_to_use + container_response.rect.center().y),
                pos2(x + r, y_offset_to_use + container_response.rect.bottom()), // right bottom
                pos2(x - r, y_offset_to_use + container_response.rect.bottom()), // left bottom
                pos2(x - r, y_offset_to_use + container_response.rect.center().y),
            ]
        };

        let response = ui.interact(
            Rect::from_points(&gizmo_rect),
            container_response.id.with(i),
            Sense::click_and_drag(),
        );

        if response.dragged_by(PointerButton::Primary) {
            selected_key_frame = Some(i);
            control_points[i].val_mut()[2] +=
                response.drag_delta().x / container_response.rect.width();
        }

        ui.painter().add(Shape::convex_polygon(
            gizmo_rect,
            picked_color,
            Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        ));
    }

    (container_response, selected_key_frame)
}

pub fn ui_t_control_points_overlay(
    ui: &mut Ui,
    parent_response: &Response,
    control_points: &mut [ControlPoint],
    modifying_control_point_index: Option<usize>,
) -> (Response, Option<usize>) {
    let container_response =
        ui.allocate_rect(parent_response.rect, Sense::focusable_noninteractive());
    const Y_OFFSET: f32 = 5.0;
    const Y_OFFSET_SELECTED: f32 = -14.0;
    ui.add_space(8.0);
    let visuals = ui.style().interact(&parent_response);

    let r = container_response.rect.height() / 4.0;

    let mut selected_key_frame = None;
    for i in 0..control_points.len() {
        let val = *control_points[i].t();
        let picked_color = control_points[i].val().color();
        // Show where the slider is at:
        let x = lerp(
            container_response.rect.left()..=container_response.rect.right(),
            val,
        );

        let y_offset_to_use = if let Some(index) = modifying_control_point_index {
            if i == index {
                Y_OFFSET_SELECTED
            } else {
                Y_OFFSET
            }
        } else {
            Y_OFFSET
        };

        let gizmo_rect: Vec<Pos2> = if i == 0 {
            // First
            vec![
                pos2(
                    x + r,
                    y_offset_to_use + container_response.rect.center().y + r,
                ),
                pos2(
                    x - r,
                    y_offset_to_use + container_response.rect.bottom() - r * 2.0,
                ),
                pos2(x - r, y_offset_to_use + container_response.rect.bottom()),
            ]
        } else if i == (control_points.len() - 1) {
            // Last
            vec![
                pos2(
                    x - r,
                    y_offset_to_use + container_response.rect.center().y + r,
                ),
                pos2(
                    x + r,
                    y_offset_to_use + container_response.rect.bottom() - r * 2.0,
                ),
                pos2(x + r, y_offset_to_use + container_response.rect.bottom()),
            ]
        } else {
            // Other
            vec![
                pos2(x + r, y_offset_to_use + container_response.rect.center().y),
                pos2(x + r, y_offset_to_use + container_response.rect.bottom()), // right bottom
                pos2(x - r, y_offset_to_use + container_response.rect.bottom()), // left bottom
                pos2(x - r, y_offset_to_use + container_response.rect.center().y),
            ]
        };

        let response = ui.interact(
            Rect::from_points(&gizmo_rect),
            container_response.id.with(i),
            Sense::click_and_drag(),
        );

        if response.dragged_by(PointerButton::Primary) {
            selected_key_frame = Some(i);
            *control_points[i].t_mut() += response.drag_delta().x / container_response.rect.width();
        }

        ui.painter().add(Shape::convex_polygon(
            gizmo_rect,
            picked_color,
            Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        ));
    }

    (container_response, selected_key_frame)
}
