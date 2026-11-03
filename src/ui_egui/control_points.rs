use ecolor::Color32;
use eframe::{
    egui::{self, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    epaint::PathShape,
};
use splines::{Interpolate, Interpolation, Key, Spline};
use std::ops::{Add, Mul, Sub};

use crate::{
    color_space::{linear_to_picker, BlendSpace, PickerSpace},
    common::SplineMode,
    datatypes::control_point::{ControlPoint, ControlPointValue},
    math::shortest_hue_delta,
    ui_egui::{
        blend::Gradient, tangents::ui_control_point_tangents, ui_common::black_or_white_against,
    },
};

const CONTROL_POINT_RADIUS: f32 = 8.0;
const FILL_RADIUS_SCALE: f32 = 1.8;
pub const CONTROL_POINT_FILL_RADIUS: f32 = FILL_RADIUS_SCALE * CONTROL_POINT_RADIUS;

#[derive(Default)]
pub struct ControlPointUiResult {
    pub dragged_point: Option<egui::Response>,
    pub selected_index: Option<usize>,
    pub hovering_control_point: Option<(egui::Response, usize)>,
    pub selected_tangent: Option<usize>,
    pub dragged_tangent: Option<egui::Response>,
    pub dragged_tangent_offset: Option<Vec2>,
}

pub fn control_point_pos(cp: &ControlPoint) -> Pos2 {
    value_pos(cp.val())
}

pub fn value_pos(val: &ControlPointValue) -> Pos2 {
    Pos2::new(val[0].clamp(0.0, 1.0), 1.0 - val[1].clamp(0.0, 1.0))
}

pub fn unit_to_screen(square: Rect, unit: Pos2) -> Pos2 {
    square.lerp_inside(unit.to_vec2())
}

pub fn screen_to_unit(square: Rect, screen: Pos2) -> Pos2 {
    ((screen - square.min) / square.size()).to_pos2()
}

fn screen_pos(square: Rect, cp: &ControlPoint) -> Pos2 {
    unit_to_screen(square, control_point_pos(cp))
}

pub fn control_point_outline(fill: Color32, emphasized: bool) -> Stroke {
    let width = if emphasized { 2.5 } else { 1.5 };
    Stroke::new(width, black_or_white_against(fill))
}

fn control_point_hint(index: usize, count: usize) -> String {
    format!(
        "Point {} of {count}. Drag: move. Right-click: remove",
        index + 1
    )
}

pub fn ui_ordered_control_points(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    marked_control_point_index: Option<usize>,
    _is_middle_interpolated: bool,
    parent_response: &egui::Response,
    show_bezier_tangents: bool,
    picker_space: PickerSpace,
) -> ControlPointUiResult {
    use egui::PointerButton::Primary;

    const SHOW_LINEAR_LINE: bool = false;

    if control_points.is_empty() {
        return ControlPointUiResult::default();
    }

    let square = parent_response.rect;
    let hit_size = Vec2::splat(2.0 * CONTROL_POINT_RADIUS);

    let mut selected_index = marked_control_point_index;
    let mut tangent_selected_index = None;
    let mut hovering_control_point = None;
    let mut dragged_point_response = None;
    let mut dragged_tangent_response = None;
    let mut dragged_tangent_offset = None;

    let mut tangent_shapes = Vec::new();
    let mut tangent_paths = Vec::new();
    if show_bezier_tangents {
        for (i, cp) in control_points.iter().enumerate() {
            let result = ui_control_point_tangents(
                ui,
                i,
                cp,
                i == 0,
                i == control_points.len() - 1,
                marked_control_point_index == Some(i),
                square,
                parent_response,
                hit_size,
                CONTROL_POINT_RADIUS,
                &mut tangent_shapes,
                &mut tangent_paths,
            );

            if dragged_tangent_response.is_none() {
                dragged_tangent_response = result.dragged_tangent;
                tangent_selected_index = result.selected_tangent;
                dragged_tangent_offset = result.dragged_tangent_offset;
            }

            if result.selected_by_tangent {
                selected_index = Some(i);
            }
        }
    }

    let fills: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let center = screen_pos(square, cp);
            let hit_rect = Rect::from_center_size(center, hit_size);
            let response = ui
                .interact(
                    hit_rect,
                    parent_response.id.with(i),
                    Sense::click_and_drag(),
                )
                .on_hover_and_drag_cursor(CursorIcon::Grab)
                .on_hover_text(control_point_hint(i, control_points.len()));
            if response.dragged_by(Primary) {
                ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
            }

            if dragged_point_response.is_none()
                && (response.dragged_by(Primary) || response.clicked_by(Primary))
            {
                selected_index = Some(i);
                dragged_point_response = Some(response.clone());
            }

            if hovering_control_point.is_none() && response.hovered() {
                hovering_control_point = Some((response, i));
            }

            Shape::circle_filled(
                center,
                CONTROL_POINT_FILL_RADIUS,
                cp.val().color(picker_space),
            )
        })
        .collect();

    let hovered_index = hovering_control_point.as_ref().map(|(_, i)| *i);
    let outlines: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let center = screen_pos(square, cp);
            let emphasized = hovered_index == Some(i) || marked_control_point_index == Some(i);
            let outline = control_point_outline(cp.val().color(picker_space), emphasized);

            if i == 0 || i == control_points.len() - 1 {
                Shape::rect_stroke(
                    Rect::from_center_size(center, Vec2::splat(CONTROL_POINT_RADIUS)),
                    0.0,
                    outline,
                    egui::StrokeKind::Middle,
                )
            } else {
                Shape::circle_stroke(center, CONTROL_POINT_RADIUS, outline)
            }
        })
        .collect();

    if SHOW_LINEAR_LINE {
        let points: Vec<Pos2> = control_points
            .iter()
            .map(|cp| screen_pos(square, cp))
            .collect();

        ui.painter().add(PathShape::line(
            points,
            Stroke::new(1.0, Color32::RED.linear_multiply(0.25)),
        ));
    }

    ui.painter().extend(fills);
    ui.painter().extend(outlines);
    ui.painter()
        .extend(tangent_paths.into_iter().map(Into::into));
    ui.painter().extend(tangent_shapes);

    if let Some(marked) = marked_control_point_index {
        let marked_point = &control_points[marked];
        let center = screen_pos(square, marked_point);
        ui.painter().add(Shape::rect_stroke(
            Rect::from_center_size(center, Vec2::splat(CONTROL_POINT_RADIUS * 0.5)),
            0.0,
            control_point_outline(marked_point.val().color(picker_space), true),
            egui::StrokeKind::Middle,
        ));
    }

    ControlPointUiResult {
        dragged_point: dragged_point_response,
        selected_index,
        hovering_control_point,
        selected_tangent: tangent_selected_index,
        dragged_tangent: dragged_tangent_response,
        dragged_tangent_offset,
    }
}

pub fn flatten_control_points(control_points: &[ControlPoint]) -> Vec<ControlPoint> {
    let mut flattened = control_points.to_vec();
    for i in 1..flattened.len() {
        let prev_raw_hue = control_points[i - 1].val()[2];
        let raw_hue = control_points[i].val()[2];
        let target = flattened[i - 1].val()[2] + shortest_hue_delta(prev_raw_hue, raw_hue);
        flattened[i].val_mut()[2] = raw_hue + (target - raw_hue).round();
    }
    flattened
}

pub const MIN_T_GAP: f32 = 1e-3;

pub fn has_valid_t(points: &[ControlPoint]) -> bool {
    match points {
        [] => true,
        [only] => *only.t() == 0.0,
        [first, .., last] => {
            *first.t() == 0.0
                && *last.t() == 1.0
                && points.windows(2).all(|pair| pair[0].t() < pair[1].t())
        }
    }
}

