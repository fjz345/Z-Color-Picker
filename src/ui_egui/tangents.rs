use ecolor::Color32;
use eframe::{
    egui::{self, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    epaint::PathShape,
};

use crate::{
    datatypes::control_point::ControlPoint,
    ui_egui::control_points::{
        bezier_handle, control_point_pos, screen_to_unit, unit_to_screen, value_pos,
    },
};

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
    square: Rect,
    parent_response: &egui::Response,
    hit_size: Vec2,
    point_radius: f32,
    tangent_shapes: &mut Vec<Shape>,
    tangent_paths: &mut Vec<PathShape>,
) -> TangentUiResult {
    use egui::PointerButton::Primary;

    const TANGENT_RADIUS_SCALE: f32 = 0.7;
    const LINE_ALPHA: f32 = 0.6;
    const UNDERLAY: Stroke = Stroke {
        width: 3.0,
        color: Color32::from_black_alpha(160),
    };

    let mut result = TangentUiResult {
        selected_by_tangent: false,
        selected_tangent: None,
        dragged_tangent: None,
        dragged_tangent_offset: None,
    };
    if !is_selected {
        return result;
    }

    let cp_screen = unit_to_screen(square, control_point_pos(cp));
    let parent_size = parent_response.rect.size();

    let active_radius = TANGENT_RADIUS_SCALE * point_radius;
    let line_stroke = Stroke::new(1.5, Color32::WHITE.linear_multiply(LINE_ALPHA));
    let ring_stroke = Stroke::new(1.5, Color32::WHITE);

    for (tangent_index, tangent) in cp.tangents().iter().enumerate() {
        if (tangent_index == 0 && is_first) || (tangent_index == 1 && is_last) {
            continue;
        }

        if tangent.is_none() {
            continue;
        }
        let tang_screen = unit_to_screen(square, value_pos(&bezier_handle(cp, tangent_index)));

        let tang_screen =
            with_min_handle_length(cp_screen, tang_screen, tangent_index, 2.0 * point_radius);
        let response = ui
            .interact(
                Rect::from_center_size(tang_screen, hit_size),
                parent_response.id.with((cp_index, tangent_index)),
                Sense::drag(),
            )
            .on_hover_and_drag_cursor(CursorIcon::Grab)
            .on_hover_text("Drag: shape the curve");
        if response.dragged_by(Primary) {
            ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        }

        if result.dragged_tangent.is_none() && response.dragged_by(Primary) {
            let drawn_end = screen_to_unit(square, tang_screen);
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

        for stroke in [UNDERLAY, line_stroke] {
            tangent_paths.push(PathShape::line(vec![cp_screen, tang_screen], stroke));
        }
        for stroke in [UNDERLAY, ring_stroke] {
            tangent_shapes.push(Shape::circle_stroke(tang_screen, active_radius, stroke));
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

    #[test]
    fn unselected_point_emits_no_tangent_shapes() {
        let mut cp = ControlPoint::new_simple([0.5, 0.5, 0.2].into(), 0.0);
        *cp.tangents_mut() = [Some([-0.2, 0.0, 0.0].into()), Some([0.2, 0.0, 0.0].into())];
        let picker_rect = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::splat(200.0));
        let mut tangent_shapes = Vec::new();
        let mut tangent_paths = Vec::new();
        let _ = egui::Context::default().run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let parent = ui.allocate_rect(picker_rect, Sense::click_and_drag());
                ui_control_point_tangents(
                    ui,
                    1,
                    &cp,
                    false,
                    false,
                    false,
                    picker_rect,
                    &parent,
                    Vec2::splat(16.0),
                    8.0,
                    &mut tangent_shapes,
                    &mut tangent_paths,
                );
            });
        });
        assert!(tangent_shapes.is_empty());
        assert!(tangent_paths.is_empty());
    }
}
