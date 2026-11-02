//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use crate::common::SplineMode;
use crate::datatypes::control_point::{ControlPoint, ControlPointValue};
use crate::ui_egui::control_points::{
    control_points_to_spline, find_spline_max_t, flatten_control_points,
};
use crate::ui_egui::ui_common::contrast_color;
use eframe::egui::{self, Sense, Ui};
use eframe::emath;
use eframe::epaint::{Color32, Pos2, Rect, Shape, Stroke, Vec2};

pub fn generate_spline_points_with_distance(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    t_distance: f32,
) -> Vec<ControlPointValue> {
    let mut spline_samples = Vec::new();

    if control_points.len() <= 1 {
        return spline_samples;
    }

    let spline = control_points_to_spline(&control_points, spline_mode);
    let spline_max_t = find_spline_max_t(&spline) as f32;
    let mut curr_t = 0.0;
    while curr_t <= spline_max_t {
        let spline_sample = spline.clamped_sample(curr_t);

        match spline_sample {
            Some(key) => spline_samples.push(key),
            None => {}
        }

        curr_t += t_distance;
    }

    spline_samples.push(
        spline
            .clamped_sample(spline_max_t)
            .expect("clamped sample at the last key t of a non-empty spline is always Some"),
    );

    spline_samples
}

pub fn ui_ordered_spline_gradient(
    ui: &mut Ui,
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    parent_response: &egui::Response,
) -> Option<egui::Response> {
    let num_control_points = control_points.len();
    if num_control_points <= 1 {
        return None;
    }

    let response: egui::Response = ui.interact(
        parent_response.rect,
        parent_response.id.with(190124502),
        Sense::focusable_noninteractive(),
    );

    let to_screen = emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0)),
        response.rect,
    );

    let flattened_points = flatten_control_points(control_points);
    let spline_points =
        generate_spline_points_with_distance(&flattened_points[..], spline_mode, 0.01);

    let control_point_radius = 8.0;
    let mut halos = Vec::with_capacity(spline_points.len());
    let mut fills = Vec::with_capacity(spline_points.len());
    for pair in spline_points.windows(2) {
        let first = pair[0];
        let next = pair[1];

        let point_first = first.pos2();
        let point_next = next.pos2();
        let point_in_screen_first = to_screen * Pos2::new(point_first.x, 1.0 - point_first.y);
        let point_in_screen_next = to_screen * Pos2::new(point_next.x, 1.0 - point_next.y);

        let segment = [point_in_screen_first, point_in_screen_next];
        let fill = Stroke::new(control_point_radius * 1.6, first.color());
        halos.push(Shape::line_segment(
            segment,
            ribbon_halo_stroke(fill.color, fill.width),
        ));
        fills.push(Shape::line_segment(segment, fill));
    }

    ui.painter()
        .with_clip_rect(response.rect)
        .extend(halos.into_iter().chain(fills));

    Some(response)
}

const RIBBON_HALO_EDGE: f32 = 1.25;
const RIBBON_HALO_OPACITY: f32 = 0.6;

pub(crate) fn ribbon_halo_stroke(fill: Color32, fill_width: f32) -> Stroke {
    Stroke::new(
        fill_width + 2.0 * RIBBON_HALO_EDGE,
        contrast_color(fill).gamma_multiply(RIBBON_HALO_OPACITY),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DARK_FILL: Color32 = Color32::from_rgb(20, 20, 60);
    const BRIGHT_FILL: Color32 = Color32::from_rgb(240, 240, 200);

    #[test]
    fn ribbon_halo_is_wider_than_fill() {
        assert!(ribbon_halo_stroke(DARK_FILL, 12.8).width > 12.8);
    }

    #[test]
    fn ribbon_halo_is_translucent_white_on_dark_fill() {
        let halo = ribbon_halo_stroke(DARK_FILL, 12.8);
        assert!(halo.color.a() > 0);
        assert_eq!(halo.color, Color32::WHITE.gamma_multiply_u8(halo.color.a()));
    }

    #[test]
    fn ribbon_halo_is_translucent_black_on_bright_fill() {
        let halo = ribbon_halo_stroke(BRIGHT_FILL, 12.8);
        assert!(halo.color.a() > 0);
        assert_eq!(halo.color, Color32::BLACK.gamma_multiply_u8(halo.color.a()));
    }
}
