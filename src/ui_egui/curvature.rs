use eframe::egui::{self, Color32, Pos2, Shape, Stroke, Ui, Vec2};

use crate::datatypes::control_point::ControlPoint;
use crate::ui_egui::control_points::{bezier_handle, control_point_pos, value_pos};

const SAMPLES_PER_SEGMENT: usize = 64;
const CURVATURE_SCALE: f32 = 100.0;
const MAX_COMB_LENGTH: f32 = 45.0;
const MIN_DERIVATIVE_LENGTH: f32 = 1.0e-3;
const MIN_TOOTH_LENGTH: f32 = 0.25;
const COMB_STROKE_WIDTH: f32 = 1.0;
const COMB_ALPHA: u8 = 120;
const COMB_OUTLINE_STROKE_WIDTH: f32 = 1.0;
const COMB_OUTLINE_ALPHA: u8 = 210;

#[derive(Clone, Copy, Debug)]
struct Cubic {
    p0: Pos2,
    p1: Pos2,
    p2: Pos2,
    p3: Pos2,
}

impl Cubic {
    fn eval(&self, t: f32) -> Pos2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;

        let p = self.p0.to_vec2() * (u * u * u)
            + self.p1.to_vec2() * (3.0 * u * u * t)
            + self.p2.to_vec2() * (3.0 * u * t * t)
            + self.p3.to_vec2() * (t * t * t);

        p.to_pos2()
    }

    fn derivative(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;

        (self.p1 - self.p0) * (3.0 * u * u)
            + (self.p2 - self.p1) * (6.0 * u * t)
            + (self.p3 - self.p2) * (3.0 * t * t)
    }

    fn second_derivative(&self, t: f32) -> Vec2 {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;

        let a = self.p2.to_vec2() - self.p1.to_vec2() * 2.0 + self.p0.to_vec2();
        let b = self.p3.to_vec2() - self.p2.to_vec2() * 2.0 + self.p1.to_vec2();

        a * (6.0 * u) + b * (6.0 * t)
    }
}

fn curvature(cubic: &Cubic, t: f32) -> Option<f32> {
    let d1 = cubic.derivative(t);
    let d2 = cubic.second_derivative(t);

    let speed_squared = d1.length_sq();
    if !speed_squared.is_finite() {
        return None;
    }
    let speed = speed_squared.sqrt();
    if !speed.is_finite() || speed < MIN_DERIVATIVE_LENGTH {
        return None;
    }

    let cross = d1.x * d2.y - d1.y * d2.x;
    let denominator = speed_squared * speed;
    if !cross.is_finite() || !denominator.is_finite() || denominator <= f32::EPSILON {
        return None;
    }

    let k = -cross / denominator;
    k.is_finite().then_some(k)
}

fn curvature_normal(cubic: &Cubic, t: f32) -> Option<Vec2> {
    let d1 = cubic.derivative(t);
    let d2 = cubic.second_derivative(t);

    let speed_squared = d1.length_sq();
    if !speed_squared.is_finite() {
        return None;
    }
    let speed = speed_squared.sqrt();
    if !speed.is_finite() || speed < MIN_DERIVATIVE_LENGTH {
        return None;
    }

    let cross = d1.x * d2.y - d1.y * d2.x;
    if !cross.is_finite() || cross.abs() < f32::EPSILON {
        return None;
    }

    let tangent = d1 / speed;
    let left_normal = Vec2::new(-tangent.y, tangent.x);
    Some(left_normal * -cross.signum())
}

fn normalized_to_screen(rect: egui::Rect, position: Pos2) -> Pos2 {
    Pos2::new(
        rect.left() + position.x * rect.width(),
        rect.top() + position.y * rect.height(),
    )
}

fn cubic_for_segment(start: &ControlPoint, end: &ControlPoint, rect: egui::Rect) -> Cubic {
    Cubic {
        p0: normalized_to_screen(rect, control_point_pos(start)),
        p1: normalized_to_screen(rect, value_pos(&bezier_handle(start, 1))),
        p2: normalized_to_screen(rect, value_pos(&bezier_handle(end, 0))),
        p3: normalized_to_screen(rect, control_point_pos(end)),
    }
}

fn curvature_to_comb_length(k: f32) -> f32 {
    if !k.is_finite() || k == 0.0 {
        return 0.0;
    }
    let scaled_curvature = k.abs() * CURVATURE_SCALE;
    scaled_curvature.atan() * (2.0 * MAX_COMB_LENGTH / std::f32::consts::PI)
}