fn evenly_spaced_t(index: usize, count: usize) -> f32 {
    if count <= 1 {
        0.0
    } else {
        index as f32 / (count - 1) as f32
    }
}

pub fn space_t_evenly(points: &mut [ControlPoint]) {
    let count = points.len();
    for (i, cp) in points.iter_mut().enumerate() {
        *cp.t_mut() = evenly_spaced_t(i, count);
    }
}

pub fn normalize_t(points: &mut [ControlPoint]) -> bool {
    if has_valid_t(points) {
        return false;
    }
    space_t_evenly(points);
    true
}

fn rescale_t_onto_unit_range(points: &mut [ControlPoint]) {
    let (Some(first), Some(last)) = (points.first(), points.last()) else {
        return;
    };
    let (first, last) = (*first.t(), *last.t());
    if last <= first {
        space_t_evenly(points);
        return;
    }
    for cp in points.iter_mut() {
        *cp.t_mut() = (*cp.t() - first) / (last - first);
    }
}

pub fn insert_point(points: &mut Vec<ControlPoint>, index: usize, mut cp: ControlPoint) {
    let old_count = points.len();
    let end_share = 1.0 / old_count.max(1) as f32;
    *cp.t_mut() = if old_count == 0 || index == 0 {
        0.0
    } else if index == old_count {
        1.0
    } else {
        0.5 * (points[index - 1].t() + points[index].t())
    };
    if old_count > 0 && index == 0 {
        for p in points.iter_mut() {
            *p.t_mut() = end_share + *p.t() * (1.0 - end_share);
        }
    } else if old_count > 0 && index == old_count {
        for p in points.iter_mut() {
            *p.t_mut() *= 1.0 - end_share;
        }
    }
    points.insert(index, cp);
    keep_min_t_gap(points);
}

fn keep_min_t_gap(points: &mut [ControlPoint]) {
    let count = points.len();
    if count < 3 || points.windows(2).all(|pair| pair[1].t() - pair[0].t() >= MIN_T_GAP) {
        return;
    }
    if (count - 1) as f32 * MIN_T_GAP > 1.0 {
        space_t_evenly(points);
        return;
    }
    for i in 1..count - 1 {
        let lo = points[i - 1].t() + MIN_T_GAP;
        *points[i].t_mut() = points[i].t().max(lo);
    }
    for i in (1..count - 1).rev() {
        let hi = points[i + 1].t() - MIN_T_GAP;
        *points[i].t_mut() = points[i].t().min(hi);
    }
}

pub fn remove_point(points: &mut Vec<ControlPoint>, index: usize) -> ControlPoint {
    let removed = points.remove(index);
    if index == 0 || index == points.len() {
        rescale_t_onto_unit_range(points);
    }
    removed
}

pub fn mirror_t(points: &mut [ControlPoint]) {
    if points.len() < 2 {
        return;
    }
    for cp in points.iter_mut() {
        *cp.t_mut() = 1.0 - *cp.t();
    }
}

pub fn t_drag_bounds(points: &[ControlPoint], index: usize) -> Option<(f32, f32)> {
    if index == 0 || index + 1 >= points.len() {
        return None;
    }
    let lo = points[index - 1].t() + MIN_T_GAP;
    let hi = points[index + 1].t() - MIN_T_GAP;
    (lo <= hi).then_some((lo, hi))
}

pub fn drag_t(points: &mut [ControlPoint], index: usize, target: f32) {
    if let Some((lo, hi)) = t_drag_bounds(points, index) {
        *points[index].t_mut() = target.clamp(lo, hi);
    }
}

pub fn t_at_pointer(pointer_x: f32, grab_offset_x: f32, strip_left: f32, strip_width: f32) -> f32 {
    (pointer_x - grab_offset_x - strip_left) / strip_width
}

pub fn drag_grab_offset_x(ui: &Ui, response: &egui::Response, anchor_x: f32) -> f32 {
    let grab_offset_id = response.id.with("grab_offset");
    if response.drag_started_by(egui::PointerButton::Primary) {
        let press_origin = ui
            .input(|i| i.pointer.press_origin())
            .or(response.interact_pointer_pos());
        if let Some(press_origin) = press_origin {
            ui.data_mut(|d| d.insert_temp(grab_offset_id, press_origin.x - anchor_x));
        }
    }
    ui.data(|d| d.get_temp::<f32>(grab_offset_id))
        .unwrap_or_default()
}

pub fn key_ts(points: &[ControlPoint]) -> Vec<f32> {
    if has_valid_t(points) {
        points.iter().map(|cp| *cp.t()).collect()
    } else {
        (0..points.len())
            .map(|i| evenly_spaced_t(i, points.len()))
            .collect()
    }
}

pub fn real_keys<V>(spline: &Spline<f32, V>, spline_mode: SplineMode) -> &[Key<f32, V>] {
    let keys = spline.keys();
    match spline_mode {
        SplineMode::HermiteBezier if keys.len() >= 2 => &keys[1..keys.len() - 1],
        SplineMode::HermiteBezier => &[],
        SplineMode::Linear | SplineMode::Bezier => keys,
    }
}

/// Samples `spline` (built by `spline_through_keys`) at `x` in [0, 1] across the real control points.
pub fn sample_spline_normalized<V: Interpolate<f32>>(
    spline: &Spline<f32, V>,
    spline_mode: SplineMode,
    x: f32,
) -> Option<V> {
    let keys = real_keys(spline, spline_mode);
    let (first, last) = (keys.first()?, keys.last()?);
    let t = first.t + x.clamp(0.0, 1.0) * (last.t - first.t);
    // CatmullRom needs keys[i + 2], so the sampler returns None exactly at the last real key.
    spline
        .clamped_sample(t)
        .or_else(|| (t >= last.t).then_some(last.value))
}

pub fn gradient_color_at(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    picker_space: PickerSpace,
    blend_space: BlendSpace,
) -> impl Fn(f32) -> Color32 {
    let gradient = Gradient::new(control_points, spline_mode, picker_space, blend_space);
    move |x| gradient.color_at(x).unwrap_or(Color32::TRANSPARENT)
}

pub fn bezier_handle(cp: &ControlPoint, slot: usize) -> ControlPointValue {
    let mut handle = *cp.val() + cp.tangents()[slot].unwrap_or_default();
    handle[0] = handle[0].clamp(0.0, 1.0);
    handle[1] = handle[1].clamp(0.0, 1.0);
    handle
}

pub fn convert_picker_space(points: &mut [ControlPoint], from: PickerSpace, to: PickerSpace) {
    if from == to {
        return;
    }
    if !from.is_same_space(to) {
        for cp in points.iter_mut() {
            convert_point(cp, from, to);
        }
    }
    clip_points_to_srgb(points, to);
    clip_handles_to_srgb(points, to);
}

pub fn clip_points_to_srgb(points: &mut [ControlPoint], space: PickerSpace) {
    if !space.clips_to_srgb() {
        return;
    }
    for cp in points {
        *cp.val_mut() = cp.val().clipped_to_srgb(space);
    }
}

pub fn clip_value_to_srgb(value: ControlPointValue, space: PickerSpace) -> ControlPointValue {
    if space.clips_to_srgb() {
        value.clipped_to_srgb(space)
    } else {
        value
    }
}

fn clip_handles_to_srgb(points: &mut [ControlPoint], space: PickerSpace) {
    if !space.clips_to_srgb() {
        return;
    }
    for cp in points {
        for slot in 0..2 {
            let handle = bezier_handle(cp, slot);
            let clipped = handle.clipped_to_srgb(space);
            if cp.tangents()[slot].is_some() && clipped != handle {
                cp.tangents_mut()[slot] = Some(clipped - *cp.val());
            }
        }
    }
}

