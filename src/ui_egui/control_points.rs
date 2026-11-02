use ecolor::{Color32, HsvaGamma};
use eframe::{
    egui::{self, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    emath::RectTransform,
    epaint::PathShape,
};
use splines::{Interpolation, Key, Spline};

use crate::{
    common::SplineMode,
    datatypes::control_point::{ControlPoint, ControlPointValue},
    ui_egui::tangents::ui_control_point_tangents,
};

#[derive(Default)]
pub struct ControlPointUiResult {
    pub dragged_point: Option<egui::Response>,
    pub selected_index: Option<usize>,
    pub hovering_control_point: Option<(egui::Response, usize)>,
    pub selected_tangent: Option<usize>,
    pub dragged_tangent: Option<egui::Response>,
}

pub fn control_point_pos(cp: &ControlPoint) -> Pos2 {
    Pos2::new(
        cp.val()[0].clamp(0.0, 1.0),
        1.0 - cp.val()[1].clamp(0.0, 1.0),
    )
}

fn to_screen_pos(to_screen: &RectTransform, cp: &ControlPoint) -> Pos2 {
    to_screen.transform_pos(control_point_pos(cp))
}

pub fn ui_ordered_control_points(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    marked_control_point_index: Option<usize>,
    _is_middle_interpolated: bool,
    parent_response: &egui::Response,
    show_bezier_tangents: bool,
) -> ControlPointUiResult {
    use egui::PointerButton::Primary;

    const SHOW_LINEAR_LINE: bool = false;

    const FILL_RADIUS_SCALE: f32 = 1.8;
    const TANGENT_RADIUS_SCALE: f32 = 0.7;
    const ACTIVE_LINE_ALPHA: f32 = 0.25;
    const INACTIVE_LINE_ALPHA: f32 = 0.002;

    if control_points.is_empty() {
        return ControlPointUiResult::default();
    }

    let to_screen = RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0)),
        parent_response.rect,
    );

    let control_point_radius = 8.0;
    let control_point_draw_size = Vec2::splat(2.0 * control_point_radius);

    let inactive_stroke = ui.style().noninteractive().fg_stroke;
    let active_stroke = ui.style().interact(parent_response).fg_stroke;

    let mut selected_index = marked_control_point_index;
    let mut tangent_selected_index = None;
    let mut hovering_control_point = None;
    let mut dragged_point_response = None;
    let mut dragged_tangent_response = None;

    let control_point_shapes_fill: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let point_in_screen = to_screen_pos(&to_screen, cp);

            let rect = Rect::from_center_size(point_in_screen, control_point_draw_size);
            let response = ui.interact(rect, parent_response.id.with(i), Sense::click_and_drag());

            if dragged_point_response.is_none()
                && (response.dragged_by(Primary) || response.clicked_by(Primary))
            {
                selected_index = Some(i);
                dragged_point_response = Some(response.clone());
            }

            if hovering_control_point.is_none() && response.hovered() {
                hovering_control_point = Some((response, i));
            }

            let color = HsvaGamma {
                h: cp.val()[2],
                s: cp.val()[0],
                v: cp.val()[1],
                a: 1.0,
            };

            Shape::circle_filled(
                point_in_screen,
                FILL_RADIUS_SCALE * control_point_radius,
                color,
            )
        })
        .collect();

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
                selected_index == Some(i),
                &to_screen,
                parent_response,
                control_point_draw_size,
                control_point_radius,
                inactive_stroke,
                &mut tangent_shapes,
                &mut tangent_paths,
            );

            if dragged_tangent_response.is_none() {
                dragged_tangent_response = result.dragged_tangent;
                tangent_selected_index = result.selected_tangent;
            }

            if result.selected_by_tangent {
                selected_index = Some(i);
            }
        }
    }

    let control_point_shapes: Vec<Shape> = control_points
        .iter()
        .enumerate()
        .map(|(i, cp)| {
            let point = to_screen_pos(&to_screen, cp);

            if i == 0 || i == control_points.len() - 1 {
                Shape::rect_stroke(
                    Rect::from_center_size(point, Vec2::splat(control_point_radius)),
                    0.0,
                    active_stroke,
                    egui::StrokeKind::Middle,
                )
            } else {
                Shape::circle_stroke(point, control_point_radius, active_stroke)
            }
        })
        .collect();

    if SHOW_LINEAR_LINE {
        let points: Vec<Pos2> = control_points
            .iter()
            .map(|cp| to_screen_pos(&to_screen, cp))
            .collect();

        ui.painter().add(PathShape::line(
            points,
            Stroke::new(1.0, Color32::RED.linear_multiply(0.25)),
        ));
    }

    ui.painter().extend(control_point_shapes_fill);
    ui.painter().extend(control_point_shapes);
    ui.painter().extend(tangent_shapes);
    ui.painter()
        .extend(tangent_paths.into_iter().map(Into::into));

    if let Some(marked) = marked_control_point_index {
        let point = to_screen_pos(&to_screen, &control_points[marked]);
        ui.painter().add(Shape::rect_stroke(
            Rect::from_center_size(point, Vec2::splat(control_point_radius * 0.5)),
            0.0,
            active_stroke,
            egui::StrokeKind::Middle,
        ));
    }

    ControlPointUiResult {
        dragged_point: dragged_point_response,
        selected_index,
        hovering_control_point,
        selected_tangent: tangent_selected_index,
        dragged_tangent: dragged_tangent_response,
    }
}

