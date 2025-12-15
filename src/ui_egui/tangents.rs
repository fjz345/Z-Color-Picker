use ecolor::Color32;
use eframe::{
    egui::{self, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    emath::RectTransform,
    epaint::PathShape,
};

use crate::{datatypes::control_point::ControlPoint, ui_egui::control_points::control_point_pos};

pub struct TangentUiResult {
    pub selected_by_tangent: bool,
    pub selected_tangent: Option<usize>,
    pub dragged_tangent: Option<egui::Response>,
}

pub fn ui_control_point_tangents(
    ui: &mut Ui,
    cp_index: usize,
    cp: &ControlPoint,
    is_first: bool,
    is_last: bool,
    is_selected: bool,
    to_screen: &RectTransform,
    parent_response: &egui::Response,
    control_point_draw_size: Vec2,
    control_point_radius: f32,
    inactive_stroke: Stroke,
    tangent_shapes: &mut Vec<Shape>,
    tangent_paths: &mut Vec<PathShape>,
) -> TangentUiResult {
    use egui::PointerButton::Primary;

    const TANGENT_RADIUS_SCALE: f32 = 0.7;
    const ACTIVE_LINE_ALPHA: f32 = 0.25;
    const INACTIVE_LINE_ALPHA: f32 = 0.002;
    const INACTIVE_RADIUS_RATIO: f32 = 0.2 / 0.7;

    let cp_screen = to_screen.transform_pos(control_point_pos(cp));
    let parent_size = parent_response.rect.size();

    let active_radius = TANGENT_RADIUS_SCALE * control_point_radius;
    let inactive_radius = INACTIVE_RADIUS_RATIO * active_radius;

    let mut result = TangentUiResult {
        selected_by_tangent: false,
        selected_tangent: None,
        dragged_tangent: None,
    };

    for (tangent_index, tangent) in cp.tangents().iter().enumerate() {
        if (tangent_index == 0 && is_first) || (tangent_index == 1 && is_last) {
            continue;
        }

        let Some(tang) = tangent else { continue };
        let tang_xy = [cp.val()[0] + tang.val[0], cp.val()[1] + tang.val[1]];
        let mut tang_screen = to_screen.transform_pos(Pos2::new(
            tang_xy[0].clamp(0.0, 1.0),
            (1.0 - tang_xy[1]).clamp(0.0, 1.0),
        ));

        if is_selected {
            let response = ui.interact(
                Rect::from_center_size(tang_screen, control_point_draw_size),
                parent_response.id.with((cp_index, tangent_index)),
                Sense::drag(),
            );

            if result.dragged_tangent.is_none() && response.dragged_by(Primary) {
                tang_screen += response.drag_delta() / parent_size;
                result.selected_by_tangent = true;
                result.selected_tangent = Some(tangent_index);
                result.dragged_tangent = Some(response.clone());
            }

            tangent_paths.push(PathShape::line(
                vec![cp_screen, tang_screen],
                Stroke::new(1.0, Color32::WHITE.linear_multiply(ACTIVE_LINE_ALPHA)),
            ));
            tangent_shapes.push(Shape::circle_stroke(
                tang_screen,
                active_radius,
                inactive_stroke,
            ));
        } else {
            tangent_paths.push(PathShape::line(
                vec![cp_screen, tang_screen],
                Stroke::new(1.0, Color32::WHITE.linear_multiply(INACTIVE_LINE_ALPHA)),
            ));
            tangent_shapes.push(Shape::circle_stroke(
                tang_screen,
                inactive_radius,
                inactive_stroke,
            ));
        }
    }
    result
}
