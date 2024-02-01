//https://github.com/emilk/egui/blob/master/crates/egui_demo_lib/src/demo/paint_bezier.rs

use eframe::egui;
use egui::epaint::{CubicBezierShape, PathShape, QuadraticBezierShape};
use egui::*;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct PaintBezier {
    /// Bézier curve degree, it can be 3, 4.
    degree: usize,

    /// The control points. The [`Self::degree`] first of them are used.
    control_points: [Pos2; 4],

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

    pub fn ui_content_with_painter(
        &mut self,
        ui: &Ui,
        response: &egui::Response,
        painter: &Painter,
    ) -> (egui::Response, Option<usize>) {
        let to_screen = emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        let control_point_radius = 8.0;

        let mut selected_index = None;
        let control_point_shapes: Vec<Shape> = self
            .control_points
            .iter_mut()
            .enumerate()
            .take(self.degree)
            .map(|(i, point)| {
                let size = Vec2::splat(2.0 * control_point_radius);

                let point_in_screen: Pos2 = to_screen.transform_pos(*point);
                let point_rect = Rect::from_center_size(point_in_screen, size);
                let point_id = response.id.with(i);
                let point_response = ui.interact(point_rect, point_id, Sense::drag());

                if point_response.dragged() {
                    *point += point_response.drag_delta();
                    selected_index = Some(i);
                }

                *point = to_screen.from().clamp(*point);

                let point_in_screen = to_screen.transform_pos(*point);
                let stroke = ui.style().interact(&point_response).fg_stroke;

                Shape::circle_stroke(point_in_screen, control_point_radius, stroke)
            })
            .collect();

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
                painter.add(epaint::RectShape::stroke(
                    shape.visual_bounding_rect(),
                    0.0,
                    self.bounding_box_stroke,
                ));
                painter.add(shape);
            }
            4 => {
                let points = points_in_screen.clone().try_into().unwrap();
                let shape =
                    CubicBezierShape::from_points_stroke(points, true, self.fill, self.stroke);
                painter.add(epaint::RectShape::stroke(
                    shape.visual_bounding_rect(),
                    0.0,
                    self.bounding_box_stroke,
                ));
                painter.add(shape);
            }
            _ => {
                unreachable!();
            }
        };

        painter.add(PathShape::line(points_in_screen, self.aux_stroke));
        painter.extend(control_point_shapes);

        (response.clone(), selected_index)
    }

    pub fn ui_content(&mut self, ui: &mut Ui) -> (egui::Response, Option<usize>) {
        let (response, painter) = ui.allocate_painter(
            Vec2::new(ui.available_width(), ui.available_height()),
            Sense::hover(),
        );

        self.ui_content_with_painter(ui, &response, &painter)
    }
}
