//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use crate::common::SplineMode;
use crate::datatypes::control_point::{ControlPoint, ControlPointValue};
use crate::ui_egui::control_points::{
    bezier_handle, control_points_to_spline, find_spline_max_t, flatten_control_points,
};
use crate::ui_egui::ui_common::contrast_color;
use eframe::egui::{self, Sense, Ui};
use eframe::emath;
use eframe::epaint::{Color32, Pos2, Rect, Shape, Stroke, Vec2};

const RIBBON_PX_PER_SAMPLE: f32 = 2.0;
const RIBBON_MIN_SAMPLES_PER_SEGMENT: usize = 8;
const RIBBON_MAX_SAMPLES_PER_SEGMENT: usize = 100;

fn ribbon_samples_per_segment(length_bound_px: f32) -> usize {
    if !length_bound_px.is_finite() || length_bound_px <= 0.0 {
        return RIBBON_MIN_SAMPLES_PER_SEGMENT;
    }
    ((length_bound_px / RIBBON_PX_PER_SAMPLE).ceil() as usize).clamp(
        RIBBON_MIN_SAMPLES_PER_SEGMENT,
        RIBBON_MAX_SAMPLES_PER_SEGMENT,
    )
}

fn ribbon_screen_pos(to_screen: &emath::RectTransform, value: ControlPointValue) -> Pos2 {
    let pos = value.pos2();
    to_screen * Pos2::new(pos.x, 1.0 - pos.y)
}

fn ribbon_segment_length_bound_px(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    to_screen: &emath::RectTransform,
) -> f32 {
    let edge = |a: ControlPointValue, b: ControlPointValue| {
        ribbon_screen_pos(to_screen, a).distance(ribbon_screen_pos(to_screen, b))
    };
    let longest = control_points
        .windows(2)
        .map(|pair| {
            let (p0, p1) = (&pair[0], &pair[1]);
            match spline_mode {
                SplineMode::Bezier => {
                    let (h0, h1) = (bezier_handle(p0, 1), bezier_handle(p1, 0));
                    edge(*p0.val(), h0) + edge(h0, h1) + edge(h1, *p1.val())
                }
                SplineMode::Linear | SplineMode::HermiteBezier => edge(*p0.val(), *p1.val()),
            }
        })
        .fold(0.0, f32::max);
    match spline_mode {
        // Catmull-Rom tangents are at most the longest neighbouring chord; arc <= chord + 2/3 (|m0| + |m1|).
        SplineMode::HermiteBezier => 7.0 / 3.0 * longest,
        SplineMode::Linear | SplineMode::Bezier => longest,
    }
}

