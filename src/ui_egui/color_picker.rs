use crate::{
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::{create_tangent_for_control_point, ControlPoint},
    ui_egui::{
        app::ZColorPickerOptions,
        control_points::{
            control_point_pos, ui_ordered_control_points, ControlPointUiResult,
            CONTROL_POINT_FILL_RADIUS,
        },
        curvature::draw_curvature_comb,
        ui_common::contrast_color,
    },
};
use eframe::{
    egui::{
        self,
        color_picker::{show_color, Alpha},
        lerp, pos2, Frame, Layout, Margin, PointerButton, Pos2, Rect, Response, Sense, Shape,
        Stroke, Ui, Widget, WidgetInfo,
    },
    epaint::{vec2, Color32, HsvaGamma, Vec2},
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

use crate::{
    math::hue_lerp,
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
    pub is_window_lock: bool,
    pub is_show_curvature_comb: bool,
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
        let spacing = ui.spacing();
        let strip_ratio = spacing.interact_size.y / spacing.slider_width;
        let label_row_height = spacing.interact_size.y;
        // Must match main_color_picker's stack inside the margins: swatch, label, hue, T, square, and the overlays' add_space(8.0).
        let gap_count = 4.0;
        let overlay_space = 2.0 * 8.0;
        let fixed_height = label_row_height + gap_count * spacing.item_spacing.y + overlay_space;
        let (side, margin) = picker_layout(ui.available_size(), strip_ratio, fixed_height);
        let desired_size = vec2(side, side);
        if let Some(last_modified_index) = *self.ctx.last_modifying_point_index {
            if self.ctx.control_points.len() == 0 {
                *self.ctx.last_modifying_point_index = None;
            } else {
                *self.ctx.last_modifying_point_index =
                    Some(last_modified_index.clamp(0, self.ctx.control_points.len() - 1));
            }
        }

        Frame::NONE
            .inner_margin(Margin::symmetric(margin.x as i8, margin.y as i8))
            .show(ui, |ui| main_color_picker(ui, desired_size, self.ctx))
            .inner
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
    pub last_modifying_point_index: Option<usize>,
    pub dragging_index: Option<usize>,
    pub right_clicked_on_index: Option<usize>,
    pub options: ZColorPickerOptions,
}

impl Default for ZColorPickerWrapper {
    fn default() -> Self {
        let new_color_picker = Self {
            last_modifying_point_index: None,
            dragging_index: None,
            right_clicked_on_index: None,
            options: ZColorPickerOptions::default(),
        };

        new_color_picker
    }
}

impl ZColorPickerWrapper {
    pub fn pre_draw_update(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: SplineMode,
    ) {
        if spline_mode == SplineMode::Bezier {
            // Force init tangents
            for control_point in control_points {
                for tang in control_point.tangents_mut().iter_mut() {
                    if tang.is_none() {
                        *tang = Some(create_tangent_for_control_point());
                    }
                }
            }
        }
    }

    pub fn draw_ui(
        &mut self,
        ui: &mut Ui,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: SplineMode,
        color_copy_format: &ColorStringCopy,
    ) -> Response {
        let inner_response = ui.vertical(|ui| {
            self.pre_draw_update(control_points, spline_mode);

            let mut ctx = MainColorPickerCtx {
                control_points: control_points,
                spline_mode: spline_mode,
                color_copy_format: *color_copy_format,
                last_modifying_point_index: &mut self.last_modifying_point_index,
                dragging_index: &mut self.dragging_index,
                control_point_right_clicked: &mut self.right_clicked_on_index,
                is_hue_middle_interpolated: self.options.is_hue_middle_interpolated,
                is_curve_locked: self.options.is_curve_locked,
                is_window_lock: self.options.is_window_lock,
                is_show_curvature_comb: self.options.is_show_curvature_comb,
            };

            let color_picker_widget: ZColorPicker<'_> = ZColorPicker::new(&mut ctx);
            let main_color_picker_response = ui.add(color_picker_widget);

            self.post_draw(control_points, &main_color_picker_response);

            main_color_picker_response
        });

        inner_response.inner
    }

    // TODO: SHOULD MOVE THIS REMOVE LOGIC OUT SIDE OF THE COLOR PICKER
    pub fn remove_control_point(&mut self, control_points: &mut Vec<ControlPoint>, index: usize) {
        control_points.remove(index);
        log::info!("CP {} removed, new len {}", index, control_points.len());
        self.last_modifying_point_index =
            selection_after_removal(self.last_modifying_point_index, index, control_points.len());
        self.dragging_index = None;
    }

    // TODO: THIS TOO
    pub fn spawn_control_point(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        cp: ControlPoint,
    ) {
        let new_index = self.last_modifying_point_index.map_or_else(
            || {
                if self.options.is_insert_right {
                    control_points.len()
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
            control_points.len(),
            cp.t(),
            cp.val()[0],
            cp.val()[1],
            cp.val()[2],
        );
        control_points.insert(new_index, cp);
        // Adding keys messes with the indicies
        self.last_modifying_point_index = Some(new_index);
    }

    fn apply_interpolate_hue_middle(&mut self, control_points: &mut Vec<ControlPoint>) {
        let num_points = control_points.len();
        if num_points >= 2 {
            let points = &mut control_points[..];

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
    fn clamp_control_points(&mut self, control_points: &mut Vec<ControlPoint>) {
        for cp in control_points.iter_mut() {
            cp.val_mut()[0] = cp.val()[0].clamp(0.0, 1.0);
            cp.val_mut()[1] = cp.val()[1].clamp(0.0, 1.0);
            cp.val_mut()[2] = cp.val()[2].clamp(0.0, 1.0);
        }
    }

    pub fn apply_control_point_constraints(&mut self, control_points: &mut Vec<ControlPoint>) {
        if self.options.is_hue_middle_interpolated {
            self.apply_interpolate_hue_middle(control_points);
        }
        if self.options.is_window_lock {
            self.clamp_control_points(control_points);
        }
    }

    fn post_draw(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        z_color_picker_response: &Response,
    ) {
        self.apply_control_point_constraints(control_points);

        match self.right_clicked_on_index {
            Some(index) => {
                self.remove_control_point(control_points, index);
            }
            _ => {}
        }
        self.handle_doubleclick_event(control_points, z_color_picker_response);
    }

    pub fn handle_doubleclick_event(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        z_color_picker_response: &Response,
    ) {
        if !z_color_picker_response.double_clicked_by(PointerButton::Primary) {
            return;
        }
        let Some(pos) = z_color_picker_response.interact_pointer_pos() else {
            return;
        };
        let rect = z_color_picker_response.rect;
        if !rect.contains(pos) {
            return;
        }
        let click_normalized = ((pos - rect.min) / rect.size()).to_pos2();
        if let Some(cp) = point_to_spawn(
            control_points,
            click_normalized,
            rect.size(),
            CONTROL_POINT_FILL_RADIUS,
        ) {
            self.spawn_control_point(control_points, cp);
            self.apply_control_point_constraints(control_points);
        }
    }
}

fn selection_after_removal(
    selected: Option<usize>,
    removed: usize,
    new_len: usize,
) -> Option<usize> {
    let last = new_len.checked_sub(1)?;
    let selected = selected?;
    let kept = match selected.cmp(&removed) {
        Ordering::Less => selected,
        Ordering::Greater => selected - 1,
        Ordering::Equal => removed.saturating_sub(1),
    };
    Some(kept.min(last))
}

fn point_to_spawn(
    control_points: &[ControlPoint],
    click_normalized: Pos2,
    rect_size: Vec2,
    min_dist_px: f32,
) -> Option<ControlPoint> {
    let click = click_normalized.clamp(Pos2::ZERO, pos2(1.0, 1.0));
    let screen_dist = |cp: &ControlPoint| ((control_point_pos(cp) - click) * rect_size).length();
    let nearest = control_points
        .iter()
        .map(|cp| (cp, screen_dist(cp)))
        .min_by(|(_, a), (_, b)| a.total_cmp(b));
    let (hue, t) = match nearest {
        None => (0.0, 0.0),
        Some((_, dist)) if dist <= min_dist_px => return None,
        Some((cp, _)) => (cp.val().h(), *cp.t()),
    };
    Some(ControlPoint::new_simple(
        [click.x, 1.0 - click.y, hue].into(),
        t,
    ))
}

pub fn format_color_as(
    color: Color32,
    format_type: ColorStringCopy,
    no_alpha: Option<bool>,
) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let alpha = a as f32 / 255.0;
    let [linear_r, linear_g, linear_b] = [r, g, b].map(ecolor::linear_f32_from_gamma_u8);
    let [hue, saturation, value] = srgb_to_rounded_hsv(r, g, b);
    match format_type {
        ColorStringCopy::HEX if no_alpha == Some(true) => format!("{r:02X}{g:02X}{b:02X}"),
        ColorStringCopy::HEX => format!("{a:02X}{r:02X}{g:02X}{b:02X}"),
        ColorStringCopy::HEXNOA => format!("{r:02X}{g:02X}{b:02X}"),
        ColorStringCopy::SRGBHEX => format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
        ColorStringCopy::SRGB => format!("rgb({r}, {g}, {b})"),
        ColorStringCopy::SRGBA => format!("rgba({r}, {g}, {b}, {alpha:.2})"),
        ColorStringCopy::RGB => format!("rgb({linear_r:.3}, {linear_g:.3}, {linear_b:.3})"),
        ColorStringCopy::RGBA => {
            format!("rgba({linear_r:.3}, {linear_g:.3}, {linear_b:.3}, {alpha:.3})")
        }
        ColorStringCopy::HSV => format!("hsv({hue}, {saturation}%, {value}%)"),
        ColorStringCopy::HSVA => format!("hsva({hue}, {saturation}%, {value}%, {alpha:.2})"),
        ColorStringCopy::INT => u32::from_be_bytes([a, r, g, b]).to_string(),
        ColorStringCopy::FLOAT => format!(
            "{:.3}, {:.3}, {:.3}, {alpha:.3}",
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0
        ),
    }
}

fn srgb_to_rounded_hsv(r: u8, g: u8, b: u8) -> [u32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = (max - min) as f32;
    let [r_f, g_f, b_f] = [r, g, b].map(f32::from);
    let hue_sector = if max == min {
        0.0
    } else if max == r {
        ((g_f - b_f) / delta).rem_euclid(6.0)
    } else if max == g {
        (b_f - r_f) / delta + 2.0
    } else {
        (r_f - g_f) / delta + 4.0
    };
    let saturation = if max == 0 { 0.0 } else { delta / max as f32 };
    [
        (hue_sector * 60.0).round() as u32 % 360,
        (saturation * 100.0).round() as u32,
        (max as f32 / 255.0 * 100.0).round() as u32,
    ]
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
                    *cp.t_mut() -= t;
                    *cp.t_mut() = cp.t().clamp(0.0, 1.0);
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

        // TODO: Fix for hermite bezier
        if ctx.is_show_curvature_comb {
            if ctx.spline_mode == SplineMode::Bezier {
                draw_curvature_comb(ui, slider_2d_response.rect, ctx.control_points);
            }
        }

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
fn picker_side(available: Vec2, strip_ratio: f32, fixed_height: f32) -> f32 {
    let height_limited_side = (available.y - fixed_height) / (1.0 + 3.0 * strip_ratio);
    available.x.min(height_limited_side).clamp(160.0, 800.0)
}

fn picker_layout(available: Vec2, strip_ratio: f32, fixed_height: f32) -> (f32, Vec2) {
    let gizmo_ratio = strip_ratio / 4.0;
    let fill_margin = CONTROL_POINT_FILL_RADIUS.ceil();
    let gizmo_margins_ceil_slack = 2.0;
    let width_limited_side = (available.x - 2.0 * fill_margin)
        .min((available.x - gizmo_margins_ceil_slack) / (1.0 + 2.0 * gizmo_ratio));
    let side = picker_side(
        vec2(width_limited_side, available.y - 2.0 * fill_margin),
        strip_ratio,
        fixed_height,
    );
    let margin = vec2(fill_margin.max((side * gizmo_ratio).ceil()), fill_margin);
    (side, margin)
}

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

fn drag_delta_sv(delta: Vec2, rect_size: Vec2) -> Vec2 {
    vec2(delta.x / rect_size.x, -delta.y / rect_size.y)
}

fn apply_drag_delta_3(point: &mut [f32; 3], delta: Vec2, rect_size: Vec2) {
    let delta_sv = drag_delta_sv(delta, rect_size);
    point[0] += delta_sv.x;
    point[1] += delta_sv.y;
}

fn limit_group_delta(points: &[ControlPoint], delta_sv: Vec2) -> Vec2 {
    let mut min_sv = Vec2::splat(f32::INFINITY);
    let mut max_sv = Vec2::splat(f32::NEG_INFINITY);
    for cp in points {
        let sv = vec2(cp.val()[0], cp.val()[1]);
        min_sv = min_sv.min(sv);
        max_sv = max_sv.max(sv);
    }
    let lo = (-min_sv).min(Vec2::ZERO);
    let hi = (Vec2::splat(1.0) - max_sv).max(Vec2::ZERO);
    delta_sv.clamp(lo, hi)
}

fn apply_group_delta_sv(points: &mut [ControlPoint], delta_sv: Vec2) {
    for cp in points {
        let val = &mut cp.val_mut().val;
        val[0] += delta_sv.x;
        val[1] += delta_sv.y;
    }
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
            if ctx.is_curve_locked && ctx.is_window_lock {
                let delta_sv = limit_group_delta(
                    ctx.control_points,
                    drag_delta_sv(r.drag_delta(), slider_2d_response.rect.size()),
                );
                apply_group_delta_sv(ctx.control_points, delta_sv);
            } else {
                if let Some(index) = selected_index {
                    let cp = &mut ctx.control_points[index];
                    apply_drag_delta_3(
                        &mut cp.val_mut().val,
                        r.drag_delta(),
                        slider_2d_response.rect.size(),
                    );
                }

                if ctx.is_curve_locked {
                    for (i, cp) in ctx.control_points.iter_mut().enumerate() {
                        if i == selected_index.unwrap_or(0) {
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

fn handle_t_slider(
    ui: &mut Ui,
    t_to_show: &mut f32,
    slider_size: Vec2,
    ctx: &mut MainColorPickerCtx,
    is_modifying_index: &mut Option<usize>,
) -> Option<f32> {
    let mut pick_t = 0.0;
    let mut pick_x = 0.0;

    let t_response = color_slider_2d(ui, slider_size, &mut pick_x, &mut pick_t, |x, _t| {
        HsvaGamma {
            h: 0.0,
            s: 0.0,
            v: x,
            a: 1.0,
        }
        .into()
    })
    .on_hover_text("T");

    let (_t_response_cp, t_selected_index) =
        ui_t_control_points_overlay(ui, &t_response, ctx.control_points);

    if let Some(new_index) = t_selected_index {
        *is_modifying_index = Some(new_index);
    }

    draw_t_slider_curve(ui, t_response.rect, ctx.control_points);

    if t_response.clicked_by(PointerButton::Primary)
        || t_response.dragged_by(PointerButton::Primary)
    {
        return Some(*t_to_show - pick_t);
    } else {
        None
    }
}

fn t_slider_x_fraction(index: usize, count: usize) -> f32 {
    if count <= 1 {
        return 0.5;
    }
    index as f32 / (count - 1) as f32
}

/// Draw a line representing the t values of control points across the slider
fn draw_t_slider_curve(
    ui: &mut egui::Ui,
    slider_rect: egui::Rect,
    control_points: &[ControlPoint],
) {
    if control_points.is_empty() {
        return;
    }

    let mut points = Vec::with_capacity(control_points.len());
    for (i, cp) in control_points.iter().enumerate() {
        let x = lerp(
            slider_rect.left()..=slider_rect.right(),
            t_slider_x_fraction(i, control_points.len()),
        );
        let y = lerp(slider_rect.bottom()..=slider_rect.top(), *cp.t());

        points.push(pos2(x, y));
    }

    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, egui::Color32::WHITE),
    ));
}

pub fn ui_t_control_points_overlay(
    ui: &mut Ui,
    parent_response: &Response,
    control_points: &mut [ControlPoint],
) -> (Response, Option<usize>) {
    let container_response =
        ui.allocate_rect(parent_response.rect, Sense::focusable_noninteractive());
    ui.add_space(8.0);
    let visuals = ui.style().interact(&parent_response);

    let r = container_response.rect.height() / 4.0;

    let mut selected_key_frame = None;
    for i in 0..control_points.len() {
        let val = *control_points[i].t();
        let picked_color = control_points[i].val().color();
        // Show where the slider is at
        let x: f32 = lerp(
            container_response.rect.left()..=container_response.rect.right(),
            t_slider_x_fraction(i, control_points.len()),
        );
        let y = lerp(
            container_response.rect.bottom()..=container_response.rect.top(),
            val,
        );

        let gizmo_rect: Vec<Pos2> = if i == 0 {
            // First
            vec![pos2(x - r, y), pos2(x + r, y + r), pos2(x + r, y - r)]
        } else if i == (control_points.len() - 1) {
            // Last
            vec![pos2(x + r, y), pos2(x - r, y + r), pos2(x - r, y - r)]
        } else {
            // Other
            vec![
                pos2(x + r, y + r),
                pos2(x + r, y - r),
                pos2(x - r, y - r),
                pos2(x - r, y + r),
            ]
        };

        let response = ui.interact(
            Rect::from_points(&gizmo_rect),
            container_response.id.with(i),
            Sense::click_and_drag(),
        );

        if response.dragged_by(PointerButton::Primary) {
            selected_key_frame = Some(i);
            *control_points[i].t_mut() -=
                response.drag_delta().y / container_response.rect.height();
            *control_points[i].t_mut() = control_points[i].t().clamp(0.0, 1.0);
        }

        ui.painter().add(Shape::convex_polygon(
            gizmo_rect,
            picked_color,
            Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        ));
    }

    (container_response, selected_key_frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;

    fn orange() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 128, 0, 204)
    }

    fn format_orange(format: ColorStringCopy) -> String {
        format_color_as(orange(), format, None)
    }

    const STRIP_RATIO: f32 = 0.18;
    const FIXED_HEIGHT: f32 = 46.0;

    fn picker_height(side: f32) -> f32 {
        side * (1.0 + 3.0 * STRIP_RATIO) + FIXED_HEIGHT
    }

    #[test]
    fn picker_side_width_limited_fills_width() {
        assert_eq!(
            picker_side(vec2(300.0, 2000.0), STRIP_RATIO, FIXED_HEIGHT),
            300.0
        );
    }

    #[test]
    fn picker_side_height_limited_fills_height() {
        let side = picker_side(vec2(2000.0, 400.0), STRIP_RATIO, FIXED_HEIGHT);
        assert!((picker_height(side) - 400.0).abs() < 1e-3, "side {side}");
    }

    #[test]
    fn picker_side_tiny_pane_clamps_to_min() {
        assert_eq!(
            picker_side(vec2(50.0, 50.0), STRIP_RATIO, FIXED_HEIGHT),
            160.0
        );
    }

    #[test]
    fn picker_side_huge_pane_clamps_to_max() {
        assert_eq!(
            picker_side(vec2(5000.0, 5000.0), STRIP_RATIO, FIXED_HEIGHT),
            800.0
        );
    }

    #[test]
    fn picker_side_unclamped_fits_available() {
        for w in [170.0, 250.0, 480.0, 799.0, 1200.0] {
            for h in [300.0, 420.0, 700.0, 1300.0] {
                let side = picker_side(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                if side > 160.0 && side < 800.0 {
                    assert!(side <= w, "{w}x{h}: side {side}");
                    assert!(picker_height(side) <= h + 1e-3, "{w}x{h}: side {side}");
                }
            }
        }
    }

    const LAYOUT_WIDTHS: [f32; 6] = [170.0, 250.0, 480.0, 799.0, 900.0, 1200.0];
    const LAYOUT_HEIGHTS: [f32; 5] = [300.0, 420.0, 700.0, 1000.0, 1400.0];

    #[test]
    fn picker_layout_margins_cover_control_point_fill() {
        for w in LAYOUT_WIDTHS {
            for h in LAYOUT_HEIGHTS {
                let (_, margin) = picker_layout(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                assert!(margin.x >= CONTROL_POINT_FILL_RADIUS, "{w}x{h}: {margin:?}");
                assert!(margin.y >= CONTROL_POINT_FILL_RADIUS, "{w}x{h}: {margin:?}");
            }
        }
    }

    #[test]
    fn picker_layout_horizontal_margin_covers_strip_gizmos() {
        for w in LAYOUT_WIDTHS {
            for h in LAYOUT_HEIGHTS {
                let (side, margin) = picker_layout(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                assert!(
                    margin.x >= side * STRIP_RATIO / 4.0,
                    "{w}x{h}: side {side}, {margin:?}"
                );
            }
        }
    }

    #[test]
    fn picker_layout_unclamped_fits_available() {
        let mut unclamped_count = 0;
        for w in LAYOUT_WIDTHS {
            for h in LAYOUT_HEIGHTS {
                let (side, margin) = picker_layout(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                if side > 160.0 && side < 800.0 {
                    unclamped_count += 1;
                    assert!(side + 2.0 * margin.x <= w + 1e-3, "{w}x{h}: side {side}");
                    assert!(
                        picker_height(side) + 2.0 * margin.y <= h + 1e-3,
                        "{w}x{h}: side {side}"
                    );
                }
            }
        }
        assert!(unclamped_count >= 10, "{unclamped_count}");
    }

    fn square_and_pane_rects(screen_size: Vec2) -> (Rect, Rect) {
        let ctx = egui::Context::default();
        let mut picker = ZColorPickerWrapper::default();
        let mut points = vec![
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.0), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 0.0, 0.3), 0.3),
            ControlPoint::new_simple(ControlPointValue::new(0.0, 1.0, 0.6), 0.6),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 1.0), 1.0),
        ];
        let mut rects = None;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen_size)),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let pane = ui.max_rect();
                    let square = picker
                        .draw_ui(
                            ui,
                            &mut points,
                            SplineMode::HermiteBezier,
                            &ColorStringCopy::HEXNOA,
                        )
                        .rect;
                    rects = Some((square, pane));
                });
            });
        }
        rects.unwrap()
    }

    fn assert_square_inset(screen_size: Vec2) {
        let (square, pane) = square_and_pane_rects(screen_size);
        let horizontal = CONTROL_POINT_FILL_RADIUS.max(square.width() * STRIP_RATIO / 4.0) - 0.5;
        let vertical = CONTROL_POINT_FILL_RADIUS - 0.5;
        assert!(
            square.width() > 160.0 && square.width() < 800.0,
            "{square:?}"
        );
        assert!(
            square.left() - pane.left() >= horizontal,
            "{square:?} in {pane:?}"
        );
        assert!(
            pane.right() - square.right() >= horizontal,
            "{square:?} in {pane:?}"
        );
        assert!(
            pane.bottom() - square.bottom() >= vertical,
            "{square:?} in {pane:?}"
        );
    }

    #[test]
    fn picker_insets_square_in_width_limited_pane() {
        assert_square_inset(vec2(260.0, 900.0));
    }

    #[test]
    fn picker_insets_square_in_height_limited_pane() {
        assert_square_inset(vec2(1000.0, 520.0));
    }

    #[test]
    fn t_slider_x_fraction_centers_single_point() {
        assert_eq!(t_slider_x_fraction(0, 1), 0.5);
    }

    #[test]
    fn t_slider_x_fraction_spans_slider_for_many_points() {
        assert_eq!(t_slider_x_fraction(0, 3), 0.0);
        assert_eq!(t_slider_x_fraction(1, 3), 0.5);
        assert_eq!(t_slider_x_fraction(2, 3), 1.0);
    }

    #[test]
    fn format_color_as_hex() {
        assert_eq!(format_orange(ColorStringCopy::HEX), "CCFF8000");
    }

    #[test]
    fn format_color_as_hexnoa() {
        assert_eq!(format_orange(ColorStringCopy::HEXNOA), "FF8000");
    }

    #[test]
    fn format_color_as_srgbhex() {
        assert_eq!(format_orange(ColorStringCopy::SRGBHEX), "#FF8000CC");
    }

    #[test]
    fn format_color_as_srgb() {
        assert_eq!(format_orange(ColorStringCopy::SRGB), "rgb(255, 128, 0)");
    }

    #[test]
    fn format_color_as_srgba() {
        assert_eq!(
            format_orange(ColorStringCopy::SRGBA),
            "rgba(255, 128, 0, 0.80)"
        );
    }

    #[test]
    fn format_color_as_rgb() {
        assert_eq!(
            format_orange(ColorStringCopy::RGB),
            "rgb(1.000, 0.216, 0.000)"
        );
    }

    #[test]
    fn format_color_as_rgba() {
        assert_eq!(
            format_orange(ColorStringCopy::RGBA),
            "rgba(1.000, 0.216, 0.000, 0.800)"
        );
    }

    #[test]
    fn format_color_as_hsv() {
        assert_eq!(format_orange(ColorStringCopy::HSV), "hsv(30, 100%, 100%)");
    }

    #[test]
    fn format_color_as_hsva() {
        assert_eq!(
            format_orange(ColorStringCopy::HSVA),
            "hsva(30, 100%, 100%, 0.80)"
        );
    }

    #[test]
    fn format_color_as_int() {
        assert_eq!(format_orange(ColorStringCopy::INT), "3439296512");
    }

    #[test]
    fn format_color_as_float() {
        assert_eq!(
            format_orange(ColorStringCopy::FLOAT),
            "1.000, 0.502, 0.000, 0.800"
        );
    }

    #[test]
    fn format_color_as_hsv_gray_has_zero_hue() {
        assert_eq!(
            format_color_as(Color32::from_rgb(128, 128, 128), ColorStringCopy::HSV, None),
            "hsv(0, 0%, 50%)"
        );
    }

    fn points_sv(values: &[(f32, f32)]) -> Vec<ControlPoint> {
        values
            .iter()
            .map(|&(s, v)| ControlPoint::new_simple([s, v, 0.0].into(), 0.0))
            .collect()
    }

    #[test]
    fn selection_after_removal_keeps_unremoved_selection() {
        assert_eq!(selection_after_removal(Some(4), 1, 4), Some(3));
        assert_eq!(selection_after_removal(Some(0), 3, 4), Some(0));
    }

    #[test]
    fn selection_after_removal_of_selected_moves_to_left_neighbour() {
        assert_eq!(selection_after_removal(Some(1), 1, 4), Some(0));
        assert_eq!(selection_after_removal(Some(0), 0, 4), Some(0));
        assert_eq!(selection_after_removal(Some(4), 4, 4), Some(3));
    }

    #[test]
    fn selection_after_removal_none_when_empty_or_unselected() {
        assert_eq!(selection_after_removal(Some(0), 0, 0), None);
        assert_eq!(selection_after_removal(None, 2, 4), None);
    }

    #[test]
    fn remove_control_point_keeps_selected_point_and_ends_drag() {
        let mut points = points_sv(&[(0.0, 0.0), (0.1, 0.1), (0.2, 0.2), (0.3, 0.3), (0.4, 0.4)]);
        let mut wrapper = ZColorPickerWrapper {
            last_modifying_point_index: Some(4),
            dragging_index: Some(4),
            ..Default::default()
        };
        wrapper.remove_control_point(&mut points, 1);
        assert_eq!(wrapper.last_modifying_point_index, Some(3));
        assert_eq!(wrapper.dragging_index, None);
    }

    #[test]
    fn remove_control_point_down_to_one_keeps_selection() {
        let mut points = points_sv(&[(0.0, 0.0), (0.5, 0.5)]);
        let mut wrapper = ZColorPickerWrapper {
            last_modifying_point_index: Some(1),
            ..Default::default()
        };
        wrapper.remove_control_point(&mut points, 0);
        assert_eq!(wrapper.last_modifying_point_index, Some(0));
    }

    #[test]
    fn limit_group_delta_in_range_unchanged() {
        let points = points_sv(&[(0.25, 0.5), (0.9, 0.3)]);
        let delta = vec2(0.05, -0.1);
        assert_eq!(limit_group_delta(&points, delta), delta);
    }

    #[test]
    fn limit_group_delta_stops_exactly_on_border() {
        let mut points = points_sv(&[(0.25, 0.5), (0.9, 0.3)]);
        let delta = limit_group_delta(&points, vec2(0.2, -0.7));
        apply_group_delta_sv(&mut points, delta);
        assert_eq!(points[1].val()[0], 1.0);
        assert_eq!(points[1].val()[1], 0.0);
    }

    #[test]
    fn limit_group_delta_push_past_border_and_back_keeps_offsets() {
        let mut points = points_sv(&[(0.25, 0.5), (0.9, 0.3)]);
        let mut wrapper = ZColorPickerWrapper::default();
        for push in [vec2(0.2, 0.0), vec2(-0.2, 0.0)] {
            let delta = limit_group_delta(&points, push);
            apply_group_delta_sv(&mut points, delta);
            wrapper.clamp_control_points(&mut points);
        }
        let ds = points[1].val()[0] - points[0].val()[0];
        let dv = points[1].val()[1] - points[0].val()[1];
        assert!((ds - 0.65).abs() < 1e-6, "ds {ds}");
        assert!((dv - -0.2).abs() < 1e-6, "dv {dv}");
    }

    #[test]
    fn limit_group_delta_empty_unchanged() {
        let delta = vec2(0.3, -2.0);
        assert_eq!(limit_group_delta(&[], delta), delta);
    }

    #[test]
    fn limit_group_delta_wider_than_range_does_not_move() {
        let points = points_sv(&[(-0.1, 0.5), (1.1, 0.5)]);
        assert_eq!(limit_group_delta(&points, vec2(0.2, 0.0)).x, 0.0);
    }

    fn spawn_at(points: &[ControlPoint], x: f32, y: f32) -> Option<ControlPoint> {
        point_to_spawn(
            points,
            pos2(x, y),
            vec2(200.0, 200.0),
            CONTROL_POINT_FILL_RADIUS,
        )
    }

    fn point(s: f32, v: f32, h: f32, t: f32) -> ControlPoint {
        ControlPoint::new_simple([s, v, h].into(), t)
    }

    #[test]
    fn point_to_spawn_empty_list_uses_click_with_zero_hue_and_t() {
        assert_eq!(spawn_at(&[], 0.25, 0.75), Some(point(0.25, 0.25, 0.0, 0.0)));
    }

    #[test]
    fn point_to_spawn_on_point_disc_spawns_nothing() {
        let points = [point(0.5, 0.5, 0.3, 0.7)];
        assert_eq!(spawn_at(&points, 0.565, 0.5), None);
        assert_eq!(spawn_at(&points, 0.5, 0.45), None);
    }

    #[test]
    fn point_to_spawn_just_outside_disc_inherits_hue_and_t() {
        let points = [point(0.5, 0.5, 0.3, 0.7)];
        assert_eq!(
            spawn_at(&points, 0.58, 0.5),
            Some(point(0.58, 0.5, 0.3, 0.7))
        );
        assert_eq!(spawn_at(&points, 0.6, 0.5), Some(point(0.6, 0.5, 0.3, 0.7)));
    }

    #[test]
    fn point_to_spawn_inherits_from_nearest_point() {
        let points = [point(0.25, 0.5, 0.1, 0.2), point(0.75, 0.5, 0.9, 0.8)];
        assert_eq!(
            spawn_at(&points, 0.625, 0.5),
            Some(point(0.625, 0.5, 0.9, 0.8))
        );
    }

    #[test]
    fn point_to_spawn_clamps_click_outside_unit_square() {
        assert_eq!(spawn_at(&[], 1.5, -0.25), Some(point(1.0, 1.0, 0.0, 0.0)));
    }

    #[test]
    fn point_to_spawn_top_left_is_zero_saturation_full_value() {
        assert_eq!(spawn_at(&[], 0.0, 0.0), Some(point(0.0, 1.0, 0.0, 0.0)));
    }
}
