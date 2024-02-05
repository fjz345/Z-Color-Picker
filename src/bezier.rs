//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use std::ops::Mul;

use ecolor::HsvaGamma;
use eframe::egui;
use eframe::egui::color_picker::show_color;
use egui::epaint::{CubicBezierShape, PathShape, QuadraticBezierShape};
use egui::*;

use crate::color_picker::{main_color_picker_color_at, xyz_to_hsva};
use crate::math::{add_array, add_array_array, combination, mul_array};
use crate::ui_common::contrast_color;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct PaintBezier {
    /// Bézier curve degree, it can be 3, 4.
    degree: usize,

    /// The control points. The [`Self::degree`] first of them are used.
    pub control_points: [Pos2; 4],

    hue: [f32; 4],

    /// Stroke for Bézier curve.
    stroke: Stroke,

    /// Fill for Bézier curve.
    fill: Color32,

    /// Stroke for auxiliary lines.
    aux_stroke: Stroke,

    bounding_box_stroke: Stroke,
}

impl Default for PaintBezier {
    fn default() -> Self {
        Self {
            degree: 4,
            control_points: [
                pos2(50.0, 50.0),
                pos2(60.0, 250.0),
                pos2(200.0, 200.0),
                pos2(250.0, 50.0),
            ],
            stroke: Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
            fill: Color32::from_rgb(50, 100, 150).linear_multiply(0.25),
            aux_stroke: Stroke::new(1.0, Color32::RED.linear_multiply(0.25)),
            bounding_box_stroke: Stroke::new(0.0, Color32::LIGHT_GREEN.linear_multiply(0.25)),
            hue: [0.0; 4],
        }
    }
}

impl PaintBezier {
    pub fn degree(&self) -> usize {
        self.degree
    }

    pub fn control_points(&self, bezier_draw_size: Vec2) -> Vec<Vec2> {
        let points_in_screen: Vec<Vec2> = self
            .control_points
            .iter()
            .take(self.degree)
            .map(|p| Vec2::new(p.x, p.y) / bezier_draw_size)
            .collect();

        points_in_screen
    }

    pub fn set_hue(&mut self, index: usize, val: f32) {
        self.hue[index] = val;
    }

    pub fn get_hue(&self, index: usize) -> f32 {
        self.hue[index]
    }

    pub fn get_hue_mut(&mut self, index: usize) -> &mut f32 {
        &mut self.hue[index]
    }

    pub fn ui_content(
        &mut self,
        ui: &mut Ui,
        response: &egui::Response,
    ) -> (egui::Response, Option<egui::Response>, Option<usize>) {
        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        let visuals = ui.style().interact(&response);

        let mut dragged_point_response = None;

        let control_point_radius = 8.0;

        // Fill Circle
        let mut selected_index = None;
        let hues = self.hue;
        let control_point_shapes_fill: Vec<Shape> = self
            .control_points
            .iter_mut()
            .enumerate()
            .take(self.degree)
            .map(|(i, point)| {
                let size: Vec2 = Vec2::splat(2.0 * control_point_radius);

                let unmodified_point = point.clone();

                let point_in_screen: Pos2 = to_screen.transform_pos(*point);
                let point_rect = Rect::from_center_size(point_in_screen, size);
                let point_id = response.id.with(i);
                let point_response = ui.interact(point_rect, point_id, Sense::drag());

                if point_response.dragged() {
                    *point += point_response.drag_delta();
                    selected_index = Some(i);
                    dragged_point_response = Some(point_response.clone());
                }

                *point = to_screen.from().clamp(*point);

                let point_in_screen = to_screen.transform_pos(*point);

                let mut color_to_show = xyz_to_hsva(
                    hues[i],
                    (unmodified_point.x / response.rect.size().x),
                    (unmodified_point.y / response.rect.size().y),
                );

                ui.painter().add(epaint::CircleShape {
                    center: point_in_screen,
                    radius: point_rect.width() / 6.0,
                    fill: color_to_show.into(),
                    stroke: Stroke::new(visuals.fg_stroke.width, contrast_color(color_to_show)),
                });
                Shape::circle_filled(point_in_screen, 1.8 * control_point_radius, color_to_show)
            })
            .collect();

        // Circle Stroke
        let control_point_shapes: Vec<Shape> = self
            .control_points
            .iter_mut()
            .enumerate()
            .take(self.degree)
            .map(|(i, point)| {
                *point = to_screen.from().clamp(*point);

                let point_in_screen = to_screen.transform_pos(*point);
                let stroke: Stroke = ui.style().interact(response).fg_stroke;

                Shape::circle_stroke(point_in_screen, control_point_radius, stroke)
            })
            .collect();

        let selected_shape = if (selected_index.is_some()) {
            let mut point = self.control_points[selected_index.unwrap()];

            point = to_screen.from().clamp(point);

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
            .control_points
            .iter()
            .take(self.degree)
            .map(|p| to_screen * *p)
            .collect();

        match self.degree {
            3 => {
                let points = points_in_screen.clone().try_into().unwrap();
                let shape =
                    QuadraticBezierShape::from_points_stroke(points, true, self.fill, self.stroke);
                ui.painter().add(epaint::RectShape::stroke(
                    shape.visual_bounding_rect(),
                    0.0,
                    self.bounding_box_stroke,
                ));
                ui.painter().add(shape);
            }
            4 => {
                let points = points_in_screen.clone().try_into().unwrap();
                let shape =
                    CubicBezierShape::from_points_stroke(points, true, self.fill, self.stroke);
                ui.painter().add(epaint::RectShape::stroke(
                    shape.visual_bounding_rect(),
                    0.0,
                    self.bounding_box_stroke,
                ));
                ui.painter().add(shape);
            }
            _ => {
                unreachable!();
            }
        };

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

        (response.clone(), dragged_point_response, selected_index)
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
