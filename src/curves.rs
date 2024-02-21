//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use std::ops::Mul;

use ecolor::{Color32, HsvaGamma};
use eframe::egui::color_picker::show_color;
use eframe::egui::{self, Id, Sense, Ui};
use eframe::epaint::{Pos2, Rect, Shape, Stroke, Vec2};
use eframe::{emath, epaint};
use egui::epaint::{CubicBezierShape, PathShape, QuadraticBezierShape};
use splines::{Key, Spline};

use crate::math::{add_array, add_array_array, combination, mul_array};
use crate::ui_common::contrast_color;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct PaintCurve<T, V> {
    pub spline: Spline<T, V>,

    /// Stroke for Bézier curve.
    stroke: Stroke,

    /// Fill for Bézier curve.
    fill: Color32,

    /// Stroke for auxiliary lines.
    aux_stroke: Stroke,

    bounding_box_stroke: Stroke,
}

impl<T: std::default::Default, V: std::default::Default> Default for PaintCurve<T, V> {
    fn default() -> Self {
        Self {
            spline: Spline::default(),
            stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
            fill: Color32::from_rgb(50, 100, 150).linear_multiply(0.25),
            aux_stroke: Stroke::new(1.0, Color32::RED.linear_multiply(0.25)),
            bounding_box_stroke: Stroke::new(0.0, Color32::LIGHT_GREEN.linear_multiply(0.25)),
        }
    }
}

