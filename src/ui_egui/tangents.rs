use ecolor::Color32;
use eframe::{
    egui::{self, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2},
    epaint::PathShape,
};

use crate::{
    datatypes::control_point::{ControlPoint, ControlPointValue},
    ui_egui::control_points::{
        bezier_handle, control_point_pos, screen_to_unit, unit_to_screen, value_pos,
    },
};

pub struct TangentUiResult {
    pub selected_by_tangent: bool,
    pub selected_tangent: Option<usize>,
    pub dragged_tangent: Option<egui::Response>,
    pub dragged_tangent_offset: Option<ControlPointValue>,
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

        let Some(tangent) = *tangent else {
            continue;
        };
        let handle = bezier_handle(cp, tangent_index);
        let handle_screen = unit_to_screen(square, value_pos(&handle));

        let tang_screen =
            with_min_handle_length(cp_screen, handle_screen, tangent_index, 2.0 * point_radius);
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

        let drag_start_id = response.id.with("drag_start");
        if response.drag_started_by(Primary) {
            let press_origin = ui
                .input(|i| i.pointer.press_origin())
                .or(response.interact_pointer_pos());
            if let Some(press_origin) = press_origin {
                let is_drawn_where_stored =
                    tang_screen == handle_screen && handle == *cp.val() + tangent;
                let start_offset = if is_drawn_where_stored {
                    tangent
                } else {
                    let drawn_end = screen_to_unit(square, tang_screen);
                    ControlPointValue::new(
                        drawn_end.x - cp.val()[0],
                        1.0 - drawn_end.y - cp.val()[1],
                        tangent.h(),
                    )
                };
                ui.data_mut(|d| d.insert_temp(drag_start_id, (start_offset, press_origin)));
            }
        }

        if result.dragged_tangent.is_none() && response.dragged_by(Primary) {
            result.selected_by_tangent = true;
            result.selected_tangent = Some(tangent_index);
            result.dragged_tangent = Some(response.clone());
            let drag_start = ui.data(|d| d.get_temp::<(ControlPointValue, Pos2)>(drag_start_id));
            if let (Some((start_offset, press_origin)), Some(pointer)) =
                (drag_start, response.interact_pointer_pos())
            {
                let delta = (pointer - press_origin) / parent_size;
                result.dragged_tangent_offset =
                    Some(start_offset + ControlPointValue::new(delta.x, -delta.y, 0.0));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

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