fn convert_point(cp: &mut ControlPoint, from: PickerSpace, to: PickerSpace) {
    let old = *cp.val();
    let handles = [0, 1].map(|slot| bezier_handle(cp, slot));
    let new = value_in_picker_space(old, from, to);
    *cp.val_mut() = new;
    *cp.curvature_mut() = None;
    let value_hue_shift = shortest_hue_delta(old.h(), new.h());
    for (tangent, handle) in cp.tangents_mut().iter_mut().zip(handles) {
        let Some(tangent) = tangent else {
            continue;
        };
        let new_handle = value_in_picker_space(handle, from, to);
        let z_offset = match (from.is_z_hue(), to.is_z_hue()) {
            (true, true) => {
                let handle_hue_shift = shortest_hue_delta(handle.h(), new_handle.h());
                tangent.h() + handle_hue_shift - value_hue_shift
            }
            (false, true) => shortest_hue_delta(new.h(), new_handle.h()),
            (_, false) => new_handle.h() - new.h(),
        };
        *tangent =
            ControlPointValue::new(new_handle.s() - new.s(), new_handle.v() - new.v(), z_offset);
    }
}

fn value_in_picker_space(
    value: ControlPointValue,
    from: PickerSpace,
    to: PickerSpace,
) -> ControlPointValue {
    let linear = if to.can_leave_srgb() {
        value.unmapped_linear(from)
    } else {
        value.linear(from)
    };
    let [z, s, y] = linear_to_picker(to, linear);
    assert!(
        [z, s, y].iter().all(|c| c.is_finite()),
        "{value:?} {from:?} -> {to:?}: {z}, {s}, {y}"
    );
    let keeps_hue_of_gray = from.is_z_hue() && to.is_z_hue() && s <= 0.0;
    let z = if keeps_hue_of_gray {
        value.h()
    } else {
        to.z_on_bar(z)
    };
    ControlPointValue::new(s.clamp(0.0, 1.0), y.clamp(0.0, 1.0), z)
}