impl PaintCurve<f32, [f32; 3]> {
    pub fn ui_content(
        &mut self,
        ui: &mut Ui,
        is_middle_interpolated: bool,
        response: &egui::Response,
    ) -> (
        /*
            dragged_point_response,
            selected_index,
            hovering_point_option,
        */
        Option<egui::Response>,
        Option<usize>,
        Option<(egui::Response, usize)>,
    ) {
        let num_control_points = self.spline.len();
        if num_control_points <= 0 {
            return (None, None, None);
        }
        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(1.0, 1.0)),
            response.rect,
        );

        let mut dragged_point_response = None;

        let control_point_radius = 8.0;

        // Fill Circle
        let first_index = 0;
        let last_index = num_control_points - 1;
        let mut selected_index = None;
        let mut hovering_control_point = None;

        let control_point_draw_size: Vec2 = Vec2::splat(2.0 * control_point_radius);
        let control_point_shapes_fill: Vec<Shape> = self
            .spline
            .into_iter()
            .enumerate()
            .take(num_control_points)
            .map(|(i, key)| {
                let control_point = &key.value;
                let mut point_xy: Pos2 =
                    Pos2::new(control_point[0], 1.0 - control_point[1].clamp(0.0, 1.0));

                let point_in_screen: Pos2 = to_screen.transform_pos(point_xy);
                let control_point_ui_rect =
                    Rect::from_center_size(point_in_screen, control_point_draw_size);
                let control_point_response =
                    ui.interact(control_point_ui_rect, response.id.with(i), Sense::drag());

                // TODO: CHECK THIS LOGIC (is_inactive, is_inactive_click_or_drag)
                let mut is_inactive: bool = false;
                let mut is_inactive_click_or_drag: bool = false;

                if is_middle_interpolated {
                    if !(i == first_index || i == last_index) {
                        is_inactive = true;
                    }
                }

                if control_point_response.dragged() {
                    println!("Dragged index: {i}");
                    is_inactive_click_or_drag = is_inactive;

                    if !is_inactive {
                        point_xy += control_point_response.drag_delta() / response.rect.size();
                        selected_index = Some(i);
                        dragged_point_response = Some(control_point_response.clone());
                    }
                }

                if control_point_response.hovered() {
                    println!("Hovered index: {i}");
                    hovering_control_point = Some((control_point_response, i));
                }

                let point_as_color = HsvaGamma {
                    h: control_point[2],
                    s: control_point[0],
                    v: control_point[1],
                    a: 1.0,
                };
                let mut color_to_show = if !is_inactive_click_or_drag {
                    point_as_color
                } else {
                    HsvaGamma {
                        h: point_as_color.h,
                        s: point_as_color.s * 0.75,
                        v: point_as_color.v * 0.6,
                        a: point_as_color.a,
                    }
                };

                if is_inactive {
                    let mut stroke: Stroke = ui.style().noninteractive().fg_stroke;
                    stroke.color = Color32::LIGHT_GRAY;
                    stroke.width *= 6.0;
                    ui.painter().add(Shape::circle_stroke(
                        point_in_screen,
                        1.8 * control_point_radius,
                        stroke,
                    ));
                }

                Shape::circle_filled(point_in_screen, 1.8 * control_point_radius, color_to_show)
            })
            .collect();

        // Circle Stroke
        let control_point_shapes: Vec<Shape> = self
            .spline
            .into_iter()
            .enumerate()
            .take(num_control_points)
            .map(|(i, key)| {
                let mut point = Pos2::new(key.value[0], 1.0 - key.value[1]);
                point = to_screen.from().clamp(point);

                let point_in_screen = to_screen.transform_pos(point);
                let stroke: Stroke = ui.style().interact(response).fg_stroke;

                Shape::circle_stroke(point_in_screen, control_point_radius, stroke)
            })
            .collect();

        let selected_shape = if selected_index.is_some() {
            let key = self.spline.get(selected_index.unwrap()).unwrap();
            let mut point = Pos2::new(key.value[0], 1.0 - key.value[1]);

            let point_in_screen = to_screen.transform_pos(point);
            let stroke: Stroke = ui.style().interact(response).fg_stroke;

            Some(Shape::circle_stroke(
                point_in_screen,
                1.6 * control_point_radius,
                stroke,
            ))
        } else {
            None
        };

        let points_in_screen: Vec<Pos2> = self
            .spline
            .into_iter()
            .take(num_control_points)
            .map(|key| {
                let point = Pos2::new(key.value[0], 1.0 - key.value[1]);
                to_screen * point
            })
            .collect();

        // match num_control_points {
        //     3 => {
        //         let points = points_in_screen.clone().try_into().unwrap();
        //         let shape =
        //             QuadraticBezierShape::from_points_stroke(points, true, self.fill, self.stroke);
        //         ui.painter().add(epaint::RectShape::stroke(
        //             shape.visual_bounding_rect(),
        //             0.0,
        //             self.bounding_box_stroke,
        //         ));
        //         ui.painter().add(shape);
        //     }
        //     4 => {
        //         let points = points_in_screen.clone().try_into().unwrap();
        //         let shape =
        //             CubicBezierShape::from_points_stroke(points, true, self.fill, self.stroke);
        //         ui.painter().add(epaint::RectShape::stroke(
        //             shape.visual_bounding_rect(),
        //             0.0,
        //             self.bounding_box_stroke,
        //         ));
        //         ui.painter().add(shape);
        //     }
        //     _ => {
        //         unreachable!();
        //     }
        // };

        ui.painter()
            .add(PathShape::line(points_in_screen, self.aux_stroke));
        ui.painter().extend(control_point_shapes_fill);
        ui.painter().extend(control_point_shapes);

        match selected_shape {
            Some(s) => {
                ui.painter().add(s);
            }
            _ => {}
        }

        (
            dragged_point_response,
            selected_index,
            hovering_control_point,
        )
    }
}

impl<T, V> PaintCurve<T, V> {
    pub fn from_vec(keys: Vec<Key<T, V>>) -> Self
    where
        T: PartialOrd,
    {
        let mut spline = Spline::from_vec(keys);
        Self {
            spline: spline,
            stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
            fill: Color32::from_rgb(50, 100, 150).linear_multiply(0.25),
            aux_stroke: Stroke::new(1.0, Color32::RED.linear_multiply(0.25)),
            bounding_box_stroke: Stroke::new(0.0, Color32::LIGHT_GREEN.linear_multiply(0.25)),
        }
    }
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
