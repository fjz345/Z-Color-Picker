//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use crate::common::SplineMode;
use crate::datatypes::control_point::{ControlPoint, ControlPointValue};
#[allow(unused_imports)]
use crate::error::Result;
use crate::ui_egui::control_points::{
    control_points_to_spline, find_spline_max_t, flatten_control_points,
};
use eframe::egui::{self, lerp, Sense, Ui};
use eframe::emath;
use eframe::epaint::{Pos2, Rect, Stroke, Vec2};
use egui::epaint::PathShape;

use crate::math::{add_array_array, mul_array};

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

    let last_spline_sample = spline.clamped_sample(spline_max_t);
    match last_spline_sample {
        Some(key) => spline_samples.push(key),
        None => todo!(),
    }

    spline_samples
}

pub fn sub_divide_control_points(
    control_points: &[ControlPoint],
    distance_per_point: f32,
) -> Vec<ControlPoint> {
    let capacity: usize = control_points.len() * 4;
    let mut sub_divided: Vec<ControlPoint> = Vec::with_capacity(capacity);

    for i in 1..control_points.len() {
        sub_divided.push(control_points[i - 1].clone());
        let hue_to_use = control_points[i - 1].val()[2];
        let first = control_points[i - 1].val().pos2();
        let last = control_points[i].val().pos2();
        let dir = (last - first).normalized();
        let mut sub_div_start = first;
        let mut distance_to_end = (last - first).dot(last - first).sqrt();
        while distance_to_end > distance_per_point {
            let new: Pos2 = sub_div_start + distance_per_point * dir;
            distance_to_end -= distance_per_point;

            let mut new_cp = ControlPoint::default();
            *new_cp.val_mut() = ControlPointValue::new(new.x, new.y, hue_to_use);
            *new_cp.t_mut() = lerp(*control_points[i - 1].t()..=*control_points[i].t(), 0.5);

            sub_divided.push(new_cp);
            sub_div_start = new;
        }
        let last_new = sub_div_start + distance_to_end.max(0.0) * dir;

        let mut new_cp = ControlPoint::default();
        *new_cp.val_mut() = ControlPointValue::new(last_new.x, last_new.y, hue_to_use);
        *new_cp.t_mut() = *control_points[i].t();

        let last_cp = new_cp;
        sub_divided.push(last_cp);
    }
    if control_points.last().is_some() {
        sub_divided.push(control_points.last().unwrap().clone());
    }

    sub_divided
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

    // let sub_divided_control_points = sub_divide_control_points(control_points, 0.01);
    let flattened_points = flatten_control_points(control_points);
    let spline_points =
        generate_spline_points_with_distance(&flattened_points[..], spline_mode, 0.01);

    for i in 1..spline_points.len() {
        let first = spline_points[i - 1];
        let next = spline_points[i];

        // let spline = control_points_to_spline(&sub_divided_control_points, spline_mode);
        let segment_color = first.color();

        let control_point_radius = 8.0;

        let point_first = first.pos2();
        let point_next = next.pos2();
        let mut points_in_screen: Vec<Pos2> = Vec::with_capacity(2);
        let point_in_screen_first = to_screen * Pos2::new(point_first.x, 1.0 - point_first.y);
        let point_in_screen_next = to_screen * Pos2::new(point_next.x, 1.0 - point_next.y);
        points_in_screen.push(point_in_screen_first);
        points_in_screen.push(point_in_screen_next);

        let shape = PathShape::line(
            points_in_screen,
            Stroke::new(control_point_radius * 1.6, segment_color),
        );

        ui.painter().add(shape);
    }

    Some(response)
}

pub struct Bezier<const D: usize, const N: usize> {
    pub control_points: [[f32; D]; N],
}

impl<const D: usize, const N: usize> Bezier<D, N> {
    pub fn new() -> Self {
        Self {
            control_points: [[0.0; D]; N],
        }
    }

    pub fn get_at(&self, t: f32) -> [f32; D] {
        // https://en.wikipedia.org/wiki/B%C3%A9zier_curve
        let mut outer_sum: [f32; D] = [0.0; D];

        for i in 0..N {
            let inner_prod = num_integer::binomial(N as u64, i as u64) as f32
                * (1.0 - t).powi(N as i32 - i as i32)
                * t.powi(i as i32);
            let inner = mul_array(self.control_points[i].clone(), inner_prod);
            outer_sum = add_array_array(outer_sum, inner);
        }

        outer_sum
    }
}