pub fn generate_spline_points(
    control_points: &[ControlPoint],
    spline_mode: SplineMode,
    samples_per_segment: usize,
) -> Vec<ControlPointValue> {
    assert!(samples_per_segment > 0);
    if control_points.len() <= 1 {
        return Vec::new();
    }

    let spline = control_points_to_spline(control_points, spline_mode);
    let steps = find_spline_max_t(&spline).round() as usize * samples_per_segment;
    let mut spline_samples: Vec<ControlPointValue> = (0..steps)
        .filter_map(|i| spline.clamped_sample(i as f32 / samples_per_segment as f32))
        .chain(spline.keys().last().map(|key| key.value))
        .collect();
    spline_samples.dedup();
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
    let length_bound_px =
        ribbon_segment_length_bound_px(&flattened_points, spline_mode, &to_screen);
    let spline_points = generate_spline_points(
        &flattened_points,
        spline_mode,
        ribbon_samples_per_segment(length_bound_px),
    );

    let control_point_radius = 8.0;
    let mut halos = Vec::with_capacity(spline_points.len());
    let mut fills = Vec::with_capacity(spline_points.len());
    for pair in spline_points.windows(2) {
        let first = pair[0];
        let next = pair[1];

        let segment = [
            ribbon_screen_pos(&to_screen, first),
            ribbon_screen_pos(&to_screen, next),
        ];
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

    const ALL_SPLINE_MODES: [SplineMode; 3] = [
        SplineMode::Linear,
        SplineMode::Bezier,
        SplineMode::HermiteBezier,
    ];

    fn zigzag_points(n: usize) -> Vec<ControlPoint> {
        (0..n)
            .map(|i| {
                let s = i as f32 / (n - 1) as f32;
                let v = if i % 2 == 0 { 0.2 } else { 0.8 };
                let mut cp = ControlPoint::new_simple(
                    ControlPointValue::new(s, v, 0.1 + 0.15 * i as f32),
                    s,
                );
                *cp.tangents_mut() = [
                    Some(ControlPointValue::new(-0.1, 0.05, 0.0)),
                    Some(ControlPointValue::new(0.1, -0.05, 0.0)),
                ];
                cp
            })
            .collect()
    }

    fn point_with_tangents(
        s: f32,
        v: f32,
        tangents: [Option<ControlPointValue>; 2],
    ) -> ControlPoint {
        let mut cp = ControlPoint::new_simple(ControlPointValue::new(s, v, 0.4), s);
        *cp.tangents_mut() = tangents;
        cp
    }

    fn unit_to_square(size: f32) -> emath::RectTransform {
        emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, Vec2::splat(1.0)),
            Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::splat(size)),
        )
    }

    #[test]
    fn spline_points_empty_for_fewer_than_two_points() {
        let single = vec![ControlPoint::new_simple(
            ControlPointValue::new(0.5, 0.5, 0.5),
            0.0,
        )];
        for mode in ALL_SPLINE_MODES {
            assert!(generate_spline_points(&[], mode, 4).is_empty());
            assert!(generate_spline_points(&single, mode, 4).is_empty());
        }
    }

    #[test]
    fn spline_points_start_at_first_and_end_at_last_point() {
        for mode in ALL_SPLINE_MODES {
            for n in 2..=6 {
                let points = zigzag_points(n);
                let samples = generate_spline_points(&points, mode, 4);
                assert_eq!(samples.first(), Some(points[0].val()), "{mode:?} n={n}");
                assert_eq!(samples.last(), Some(points[n - 1].val()), "{mode:?} n={n}");
            }
        }
    }

    #[test]
    fn spline_points_have_no_consecutive_duplicates() {
        for mode in ALL_SPLINE_MODES {
            for n in 2..=6 {
                for samples_per_segment in [1, 4, 100] {
                    let samples =
                        generate_spline_points(&zigzag_points(n), mode, samples_per_segment);
                    assert!(
                        samples.windows(2).all(|pair| pair[0] != pair[1]),
                        "{mode:?} n={n} samples_per_segment={samples_per_segment}"
                    );
                }
            }
        }
    }

    #[test]
    fn linear_spline_points_take_samples_per_segment() {
        let samples = generate_spline_points(&zigzag_points(3), SplineMode::Linear, 2);
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn monotone_spline_points_stay_monotone_and_in_range() {
        for mode in [SplineMode::Linear, SplineMode::HermiteBezier] {
            for n in 2..=6 {
                let points: Vec<ControlPoint> = (0..n)
                    .map(|i| {
                        let x = i as f32 / (n - 1) as f32;
                        ControlPoint::new_simple(ControlPointValue::new(x, x, 0.3), x)
                    })
                    .collect();
                let samples = generate_spline_points(&points, mode, 16);
                for pair in samples.windows(2) {
                    assert!(pair[1][0] >= pair[0][0] - 1e-5, "{mode:?} n={n} {pair:?}");
                }
                for sample in &samples {
                    assert!(
                        (0..3).all(|i| (0.0..=1.0).contains(&sample[i])),
                        "{mode:?} n={n} {sample:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn ribbon_samples_per_segment_grows_with_length_and_is_clamped() {
        let counts: Vec<usize> = (0..=400)
            .map(|px| ribbon_samples_per_segment(px as f32))
            .collect();
        assert!(counts.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(counts[1], 8);
        assert_eq!(counts[51], 26);
        assert_eq!(counts[400], 100);
        assert_eq!(ribbon_samples_per_segment(1e9), 100);
    }

    #[test]
    fn ribbon_samples_per_segment_is_minimum_for_invalid_length() {
        for length in [0.0, -3.0, f32::NAN, f32::INFINITY] {
            assert_eq!(ribbon_samples_per_segment(length), 8, "{length}");
        }
    }

    #[test]
    fn linear_length_bound_is_screen_chord() {
        let points = [
            point_with_tangents(0.0, 0.0, [None, None]),
            point_with_tangents(1.0, 1.0, [None, None]),
        ];
        let bound =
            ribbon_segment_length_bound_px(&points, SplineMode::Linear, &unit_to_square(100.0));
        assert!((bound - 141.42136).abs() < 1e-3, "{bound}");
    }

    #[test]
    fn bezier_length_bound_is_clamped_control_polygon() {
        let up = Some(ControlPointValue::new(0.0, 0.9, 0.0));
        let points = [
            point_with_tangents(0.0, 0.5, [None, up]),
            point_with_tangents(1.0, 0.5, [up, None]),
        ];
        let bound =
            ribbon_segment_length_bound_px(&points, SplineMode::Bezier, &unit_to_square(100.0));
        assert!((bound - 200.0).abs() < 1e-3, "{bound}");
    }

    #[test]
    fn hermite_length_bound_is_seven_thirds_of_longest_chord() {
        let points = [
            point_with_tangents(0.0, 0.5, [None, None]),
            point_with_tangents(0.3, 0.5, [None, None]),
            point_with_tangents(1.0, 0.5, [None, None]),
        ];
        let bound = ribbon_segment_length_bound_px(
            &points,
            SplineMode::HermiteBezier,
            &unit_to_square(100.0),
        );
        assert!((bound - 70.0 * 7.0 / 3.0).abs() < 1e-3, "{bound}");
    }
}