pub fn control_points_to_spline(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> Spline<f32, ControlPointValue> {
    let keys: Vec<[ControlPointValue; 3]> = control_points
        .iter()
        .map(|cp| [*cp.val(), bezier_handle(cp, 0), bezier_handle(cp, 1)])
        .collect();
    spline_through_keys(&key_ts(control_points), &keys, spline_mode)
}

fn polynomial_through<V>(keys: &[Key<f32, V>], t: f32) -> V
where
    V: Copy + Add<Output = V> + Sub<Output = V> + Mul<f32, Output = V>,
{
    let mut values: Vec<V> = keys.iter().map(|key| key.value).collect();
    for width in 1..keys.len() {
        for i in 0..keys.len() - width {
            let s = (t - keys[i].t) / (keys[i + width].t - keys[i].t);
            values[i] = values[i] + (values[i + 1] - values[i]) * s;
        }
    }
    values[0]
}

pub fn spline_through_keys<V>(
    ts: &[f32],
    values_and_handles: &[[V; 3]],
    spline_mode: SplineMode,
) -> Spline<f32, V>
where
    V: Copy + Add<Output = V> + Sub<Output = V> + Mul<f32, Output = V>,
{
    assert_eq!(ts.len(), values_and_handles.len());
    let keys_at_t = values_and_handles.iter().zip(ts.iter().copied());
    match spline_mode {
        SplineMode::Linear => Spline::from_vec(
            keys_at_t
                .map(|([value, ..], t)| Key::new(t, *value, Interpolation::Linear))
                .collect(),
        ),
        SplineMode::Bezier => Spline::from_vec(
            keys_at_t
                .map(|(&[value, handle_in, handle_out], t)| {
                    Key::new(t, value, Interpolation::StrokeBezier(handle_in, handle_out))
                })
                .collect(),
        ),
        SplineMode::HermiteBezier => {
            let n = ts.len();
            if n == 0 {
                return Spline::from_vec(Vec::new());
            }
            let (before, after) = if n >= 2 {
                (2.0 * ts[0] - ts[1], 2.0 * ts[n - 1] - ts[n - 2])
            } else {
                (ts[0] - 1.0, ts[0] + 1.0)
            };
            let catmull_rom_key = |t, value: V| Key::new(t, value, Interpolation::CatmullRom);
            let keys: Vec<Key<f32, V>> = keys_at_t
                .map(|([value, ..], t)| catmull_rom_key(t, *value))
                .collect();
            let end_count = n.min(3);
            let phantom_before = polynomial_through(&keys[..end_count], before);
            let phantom_after = polynomial_through(&keys[n - end_count..], after);
            Spline::from_vec(
                std::iter::once(catmull_rom_key(before, phantom_before))
                    .chain(keys)
                    .chain(std::iter::once(catmull_rom_key(after, phantom_after)))
                    .collect(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::test_support::{
        point_with_tangent, RUNAWAY_POINT, RUNAWAY_TANGENT,
    };
    use crate::datatypes::hsv_key_value::test_support::assert_close;
    use crate::math::hue_abs_distance;

    fn three_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.2), 0.5),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.2), 1.0),
        ]
    }

    #[test]
    fn hermite_sample_at_end_returns_last_point() {
        let cps = three_points();
        let spline = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(
            sample_spline_normalized(&spline, SplineMode::HermiteBezier, 1.0),
            Some(*cps[2].val())
        );
    }

    #[test]
    fn hermite_sample_at_start_returns_first_point() {
        let cps = three_points();
        let spline = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(
            sample_spline_normalized(&spline, SplineMode::HermiteBezier, 0.0),
            Some(*cps[0].val())
        );
    }

    #[test]
    fn linear_sample_covers_first_segment() {
        let cps = three_points();
        let spline = control_points_to_spline(&cps, SplineMode::Linear);
        assert_eq!(
            sample_spline_normalized(&spline, SplineMode::Linear, 0.0),
            Some(*cps[0].val())
        );
        let mid = sample_spline_normalized(&spline, SplineMode::Linear, 0.25).unwrap();
        assert!((mid[0] - 0.25).abs() < 1e-6 && (mid[1] - 0.25).abs() < 1e-6);
    }

    const SAMPLED_MODES: [SplineMode; 3] = [
        SplineMode::Linear,
        SplineMode::Bezier,
        SplineMode::HermiteBezier,
    ];

    fn points_with_hues(hues: &[f32]) -> Vec<ControlPoint> {
        hues.iter()
            .enumerate()
            .map(|(i, h)| ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, *h), i as f32))
            .collect()
    }

    #[test]
    fn flatten_control_points_unwraps_hue_across_boundary() {
        let hues = [0.9, 0.1, 0.3];
        let flattened = flatten_control_points(&points_with_hues(&hues));
        assert_eq!(flattened.len(), hues.len());
        for pair in flattened.windows(2) {
            assert!((pair[1].val()[2] - pair[0].val()[2]).abs() <= 0.5);
        }
        for (cp, hue) in flattened.iter().zip(hues) {
            assert!((cp.val()[2].rem_euclid(1.0) - hue).abs() < 1e-5);
        }
    }

    #[test]
    fn flatten_control_points_keeps_close_hues() {
        let cps = points_with_hues(&[0.2, 0.4]);
        let flattened = flatten_control_points(&cps);
        assert_eq!(flattened.len(), cps.len());
        for (flat, cp) in flattened.iter().zip(&cps) {
            assert_eq!(flat.val(), cp.val());
        }
    }

    #[test]
    fn flatten_control_points_unwraps_any_hue_sequence() {
        let sequences: [&[f32]; 4] = [
            &[2.3, 0.2, 0.4],
            &[-1.4, 0.3],
            &[0.9, 0.1, 0.9, 0.1],
            &[0.0, 0.4, 0.8, 0.2, 0.6, 0.0],
        ];
        for hues in sequences {
            let flattened = flatten_control_points(&points_with_hues(hues));
            assert_eq!(flattened.len(), hues.len());
            for pair in flattened.windows(2) {
                let step = pair[1].val()[2] - pair[0].val()[2];
                assert!(step.abs() <= 0.5 + 1e-5, "{hues:?}: step {step}");
            }
            for (cp, hue) in flattened.iter().zip(hues) {
                let flat_hue = cp.val()[2];
                assert!(
                    hue_abs_distance(flat_hue, *hue) < 1e-5,
                    "{hues:?}: {flat_hue} vs {hue}"
                );
            }
        }
    }

    #[test]
    fn two_point_hue_route_is_mode_independent() {
        let sample_hue = |mode, from: f32, to: f32, x: f32| {
            let flattened = flatten_control_points(&points_with_hues(&[from, to]));
            let spline = control_points_to_spline(&flattened, mode);
            sample_spline_normalized(&spline, mode, x).unwrap()[2]
        };
        let grid: Vec<f32> = (0..20).map(|i| i as f32 / 20.0).collect();
        for &a in &grid {
            for &b in &grid {
                let arc = shortest_hue_delta(a, b);
                let linear_mid = sample_hue(SplineMode::Linear, a, b, 0.5);
                for mode in SAMPLED_MODES {
                    for x in [0.25, 0.5, 0.75] {
                        let hue = sample_hue(mode, a, b, x);
                        let progress = shortest_hue_delta(a, hue) * arc.signum();
                        assert!(
                            (-1e-4..=arc.abs() + 1e-4).contains(&progress),
                            "{mode:?} ({a}, {b}) at {x}: {hue} is off the arc {arc}"
                        );
                        let reversed = sample_hue(mode, b, a, 1.0 - x);
                        assert!(
                            hue_abs_distance(hue, reversed) < 1e-4,
                            "{mode:?} ({a}, {b}) at {x}: {hue} vs reversed {reversed}"
                        );
                    }
                    let mid = sample_hue(mode, a, b, 0.5);
                    assert!(
                        hue_abs_distance(mid, linear_mid) < 1e-4,
                        "{mode:?} ({a}, {b}): mid {mid} vs linear {linear_mid}"
                    );
                }
            }
        }
    }

    #[test]
    fn sample_without_control_points_is_none() {
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&[], mode);
            for x in [0.0, 0.5, 1.0] {
                assert_eq!(
                    sample_spline_normalized(&spline, mode, x),
                    None,
                    "{mode:?} at {x}"
                );
            }
        }
    }

    #[test]
    fn sample_single_control_point_returns_it() {
        let cps = [ControlPoint::new_simple(
            ControlPointValue::new(0.3, 0.6, 0.7),
            0.0,
        )];
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&cps, mode);
            for x in [0.0, 0.5, 1.0] {
                assert_eq!(
                    sample_spline_normalized(&spline, mode, x),
                    Some(*cps[0].val()),
                    "{mode:?} at {x}"
                );
            }
        }
    }

    #[test]
    fn sample_two_control_points_hits_endpoints() {
        let cps = [
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.2, 0.4), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 0.8, 0.4), 1.0),
        ];
        for mode in [SplineMode::Linear, SplineMode::Bezier] {
            let spline = control_points_to_spline(&cps, mode);
            assert_eq!(
                sample_spline_normalized(&spline, mode, 0.0),
                Some(*cps[0].val()),
                "{mode:?}"
            );
            assert_eq!(
                sample_spline_normalized(&spline, mode, 1.0),
                Some(*cps[1].val()),
                "{mode:?}"
            );
        }
        let spline = control_points_to_spline(&cps, SplineMode::Linear);
        let mid = sample_spline_normalized(&spline, SplineMode::Linear, 0.5).unwrap();
        assert!((mid[0] - 0.5).abs() < 1e-6 && (mid[1] - 0.5).abs() < 1e-6);
    }

    fn with_key_t(
        spline: &Spline<f32, ControlPointValue>,
        key_t: &[f32],
    ) -> Spline<f32, ControlPointValue> {
        let mut keys = spline.keys().to_vec();
        for (key, t) in keys.iter_mut().zip(key_t) {
            key.t = *t;
        }
        Spline::from_vec(keys)
    }

    #[test]
    fn sample_with_preview_key_t_hits_every_key() {
        let cps = three_points();
        for mode in [SplineMode::Linear, SplineMode::Bezier] {
            for key_t in [[50.0, 150.0, 250.0], [50.0, 200.0, 350.0]] {
                let spline = with_key_t(&control_points_to_spline(&cps, mode), &key_t);
                for (x, cp) in [0.0, 0.5, 1.0].into_iter().zip(&cps) {
                    let sample = sample_spline_normalized(&spline, mode, x).unwrap();
                    for c in 0..3 {
                        assert!(
                            (sample[c] - cp.val()[c]).abs() < 1e-5,
                            "{mode:?} {key_t:?} at {x}: {sample:?}"
                        );
                    }
                }
            }
        }
    }

    fn points_with_t(ts: &[f32]) -> Vec<ControlPoint> {
        ts.iter()
            .enumerate()
            .map(|(i, &t)| {
                let i = i as f32;
                ControlPoint::new_simple(ControlPointValue::new(0.1 * i, 0.9 - 0.2 * i, 0.4), t)
            })
            .collect()
    }

    fn ts_of(points: &[ControlPoint]) -> Vec<f32> {
        points.iter().map(|cp| *cp.t()).collect()
    }

    fn assert_ts_near(points: &[ControlPoint], expected: &[f32]) {
        let actual = ts_of(points);
        assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < 1e-6, "{actual:?} vs {expected:?}");
        }
    }

    fn key_ts(spline: &Spline<f32, ControlPointValue>) -> Vec<f32> {
        spline.keys().iter().map(|k| k.t).collect()
    }

    #[test]
    fn has_valid_t_needs_pinned_ends_and_strictly_increasing_t() {
        let valid: [&[f32]; 4] = [&[], &[0.0], &[0.0, 1.0], &[0.0, 0.2, 0.9, 1.0]];
        for ts in valid {
            assert!(has_valid_t(&points_with_t(ts)), "{ts:?}");
        }
        let invalid: [&[f32]; 9] = [
            &[0.5],
            &[0.0, 0.0],
            &[0.0, 0.0, 0.0, 0.0],
            &[0.0, 1.0, 2.0],
            &[0.1, 1.0],
            &[0.0, 0.9],
            &[0.0, 0.6, 0.4, 1.0],
            &[0.0, 0.5, 0.5, 1.0],
            &[0.0, f32::NAN, 1.0],
        ];
        for ts in invalid {
            assert!(!has_valid_t(&points_with_t(ts)), "{ts:?}");
        }
    }

    #[test]
    fn normalize_t_spaces_invalid_t_evenly() {
        for (ts, expected) in [
            (
                &[0.0, 0.0, 0.0, 0.0][..],
                &[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0][..],
            ),
            (&[0.0, 1.0, 2.0], &[0.0, 0.5, 1.0]),
            (&[0.0, 0.7, 0.3, 1.0], &[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]),
            (&[0.7], &[0.0]),
        ] {
            let mut points = points_with_t(ts);
            assert!(normalize_t(&mut points), "{ts:?}");
            assert_eq!(ts_of(&points), expected, "{ts:?}");
            assert!(has_valid_t(&points), "{ts:?}");
        }
    }

    #[test]
    fn normalize_t_keeps_valid_t() {
        for ts in [&[][..], &[0.0], &[0.0, 0.2, 0.25, 1.0]] {
            let mut points = points_with_t(ts);
            assert!(!normalize_t(&mut points), "{ts:?}");
            assert_eq!(ts_of(&points), ts);
        }
    }

    #[test]
    fn space_t_evenly_is_valid_for_any_count() {
        for n in [0, 1, 2, 3, 7, 2000] {
            let mut points = points_with_t(&vec![0.0; n]);
            space_t_evenly(&mut points);
            assert!(has_valid_t(&points), "{n}");
        }
    }

    #[test]
    fn inserting_between_two_points_takes_the_middle_of_their_t() {
        let mut points = points_with_t(&[0.0, 0.2, 1.0]);
        insert_point(&mut points, 2, ControlPoint::default());
        assert_ts_near(&points, &[0.0, 0.2, 0.6, 1.0]);
        insert_point(&mut points, 1, ControlPoint::default());
        assert_ts_near(&points, &[0.0, 0.1, 0.2, 0.6, 1.0]);
    }

    #[test]
    fn inserting_at_an_end_pins_the_new_point_and_squeezes_the_others() {
        let third = 1.0 / 3.0;
        for (ts, index, expected) in [
            (&[0.0, 0.5, 1.0][..], 3, &[0.0, third, 2.0 * third, 1.0][..]),
            (&[0.0, 0.5, 1.0], 0, &[0.0, third, 2.0 * third, 1.0]),
            (
                &[0.0, 0.2, 1.0],
                3,
                &[0.0, 0.2 * 2.0 * third, 2.0 * third, 1.0],
            ),
            (
                &[0.0, 0.2, 1.0],
                0,
                &[0.0, third, third + 0.2 * 2.0 * third, 1.0],
            ),
            (&[0.0], 1, &[0.0, 1.0]),
            (&[0.0], 0, &[0.0, 1.0]),
            (&[], 0, &[0.0]),
        ] {
            let mut points = points_with_t(ts);
            insert_point(&mut points, index, ControlPoint::default());
            assert_ts_near(&points, expected);
            assert!(has_valid_t(&points), "{ts:?} at {index}");
        }
    }

    #[test]
    fn repeated_inserts_into_one_gap_keep_t_valid_and_the_min_gap() {
        let mut points = points_with_t(&[0.0, 0.3, 1.0]);
        for _ in 0..40 {
            let before_last = points.len() - 1;
            insert_point(&mut points, before_last, ControlPoint::default());
        }
        assert!(has_valid_t(&points), "{:?}", ts_of(&points));
        assert_eq!(*points[1].t(), 0.3);
        for pair in points.windows(2) {
            let gap = pair[1].t() - pair[0].t();
            assert!(gap >= MIN_T_GAP * 0.999, "{gap} in {:?}", ts_of(&points));
        }
    }

    #[test]
    fn inserting_puts_the_point_at_the_index() {
        let mut points = points_with_t(&[0.0, 1.0]);
        let inserted = ControlPoint::new_simple(ControlPointValue::new(0.7, 0.7, 0.7), 0.0);
        insert_point(&mut points, 1, inserted.clone());
        assert_eq!(points.len(), 3);
        assert_eq!(points[1].val(), inserted.val());
    }

    #[test]
    fn removing_an_end_point_rescales_the_rest_onto_zero_one() {
        for (ts, index, expected) in [
            (&[0.0, 0.25, 0.5, 1.0][..], 0, &[0.0, 1.0 / 3.0, 1.0][..]),
            (&[0.0, 0.25, 0.5, 1.0], 3, &[0.0, 0.5, 1.0]),
            (&[0.0, 0.25, 0.5, 1.0], 1, &[0.0, 0.5, 1.0]),
            (&[0.0, 1.0], 0, &[0.0]),
            (&[0.0, 1.0], 1, &[0.0]),
            (&[0.0], 0, &[]),
        ] {
            let mut points = points_with_t(ts);
            let removed = remove_point(&mut points, index);
            assert_eq!(*removed.t(), ts[index]);
            assert_ts_near(&points, expected);
            assert!(has_valid_t(&points), "{ts:?} at {index}");
        }
    }

    #[test]
    fn inserting_at_an_end_then_removing_that_point_restores_t() {
        let ts = [0.0, 0.1, 0.45, 1.0];
        for index in [0, ts.len()] {
            let mut points = points_with_t(&ts);
            insert_point(&mut points, index, ControlPoint::default());
            remove_point(&mut points, index);
            assert_ts_near(&points, &ts);
        }
    }

    #[test]
    fn mirror_t_maps_t_to_one_minus_t() {
        let mut points = points_with_t(&[0.0, 0.25, 1.0]);
        mirror_t(&mut points);
        assert_eq!(ts_of(&points), [1.0, 0.75, 0.0]);
        let mut single = points_with_t(&[0.0]);
        mirror_t(&mut single);
        assert_eq!(ts_of(&single), [0.0]);
    }

    #[test]
    fn t_drag_bounds_keep_the_min_gap_to_both_neighbours() {
        let points = points_with_t(&[0.0, 0.4, 0.6, 1.0]);
        assert_eq!(
            t_drag_bounds(&points, 1),
            Some((MIN_T_GAP, 0.6 - MIN_T_GAP))
        );
        assert_eq!(
            t_drag_bounds(&points, 2),
            Some((0.4 + MIN_T_GAP, 1.0 - MIN_T_GAP))
        );
    }

    #[test]
    fn t_drag_bounds_are_none_for_end_points_and_tight_neighbours() {
        let points = points_with_t(&[0.0, 0.4, 0.6, 1.0]);
        assert_eq!(t_drag_bounds(&points, 0), None);
        assert_eq!(t_drag_bounds(&points, 3), None);
        assert_eq!(t_drag_bounds(&points, 4), None);
        let tight = points_with_t(&[0.0, 0.5, 0.5005, 0.501, 1.0]);
        assert_eq!(t_drag_bounds(&tight, 2), None);
    }

    #[test]
    fn drag_t_clamps_a_middle_point_between_its_neighbours() {
        let mut points = points_with_t(&[0.0, 0.4, 0.6, 1.0]);
        drag_t(&mut points, 1, 0.3);
        assert_eq!(*points[1].t(), 0.3);
        drag_t(&mut points, 1, 0.9);
        assert_eq!(*points[1].t(), 0.6 - MIN_T_GAP);
        drag_t(&mut points, 2, -5.0);
        assert_eq!(*points[2].t(), *points[1].t() + MIN_T_GAP);
        assert!(has_valid_t(&points));
    }

    #[test]
    fn drag_t_leaves_end_points_pinned() {
        let mut points = points_with_t(&[0.0, 0.4, 1.0]);
        drag_t(&mut points, 0, 0.3);
        drag_t(&mut points, 2, 0.5);
        assert_eq!(ts_of(&points), [0.0, 0.4, 1.0]);
    }

    #[test]
    fn t_at_pointer_is_the_strip_fraction_without_the_grab_offset() {
        assert_eq!(t_at_pointer(150.0, 0.0, 100.0, 200.0), 0.25);
        assert_eq!(t_at_pointer(160.0, 10.0, 100.0, 200.0), 0.25);
        assert_eq!(t_at_pointer(50.0, 0.0, 100.0, 200.0), -0.25);
    }

    #[test]
    fn spline_keys_sit_at_point_t() {
        let cps = points_with_t(&[0.0, 0.25, 1.0]);
        for mode in [SplineMode::Linear, SplineMode::Bezier] {
            let spline = control_points_to_spline(&cps, mode);
            assert_eq!(key_ts(&spline), [0.0, 0.25, 1.0], "{mode:?}");
        }
        let hermite = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(key_ts(&hermite), [-0.25, 0.0, 0.25, 1.0, 1.75]);
    }

    #[test]
    fn hermite_phantoms_extend_the_polynomial_through_the_end_keys() {
        let line = |t: f32| ControlPointValue::new(0.2 + 0.5 * t, 0.7 - 0.4 * t, 0.95 + 0.2 * t);
        let quadratic = |t: f32| {
            ControlPointValue::new(
                0.2 + 0.9 * t - 0.6 * t * t,
                0.8 - 0.3 * t * t,
                0.9 + 0.3 * t - 0.1 * t * t,
            )
        };
        type Polynomial = fn(f32) -> ControlPointValue;
        let cases: [(&[f32], Polynomial); 3] = [
            (&[0.0, 1.0], line),
            (&[0.0, 0.3, 1.0], quadratic),
            (&[0.0, 0.25, 0.4, 1.0], quadratic),
        ];
        for (ts, polynomial) in cases {
            let keys: Vec<_> = ts.iter().map(|&t| [polynomial(t); 3]).collect();
            let spline = spline_through_keys(ts, &keys, SplineMode::HermiteBezier);
            for phantom in [&spline.keys()[0], &spline.keys()[spline.len() - 1]] {
                assert_close(phantom.value, polynomial(phantom.t));
            }
        }
    }

    #[test]
    fn hermite_single_point_phantoms_are_one_apart() {
        let hermite = control_points_to_spline(&points_with_t(&[0.0]), SplineMode::HermiteBezier);
        assert_eq!(key_ts(&hermite), [-1.0, 0.0, 1.0]);
    }

    #[test]
    fn spline_of_points_with_invalid_t_keys_them_evenly() {
        let cps = points_with_t(&[0.0, 0.0, 0.0]);
        let spline = control_points_to_spline(&cps, SplineMode::Linear);
        assert_eq!(key_ts(&spline), [0.0, 0.5, 1.0]);
        let hermite = control_points_to_spline(&cps, SplineMode::HermiteBezier);
        assert_eq!(key_ts(&hermite), [-0.5, 0.0, 0.5, 1.0, 1.5]);
    }

    #[test]
    fn sample_at_a_point_t_hits_that_point() {
        let cps = points_with_t(&[0.0, 0.1, 0.35, 1.0]);
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&cps, mode);
            for cp in &cps {
                assert_sample_near(
                    sample_spline_normalized(&spline, mode, *cp.t()),
                    *cp.val(),
                    &format!("{mode:?} at {}", cp.t()),
                );
            }
        }
    }

    #[test]
    fn gradient_color_at_hits_the_point_colors_at_their_t() {
        let cps = points_with_t(&[0.0, 0.3, 1.0]);
        let blend = BlendSpace::Picker;
        for space in PickerSpace::ALL {
            let color_at = gradient_color_at(&cps, SplineMode::Linear, space, blend);
            for cp in &cps {
                assert_eq!(color_at(*cp.t()), cp.val().color(space), "{space:?}");
            }
        }
        assert_eq!(
            gradient_color_at(&[], SplineMode::Linear, PickerSpace::Hsv, blend)(0.5),
            Color32::TRANSPARENT
        );
    }

    fn space_switch_points() -> Vec<ControlPoint> {
        [
            (0.8, 0.91, 0.05),
            (0.3, 0.6, 0.62),
            (1.0, 1.0, 0.67),
            (0.0, 0.52, 0.37),
            (0.6, 0.0, 0.2),
            (0.4, 1.0, 0.5),
            (0.5, 0.25, 0.95),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (s, v, h))| {
            let mut cp = ControlPoint::new_simple(ControlPointValue::new(s, v, h), i as f32 / 6.0);
            *cp.tangents_mut() = [
                Some(ControlPointValue::new(-0.1, 0.05, 0.02)),
                Some(ControlPointValue::new(0.15, -0.2, -0.03)),
            ];
            cp
        })
        .collect()
    }

    fn assert_linear_close(actual: [f32; 3], expected: [f32; 3], context: &str) {
        for c in 0..3 {
            assert!(
                (actual[c] - expected[c]).abs() < 1e-3,
                "{context}: {actual:?} != {expected:?}"
            );
        }
    }

    #[test]
    fn switching_space_keeps_key_colors() {
        for from in PickerSpace::ALL {
            for to in PickerSpace::ALL {
                let mut points = space_switch_points();
                let before: Vec<Color32> = points.iter().map(|cp| cp.val().color(from)).collect();
                convert_picker_space(&mut points, from, to);
                let after: Vec<Color32> = points.iter().map(|cp| cp.val().color(to)).collect();
                assert_eq!(after, before, "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn switching_space_drops_stored_curvature() {
        let mut points = space_switch_points();
        for cp in points.iter_mut() {
            *cp.curvature_mut() = Some(2.0);
        }
        convert_picker_space(&mut points, PickerSpace::Hsv, PickerSpace::Hsv);
        assert!(points.iter().all(|cp| cp.curvature() == Some(2.0)));
        convert_picker_space(&mut points, PickerSpace::Hsv, PickerSpace::Okhsl);
        assert!(points.iter().all(|cp| cp.curvature().is_none()));
    }

    #[test]
    fn switching_space_keeps_tangent_handle_colors() {
        for from in PickerSpace::ALL {
            for to in PickerSpace::ALL {
                let mut points = space_switch_points();
                let handles = |points: &[ControlPoint], space| -> Vec<[f32; 3]> {
                    points
                        .iter()
                        .flat_map(|cp| [0, 1].map(|slot| bezier_handle(cp, slot).linear(space)))
                        .collect()
                };
                let before = handles(&points, from);
                convert_picker_space(&mut points, from, to);
                for (i, (after, before)) in handles(&points, to).into_iter().zip(before).enumerate()
                {
                    assert_linear_close(after, before, &format!("{from:?} -> {to:?} #{i}"));
                }
            }
        }
    }

    #[test]
    fn switching_space_keeps_the_hue_of_gray_points() {
        let hue_spaces = || PickerSpace::ALL.into_iter().filter(|s| s.is_z_hue());
        for from in hue_spaces() {
            for to in hue_spaces() {
                for (s, y) in [(0.0, 0.5), (0.7, 0.0)] {
                    let mut points = [ControlPoint::new_simple(
                        ControlPointValue::new(s, y, 0.37),
                        0.0,
                    )];
                    convert_picker_space(&mut points, from, to);
                    assert_eq!(points[0].val().h(), 0.37, "{from:?} -> {to:?} ({s}, {y})");
                }
            }
        }
    }

    const OKLCH: PickerSpace = PickerSpace::Oklch {
        clip_to_srgb: false,
    };
    const OKLAB: PickerSpace = PickerSpace::Oklab {
        clip_to_srgb: false,
    };

    fn points_outside_srgb() -> Vec<ControlPoint> {
        let mut points = space_switch_points();
        points.retain(|cp| !cp.val().is_in_srgb(OKLCH));
        assert!(points.len() >= 2);
        points
    }

    #[test]
    fn switching_between_oklch_and_oklab_keeps_points_outside_srgb() {
        let mut points = points_outside_srgb();
        let before: Vec<[f32; 3]> = points
            .iter()
            .map(|cp| cp.val().unmapped_linear(OKLCH))
            .collect();
        convert_picker_space(&mut points, OKLCH, OKLAB);
        for (cp, before) in points.iter().zip(&before) {
            assert!(!cp.val().is_in_srgb(OKLAB), "{cp:?}");
            assert_linear_close(cp.val().unmapped_linear(OKLAB), *before, "Oklab");
        }
        convert_picker_space(&mut points, OKLAB, OKLCH);
        for (cp, before) in points.iter().zip(&before) {
            assert_linear_close(cp.val().unmapped_linear(OKLCH), *before, "OkLCh");
        }
    }

    #[test]
    fn switching_to_a_clipping_space_pulls_every_point_into_srgb() {
        for from in PickerSpace::ALL {
            for to in [OKLCH, OKLAB] {
                let to = to.with_clip_to_srgb(true);
                let mut points = points_outside_srgb();
                convert_picker_space(&mut points, from, to);
                for cp in &points {
                    assert!(cp.val().is_in_srgb(to), "{from:?} -> {to:?}: {cp:?}");
                    for slot in 0..2 {
                        let handle = bezier_handle(cp, slot);
                        assert!(handle.is_in_srgb(to), "{from:?} -> {to:?}: {cp:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn turning_clip_on_clamps_the_points_once_and_turning_it_off_keeps_them() {
        for space in [OKLCH, OKLAB] {
            let clipped = space.with_clip_to_srgb(true);
            let mut points = points_outside_srgb();
            convert_picker_space(&mut points, space, clipped);
            let expected: Vec<ControlPoint> = points_outside_srgb()
                .into_iter()
                .map(|mut cp| {
                    *cp.val_mut() = cp.val().clipped_to_srgb(clipped);
                    for slot in 0..2 {
                        let handle = bezier_handle(&cp, slot);
                        let clipped_handle = handle.clipped_to_srgb(clipped);
                        if clipped_handle != handle {
                            cp.tangents_mut()[slot] = Some(clipped_handle - *cp.val());
                        }
                    }
                    cp
                })
                .collect();
            assert_eq!(points, expected, "{space:?}");
            convert_picker_space(&mut points, clipped, space);
            assert_eq!(points, expected, "{space:?}");
        }
    }

    #[test]
    fn turning_clip_on_pulls_handles_into_srgb_and_leaves_the_others_bit_for_bit() {
        for space in [OKLCH, OKLAB] {
            let clipped = space.with_clip_to_srgb(true);
            let before = space_switch_points();
            let mut points = before.clone();
            convert_picker_space(&mut points, space, clipped);
            let mut clipped_handles = 0;
            for (cp, old) in points.iter().zip(&before) {
                for slot in 0..2 {
                    assert!(
                        bezier_handle(cp, slot).is_in_srgb(clipped),
                        "{space:?}: {cp:?}"
                    );
                    let was_inside =
                        old.val().is_in_srgb(space) && bezier_handle(old, slot).is_in_srgb(space);
                    if was_inside {
                        assert_eq!(cp.tangents()[slot], old.tangents()[slot], "{space:?}");
                    }
                    clipped_handles += usize::from(cp.tangents()[slot] != old.tangents()[slot]);
                }
            }
            assert!(clipped_handles > 0, "{space:?}");
        }
    }

    #[test]
    fn switching_to_the_same_space_changes_nothing() {
        for space in PickerSpace::ALL {
            let mut points = space_switch_points();
            convert_picker_space(&mut points, space, space);
            assert_eq!(points, space_switch_points(), "{space:?}");
        }
    }

    #[test]
    fn switching_space_keeps_tangent_hue_turns_across_zero() {
        for offset in [0.03, 1.5, -2.25] {
            let mut points = [point_with_tangent([0.8, 0.9, 0.99], 1, [0.0, 0.0, offset])];
            convert_picker_space(&mut points, PickerSpace::Hsv, PickerSpace::Okhsl);
            let new_offset = points[0].tangents()[1].unwrap().h();
            assert!(
                (new_offset - offset).abs() < 0.1,
                "{offset} -> {new_offset}"
            );
        }
    }

    #[test]
    fn moving_a_middle_t_moves_where_the_gradient_passes_its_point() {
        let early = points_with_t(&[0.0, 0.2, 1.0]);
        let late = points_with_t(&[0.0, 0.8, 1.0]);
        for mode in SAMPLED_MODES {
            let sample_s = |cps: &[ControlPoint]| {
                let spline = control_points_to_spline(cps, mode);
                sample_spline_normalized(&spline, mode, 0.5).unwrap()[0]
            };
            assert!(sample_s(&early) > sample_s(&late), "{mode:?}");
        }
    }

    fn assert_sample_near(
        sample: Option<ControlPointValue>,
        expected: ControlPointValue,
        context: &str,
    ) {
        let sample = sample.unwrap_or_else(|| panic!("{context}: no sample"));
        for c in 0..3 {
            assert!(
                (sample[c] - expected[c]).abs() < 1e-5,
                "{context}: {sample:?} vs {expected:?}"
            );
        }
    }

    fn distinct_points(n: usize) -> Vec<ControlPoint> {
        (0..n)
            .map(|i| {
                let i = i as f32;
                let val =
                    ControlPointValue::new((0.1 + 0.37 * i) % 1.0, (0.8 + 0.61 * i) % 1.0, 0.4);
                ControlPoint::new_simple(val, i)
            })
            .collect()
    }

    #[test]
    fn bezier_segment_uses_outgoing_slot_of_start_and_incoming_slot_of_end() {
        let cps = [
            point_with_tangent([0.0, 0.0, 0.0], 1, [0.3, 0.0, 0.0]),
            point_with_tangent([1.0, 1.0, 0.0], 0, [0.0, -0.2, 0.0]),
        ];
        let spline = control_points_to_spline(&cps, SplineMode::Bezier);
        assert_sample_near(
            sample_spline_normalized(&spline, SplineMode::Bezier, 0.5),
            ControlPointValue::new(0.6125, 0.425, 0.0),
            "bezier midpoint",
        );
    }

    #[test]
    fn sample_hits_every_key() {
        for n in [3, 4, 5, 7] {
            let cps = distinct_points(n);
            for mode in SAMPLED_MODES {
                let spline = control_points_to_spline(&cps, mode);
                for (i, cp) in cps.iter().enumerate() {
                    let x = i as f32 / (n - 1) as f32;
                    assert_sample_near(
                        sample_spline_normalized(&spline, mode, x),
                        *cp.val(),
                        &format!("{mode:?} n={n} key {i}"),
                    );
                }
            }
        }
    }

    #[test]
    fn sample_out_of_range_x_clamps_to_end_keys() {
        for n in [2, 3] {
            let cps = distinct_points(n);
            for mode in SAMPLED_MODES {
                let spline = control_points_to_spline(&cps, mode);
                for (x, key) in [(-1.0, 0), (-0.25, 0), (1.25, n - 1), (1.5, n - 1)] {
                    assert_sample_near(
                        sample_spline_normalized(&spline, mode, x),
                        *cps[key].val(),
                        &format!("{mode:?} n={n} at {x}"),
                    );
                }
            }
        }
    }

    #[test]
    fn hue_is_continuous_across_wrap_for_three_keys() {
        let flattened = flatten_control_points(&points_with_hues(&[0.9, 0.1, 0.3]));
        for mode in SAMPLED_MODES {
            let spline = control_points_to_spline(&flattened, mode);
            let hues: Vec<f32> = (0..=200)
                .map(|i| sample_spline_normalized(&spline, mode, i as f32 / 200.0).unwrap()[2])
                .collect();
            for pair in hues.windows(2) {
                assert!(
                    hue_abs_distance(pair[0], pair[1]) < 0.01,
                    "{mode:?}: jump {} -> {}",
                    pair[0],
                    pair[1]
                );
            }
            for hue in hues {
                assert!(
                    hue_abs_distance(hue, 0.1) <= 0.2 + 1e-4,
                    "{mode:?}: {hue} is off the arc 0.9 -> 0.3"
                );
            }
        }
    }

    #[test]
    fn hermite_through_collinear_evenly_spaced_keys_is_linear() {
        let two = [
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 0.2), 1.0),
        ];
        let three = three_points();
        for (cps, x, sv) in [
            (&two[..], 0.25, 0.25),
            (&two[..], 0.5, 0.5),
            (&two[..], 0.75, 0.75),
            (&three[..], 0.25, 0.25),
        ] {
            let spline = control_points_to_spline(cps, SplineMode::HermiteBezier);
            assert_sample_near(
                sample_spline_normalized(&spline, SplineMode::HermiteBezier, x),
                ControlPointValue::new(sv, sv, 0.2),
                &format!("{} points at {x}", cps.len()),
            );
        }
    }

    const PICKER_RECT: Rect = Rect {
        min: Pos2::new(100.0, 100.0),
        max: Pos2::new(300.0, 300.0),
    };
    const MIDDLE_CENTER: Pos2 = Pos2::new(200.0, 200.0);

    fn three_points_with_zero_middle_tangents() -> Vec<ControlPoint> {
        let mut cps = three_points();
        *cps[1].tangents_mut() = [Some(ControlPointValue::new(0.0, 0.0, 0.0)); 2];
        cps
    }

    fn run_frames(frames: Vec<Vec<egui::Event>>) -> ControlPointUiResult {
        run_frames_with_output(frames).0
    }

    fn run_frames_with_output(
        frames: Vec<Vec<egui::Event>>,
    ) -> (ControlPointUiResult, egui::FullOutput) {
        let ctx = egui::Context::default();
        let points = three_points_with_zero_middle_tangents();
        let mut result = ControlPointUiResult::default();
        let mut output = egui::FullOutput::default();
        for events in std::iter::once(Vec::new()).chain(frames) {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            };
            output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let parent = ui.allocate_rect(PICKER_RECT, Sense::click_and_drag());
                    result = ui_ordered_control_points(
                        ui,
                        &points,
                        Some(1),
                        false,
                        &parent,
                        true,
                        PickerSpace::Hsv,
                    );
                });
            });
        }
        (result, output)
    }

    fn button(pos: Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::default(),
        }
    }

    fn primary_drag(start: Pos2, dx: f32) -> Vec<Vec<egui::Event>> {
        let mut frames = vec![
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, egui::PointerButton::Primary, true)],
        ];
        for step in 1..=3 {
            let x = dx * step as f32 / 3.0;
            frames.push(vec![egui::Event::PointerMoved(start + Vec2::new(x, 0.0))]);
        }
        frames
    }

    #[test]
    fn dragging_selected_bezier_point_center_drags_the_point() {
        let result = run_frames(primary_drag(MIDDLE_CENTER, 20.0));
        assert!(result.dragged_point.is_some());
        assert!(result.dragged_tangent.is_none());
    }

    #[test]
    fn zero_bezier_tangent_is_grabbed_at_min_handle_length() {
        let drawn_handle = MIDDLE_CENTER - Vec2::new(2.0 * CONTROL_POINT_RADIUS, 0.0);
        let result = run_frames(primary_drag(drawn_handle, -21.0));
        assert!(result.dragged_point.is_none());
        assert!(result.dragged_tangent.is_some());
        assert_eq!(result.selected_tangent, Some(0));
        let offset = result.dragged_tangent_offset.unwrap();
        let min_length_sv = 2.0 * CONTROL_POINT_RADIUS / PICKER_RECT.width();
        assert!(
            offset.x < -min_length_sv && offset.y.abs() < 1e-6,
            "{offset:?}"
        );
    }

    #[test]
    fn right_clicking_selected_bezier_point_clicks_the_point() {
        let result = run_frames(vec![
            vec![egui::Event::PointerMoved(MIDDLE_CENTER)],
            vec![button(MIDDLE_CENTER, egui::PointerButton::Secondary, true)],
            vec![button(MIDDLE_CENTER, egui::PointerButton::Secondary, false)],
        ]);
        let (response, index) = result.hovering_control_point.expect("point hovered");
        assert_eq!(index, 1);
        assert!(response.clicked_by(egui::PointerButton::Secondary));
    }

    #[test]
    fn bezier_handle_clamps_runaway_tangent_to_square() {
        let cp = point_with_tangent(RUNAWAY_POINT, 1, RUNAWAY_TANGENT);
        assert_eq!(
            bezier_handle(&cp, 1),
            ControlPointValue::new(1.0, 1.0, 0.566)
        );
    }

    #[test]
    fn bezier_handle_in_range_is_point_plus_offset() {
        let offset = [0.1, -0.2, 0.05];
        let cp = point_with_tangent([0.4, 0.5, 0.2], 0, offset);
        assert_eq!(
            bezier_handle(&cp, 0),
            *cp.val() + ControlPointValue::from(offset)
        );
    }

    #[test]
    fn bezier_handle_clamps_negative_overshoot_to_zero() {
        let cp = point_with_tangent([0.2, 0.3, 0.5], 0, [-0.5, -0.9, 0.0]);
        assert_eq!(bezier_handle(&cp, 0), ControlPointValue::new(0.0, 0.0, 0.5));
    }

    #[test]
    fn bezier_handle_keeps_unwrapped_hue() {
        let cp = point_with_tangent([0.5, 0.5, 0.9], 1, [0.0, 0.0, 0.3]);
        let hue = bezier_handle(&cp, 1)[2];
        assert!((hue - 1.2).abs() < 1e-6, "{hue}");
    }

    #[test]
    fn bezier_spline_with_runaway_tangent_stays_in_square() {
        let cps = [
            point_with_tangent(RUNAWAY_POINT, 1, RUNAWAY_TANGENT),
            point_with_tangent([0.775, 0.592, 0.604], 0, [-0.175, 0.315, 0.0]),
        ];
        let spline = control_points_to_spline(&cps, SplineMode::Bezier);
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let sample = sample_spline_normalized(&spline, SplineMode::Bezier, x).unwrap();
            assert!(
                (0.0..=1.0).contains(&sample[0]) && (0.0..=1.0).contains(&sample[1]),
                "at {x}: {sample:?}"
            );
        }
    }

    #[test]
    fn control_point_outline_is_white_on_dark_fill() {
        let outline = control_point_outline(Color32::from_rgb(20, 20, 60), false);
        assert_eq!(outline.color, Color32::WHITE);
    }

    #[test]
    fn control_point_outline_is_black_on_bright_fill() {
        let outline = control_point_outline(Color32::from_rgb(240, 240, 200), false);
        assert_eq!(outline.color, Color32::BLACK);
    }

    #[test]
    fn emphasized_control_point_outline_is_wider() {
        let fill = Color32::from_rgb(20, 20, 60);
        assert!(control_point_outline(fill, true).width > control_point_outline(fill, false).width);
    }

    #[test]
    fn control_point_hint_is_one_based_and_mentions_right_click() {
        let hint = control_point_hint(1, 4);
        assert!(hint.contains("2 of 4"), "{hint}");
        assert!(hint.to_lowercase().contains("right-click"), "{hint}");
    }

    #[test]
    fn hovering_a_point_sets_grab_cursor() {
        let (_, output) =
            run_frames_with_output(vec![vec![egui::Event::PointerMoved(MIDDLE_CENTER)]]);
        assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::Grab);
    }

    #[test]
    fn hermite_sample_follows_affine_key_t() {
        let index_keyed = control_points_to_spline(&distinct_points(4), SplineMode::HermiteBezier);
        let affine_t: Vec<f32> = index_keyed
            .keys()
            .iter()
            .map(|k| 50.0 + 100.0 * k.t)
            .collect();
        let affine = with_key_t(&index_keyed, &affine_t);
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            let expected = sample_spline_normalized(&index_keyed, SplineMode::HermiteBezier, x);
            assert_sample_near(
                sample_spline_normalized(&affine, SplineMode::HermiteBezier, x),
                expected.unwrap(),
                &format!("x {x}"),
            );
        }
    }
}