pub fn flatten_control_points(control_points: &[ControlPoint]) -> Vec<ControlPoint> {
    let mut flattened: Vec<ControlPoint> = Vec::with_capacity(control_points.len());

    for (i, cp) in control_points.iter().enumerate() {
        if i == 0 {
            flattened.push(cp.clone());
            continue;
        }

        let prev_hue = flattened[i - 1].val()[2];
        let hue_diff = cp.val()[2] - prev_hue;

        let cp_clone = cp.clone();

        if hue_diff.abs() > 0.5 {
            // Adjust all previous hues by ±1 to smooth wraparound
            let adjustment = if hue_diff > 0.0 { 1.0 } else { -1.0 };
            for prev_cp in &mut flattened {
                prev_cp.val_mut()[2] += adjustment;
            }
        }

        flattened.push(cp_clone);
    }

    flattened
}

pub fn find_spline_max_t(spline: &Spline<f32, ControlPointValue>) -> f32 {
    let vec_of_t_values: Vec<f32> = spline.into_iter().map(|k| k.t).collect();
    let max_t = vec_of_t_values
        .into_iter()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);
    max_t
}

/// Samples `spline` (built by `control_points_to_spline`) at `x` in [0, 1] across the real control points.
pub fn sample_spline_normalized(
    spline: &Spline<f32, ControlPointValue>,
    spline_mode: SplineMode,
    x: f32,
) -> Option<ControlPointValue> {
    match spline_mode {
        SplineMode::HermiteBezier => {
            // Real keys are at 1..=n, with phantom endpoint duplicates at 0 and n + 1.
            let n = spline.len().checked_sub(2)?;
            if n == 0 {
                return None;
            }
            let t = 1.0 + x * (n - 1) as f32;
            // CatmullRom needs keys[i + 2], so the sampler returns None exactly at the last real key.
            spline
                .clamped_sample(t)
                .or_else(|| (t >= n as f32).then(|| spline.keys()[n].value))
        }
        _ => spline.clamped_sample(x * find_spline_max_t(spline)),
    }
}

pub fn control_points_to_spline(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
) -> Spline<f32, ControlPointValue> {
    match spline_mode {
        SplineMode::Linear => Spline::from_vec(
            control_points
                .iter()
                .enumerate()
                .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::Linear))
                .collect(),
        ),
        SplineMode::Bezier => Spline::from_vec(
            control_points
                .iter()
                .enumerate()
                .map(|(index, e)| {
                    Key::new(
                        index as f32,
                        *e.val(),
                        Interpolation::StrokeBezier(
                            *control_points[index].val()
                                + control_points[index].tangents()[0].unwrap_or_default(),
                            *control_points[index].val()
                                + control_points[index].tangents()[1].unwrap_or_default(),
                        ),
                    )
                })
                .collect(),
        ),
        SplineMode::HermiteBezier => {
            let mut catmul_rom_spline_vec = control_points.to_vec();
            if control_points.len() >= 1 {
                catmul_rom_spline_vec.insert(0, control_points.first().unwrap().clone());
            }
            if control_points.len() >= 1 {
                catmul_rom_spline_vec.push(control_points.last().unwrap().clone());
            }

            let new_spline = Spline::from_vec(
                catmul_rom_spline_vec
                    .iter()
                    .enumerate()
                    .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::CatmullRom))
                    .collect(),
            );

            new_spline
        }
        SplineMode::Polynomial => todo!(),
        _ => {
            log::info!("Not Implemented...");
            Spline::from_vec(
                control_points
                    .iter()
                    .enumerate()
                    .map(|(index, e)| Key::new(index as f32, *e.val(), Interpolation::Linear))
                    .collect(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