pub fn draw_curvature_comb(ui: &mut Ui, rect: egui::Rect, control_points: &[ControlPoint]) {
    if control_points.len() < 2 {
        return;
    }

    let tooth_color = Color32::from_rgba_unmultiplied(255, 255, 255, COMB_ALPHA);
    let outline_color = Color32::from_rgba_unmultiplied(255, 255, 255, COMB_OUTLINE_ALPHA);
    let mut shapes = Vec::with_capacity((control_points.len() - 1) * (SAMPLES_PER_SEGMENT + 1) * 2);
    // Index order, not t order: spline keys are placed at t = index (see control_points_to_spline).
    for (segment_index, pair) in control_points.windows(2).enumerate() {
        let start = &pair[0];
        let end = &pair[1];

        let cubic = cubic_for_segment(start, end, rect);

        let mut outline_points = Vec::with_capacity(SAMPLES_PER_SEGMENT + 1);

        for sample_index in 0..=SAMPLES_PER_SEGMENT {
            if segment_index > 0 && sample_index == 0 {
                continue;
            }

            let t = sample_index as f32 / SAMPLES_PER_SEGMENT as f32;
            let point = cubic.eval(t);
            let Some(k) = curvature(&cubic, t) else {
                if outline_points.len() >= 2 {
                    shapes.push(Shape::line(
                        outline_points.clone(),
                        Stroke::new(COMB_OUTLINE_STROKE_WIDTH, outline_color),
                    ));
                }
                outline_points.clear();
                continue;
            };

            let Some(normal) = curvature_normal(&cubic, t) else {
                if outline_points.len() >= 2 {
                    shapes.push(Shape::line(
                        outline_points.clone(),
                        Stroke::new(COMB_OUTLINE_STROKE_WIDTH, outline_color),
                    ));
                }
                outline_points.clear();
                continue;
            };

            let tooth_length = curvature_to_comb_length(k);
            let tooth_end = if tooth_length < MIN_TOOTH_LENGTH {
                point
            } else {
                point + normal * tooth_length
            };

            if tooth_length >= MIN_TOOTH_LENGTH {
                shapes.push(Shape::line_segment(
                    [point, tooth_end],
                    Stroke::new(COMB_STROKE_WIDTH, tooth_color),
                ));
            }
            outline_points.push(tooth_end);
        }

        if outline_points.len() >= 2 {
            shapes.push(Shape::line(
                outline_points,
                Stroke::new(COMB_OUTLINE_STROKE_WIDTH, outline_color),
            ));
        }
    }

    ui.painter().with_clip_rect(rect).extend(shapes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;

    const RECT: egui::Rect = egui::Rect {
        min: Pos2::new(100.0, 100.0),
        max: Pos2::new(300.0, 300.0),
    };

    fn point_with_tangent(val: [f32; 3], slot: usize, offset: [f32; 3]) -> ControlPoint {
        let mut cp = ControlPoint::new_simple(ControlPointValue::from(val), 0.0);
        cp.tangents_mut()[slot] = Some(ControlPointValue::from(offset));
        cp
    }

    #[test]
    fn runaway_outgoing_tangent_puts_p1_on_border_at_clamped_handle() {
        let start = point_with_tangent([0.035, 0.855, 0.566], 1, [161.96, 35.47, 0.0]);
        let end = point_with_tangent([0.775, 0.592, 0.604], 0, [-0.175, 0.315, 0.0]);
        let cubic = cubic_for_segment(&start, &end, RECT);
        assert_eq!(cubic.p1, RECT.right_top());
    }

    #[test]
    fn in_range_tangents_offset_the_endpoints() {
        let start = point_with_tangent([0.2, 0.3, 0.5], 1, [0.1, 0.2, 0.0]);
        let end = point_with_tangent([0.8, 0.6, 0.5], 0, [-0.1, 0.15, 0.0]);
        let cubic = cubic_for_segment(&start, &end, RECT);
        let size = RECT.size();
        let expected_p1 = cubic.p0 + Vec2::new(0.1 * size.x, -0.2 * size.y);
        let expected_p2 = cubic.p3 + Vec2::new(-0.1 * size.x, -0.15 * size.y);
        assert!((cubic.p1 - expected_p1).length() < 1e-3, "{cubic:?}");
        assert!((cubic.p2 - expected_p2).length() < 1e-3, "{cubic:?}");
    }
}
