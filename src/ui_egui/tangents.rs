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
    pub dragged_tangent_offset: Option<Vec2>,
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
        dragged_tangent_offset: None,
    };

    for (tangent_index, tangent) in cp.tangents().iter().enumerate() {
        if (tangent_index == 0 && is_first) || (tangent_index == 1 && is_last) {
            continue;
        }

        let Some(tang) = tangent else { continue };
        let tang_xy = [cp.val()[0] + tang.val[0], cp.val()[1] + tang.val[1]];
        let tang_screen = to_screen.transform_pos(Pos2::new(
            tang_xy[0].clamp(0.0, 1.0),
            (1.0 - tang_xy[1]).clamp(0.0, 1.0),
        ));

        if is_selected {
            let tang_screen = with_min_handle_length(
                cp_screen,
                tang_screen,
                tangent_index,
                2.0 * control_point_radius,
            );
            let response = ui.interact(
                Rect::from_center_size(tang_screen, control_point_draw_size),
                parent_response.id.with((cp_index, tangent_index)),
                Sense::drag(),
            );

            if result.dragged_tangent.is_none() && response.dragged_by(Primary) {
                let drawn_end = to_screen.inverse().transform_pos(tang_screen);
                let delta = response.drag_delta() / parent_size;
                result.selected_by_tangent = true;
                result.selected_tangent = Some(tangent_index);
                result.dragged_tangent = Some(response.clone());
                result.dragged_tangent_offset = Some(tangent_offset_after_drag(
                    Vec2::new(cp.val()[0], cp.val()[1]),
                    Vec2::new(drawn_end.x, 1.0 - drawn_end.y),
                    Vec2::new(delta.x, -delta.y),
                ));
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

fn with_min_handle_length(
    cp_screen: Pos2,
    tang_screen: Pos2,
    tangent_index: usize,
    min_length: f32,
) -> Pos2 {
    let handle = tang_screen - cp_screen;
    if handle.length() >= min_length {
        return tang_screen;
    }
    let direction = match (handle == Vec2::ZERO, tangent_index) {
        (false, _) => handle.normalized(),
        (true, 0) => -Vec2::X,
        (true, _) => Vec2::X,
    };
    cp_screen + min_length * direction
}

fn tangent_offset_after_drag(point_sv: Vec2, drawn_end_sv: Vec2, delta_sv: Vec2) -> Vec2 {
    (drawn_end_sv + delta_sv).clamp(Vec2::ZERO, Vec2::splat(1.0)) - point_sv
}

#[cfg(test)]
mod tests {
    use super::*;

    const POINT: Vec2 = Vec2::new(0.5, 0.5);

    #[test]
    fn drag_inside_square_follows_delta() {
        let offset = tangent_offset_after_drag(POINT, Vec2::new(0.6, 0.5), Vec2::new(0.1, -0.2));
        assert!(
            (offset - Vec2::new(0.2, -0.2)).length() < 1e-6,
            "{offset:?}"
        );
    }

    #[test]
    fn drag_past_square_stops_at_edge() {
        let past_max = tangent_offset_after_drag(POINT, Vec2::new(0.9, 0.9), Vec2::new(0.5, 0.5));
        assert_eq!(POINT + past_max, Vec2::splat(1.0));
        let past_min = tangent_offset_after_drag(POINT, Vec2::new(0.1, 0.1), Vec2::new(-0.5, -0.5));
        assert_eq!(POINT + past_min, Vec2::ZERO);
    }

    #[test]
    fn drag_back_from_edge_moves_immediately() {
        let at_edge =
            POINT + tangent_offset_after_drag(POINT, Vec2::new(0.9, 0.5), Vec2::new(0.5, 0.0));
        let back = POINT + tangent_offset_after_drag(POINT, at_edge, Vec2::new(-0.1, 0.0));
        assert!((back - Vec2::new(0.9, 0.5)).length() < 1e-6, "{back:?}");
    }
}
