use std::ops::RangeInclusive;

use eframe::egui::{DragValue, Ui};
use eframe::epaint::Vec2;

use crate::{
    datatypes::control_point::ControlPoint,
    ui_egui::{
        color_picker::{
            apply_group_delta_sv, limit_group_delta, HUE_DRAG_MAX_FRACTION, NO_SELECTION_HINT,
        },
        control_points::{drag_t, t_drag_bounds},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectorField {
    H,
    S,
    V,
    T,
}

impl InspectorField {
    const ALL: [Self; 4] = [Self::H, Self::S, Self::V, Self::T];

    fn label(self) -> &'static str {
        match self {
            Self::H => "H",
            Self::S => "S",
            Self::V => "V",
            Self::T => "T",
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::H => "°",
            Self::S | Self::V => "%",
            Self::T => "",
        }
    }

    fn speed(self) -> f32 {
        match self {
            Self::H => 1.0,
            Self::S | Self::V => 0.5,
            Self::T => 0.005,
        }
    }

    fn max_decimals(self) -> usize {
        match self {
            Self::H | Self::S | Self::V => 1,
            Self::T => 3,
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Self::H => "Hue in degrees",
            Self::S => {
                "Saturation: the point's x in the square. The HSV readout derives S from the \
                 sRGB bytes, so the two can differ"
            }
            Self::V => "Value: the point's y in the square",
            Self::T => "T: the point's position along the gradient, between its neighbours",
        }
    }
}

const EDIT_HINT: &str = "Drag, or click and type: Enter or Tab commits, Esc cancels";
pub const AUTO_HUE_HINT: &str = "Auto hue: middle hues follow the end points";
pub const PINNED_T_HINT: &str = "The first and last points are pinned to T 0 and 1";
pub const NO_ROOM_T_HINT: &str = "No room for T between the neighbouring points";

#[derive(Clone, Copy, Debug)]
pub struct InspectorOptions {
    pub is_hue_middle_interpolated: bool,
    pub is_curve_locked: bool,
    pub is_window_lock: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldState {
    pub value: Option<f32>,
    pub range: RangeInclusive<f32>,
    pub disabled_reason: Option<&'static str>,
}

pub fn inspector_fields(
    points: &[ControlPoint],
    selected: Option<usize>,
    opts: InspectorOptions,
) -> [FieldState; 4] {
    let Some((index, cp)) = selected.and_then(|i| Some((i, points.get(i)?))) else {
        return InspectorField::ALL.map(|_| FieldState {
            value: None,
            range: 0.0..=0.0,
            disabled_reason: Some(NO_SELECTION_HINT),
        });
    };
    let val = cp.val();
    let t = *cp.t();
    let is_middle = index != 0 && index + 1 != points.len();
    let sv_range = if opts.is_window_lock {
        0.0..=100.0
    } else {
        f32::NEG_INFINITY..=f32::INFINITY
    };
    let t_state = match t_drag_bounds(points, index) {
        Some((lo, hi)) => FieldState {
            value: Some(t),
            range: lo..=hi,
            disabled_reason: None,
        },
        None => FieldState {
            value: Some(t),
            range: t..=t,
            disabled_reason: Some(if is_middle {
                NO_ROOM_T_HINT
            } else {
                PINNED_T_HINT
            }),
        },
    };
    [
        FieldState {
            value: Some(val[2].rem_euclid(1.0) * 360.0),
            range: 0.0..=360.0,
            disabled_reason: (opts.is_hue_middle_interpolated && is_middle)
                .then_some(AUTO_HUE_HINT),
        },
        FieldState {
            value: Some(val[0] * 100.0),
            range: sv_range.clone(),
            disabled_reason: None,
        },
        FieldState {
            value: Some(val[1] * 100.0),
            range: sv_range,
            disabled_reason: None,
        },
        t_state,
    ]
}

pub fn apply_inspector_edit(
    points: &mut [ControlPoint],
    index: usize,
    field: InspectorField,
    value: f32,
    opts: InspectorOptions,
) {
    if !value.is_finite() {
        return;
    }
    match field {
        InspectorField::H => {
            let hue = &mut points[index].val_mut().val[2];
            *hue = hue.floor() + (value / 360.0).clamp(0.0, HUE_DRAG_MAX_FRACTION);
        }
        InspectorField::S => set_sv(points, index, 0, value / 100.0, opts),
        InspectorField::V => set_sv(points, index, 1, value / 100.0, opts),
        InspectorField::T => drag_t(points, index, value),
    }
}

fn set_sv(
    points: &mut [ControlPoint],
    index: usize,
    axis: usize,
    target: f32,
    opts: InspectorOptions,
) {
    if !opts.is_curve_locked {
        points[index].val_mut().val[axis] = target;
        return;
    }
    let mut delta_sv = Vec2::ZERO;
    delta_sv[axis] = target - points[index].val()[axis];
    if opts.is_window_lock {
        delta_sv = limit_group_delta(points, delta_sv);
    }
    apply_group_delta_sv(points, delta_sv);
}

// A DragValue left after focusing re-commits its rounded display text.
fn is_shown_value(committed: f32, shown: f32, max_decimals: usize) -> bool {
    (committed - shown).abs() < 0.5 * 10f32.powi(-(max_decimals as i32))
}

pub fn point_inspector(
    ui: &mut Ui,
    points: &mut [ControlPoint],
    selected: Option<usize>,
    opts: InspectorOptions,
) {
    let states = inspector_fields(points, selected, opts);
    ui.push_id(("inspector", selected), |ui| {
        ui.horizontal(|ui| {
            for (field, state) in InspectorField::ALL.into_iter().zip(states) {
                ui.label(field.label());
                let mut value = state.value.unwrap_or_default();
                let drag = DragValue::new(&mut value)
                    .range(state.range)
                    .speed(field.speed())
                    .max_decimals(field.max_decimals())
                    .clamp_existing_to_range(false)
                    .update_while_editing(false);
                let drag = match state.value {
                    Some(_) => drag.suffix(field.suffix()),
                    None => drag.custom_formatter(|_, _| "-".to_owned()),
                };
                let mut response = ui
                    .add_enabled(state.disabled_reason.is_none(), drag)
                    .on_hover_text(format!("{}. {EDIT_HINT}", field.hint()));
                if let Some(reason) = state.disabled_reason {
                    response = response.on_disabled_hover_text(reason);
                }
                let is_edited = response.dragged()
                    || state
                        .value
                        .is_some_and(|shown| !is_shown_value(value, shown, field.max_decimals()));
                if let (true, true, Some(index)) = (response.changed(), is_edited, selected) {
                    apply_inspector_edit(points, index, field, value, opts);
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;
    use crate::datatypes::hsv_key_value::test_support::assert_close;
    use crate::ui_egui::control_points::{has_valid_t, MIN_T_GAP};
    use eframe::egui::{self, Pos2, Rect};
    use eframe::epaint::vec2;
    use InspectorField::{H, S, T, V};

    const FREE: InspectorOptions = InspectorOptions {
        is_hue_middle_interpolated: false,
        is_curve_locked: false,
        is_window_lock: false,
    };

    fn points_at(svht: &[(f32, f32, f32, f32)]) -> Vec<ControlPoint> {
        svht.iter()
            .map(|&(s, v, h, t)| ControlPoint::new_simple(ControlPointValue::new(s, v, h), t))
            .collect()
    }

    fn three_points() -> Vec<ControlPoint> {
        points_at(&[
            (0.2, 0.3, 0.1, 0.0),
            (0.5, 0.6, 0.4, 0.4),
            (0.7, 0.9, 0.8, 1.0),
        ])
    }

    fn points_with_t(ts: &[f32]) -> Vec<ControlPoint> {
        ts.iter()
            .map(|&t| ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.5), t))
            .collect()
    }

    fn state(
        points: &[ControlPoint],
        selected: usize,
        opts: InspectorOptions,
        field: InspectorField,
    ) -> FieldState {
        inspector_fields(points, Some(selected), opts)[field as usize].clone()
    }

    fn assert_values(points: &[ControlPoint], expected: &[ControlPoint]) {
        assert_eq!(points.len(), expected.len());
        for (actual, expected) in points.iter().zip(expected) {
            assert_close(*actual.val(), *expected.val());
        }
    }

    #[test]
    fn no_selection_disables_every_field() {
        for selected in [None, Some(3)] {
            for state in inspector_fields(&three_points(), selected, FREE) {
                assert_eq!(state.value, None, "{selected:?}");
                assert_eq!(
                    state.disabled_reason,
                    Some(NO_SELECTION_HINT),
                    "{selected:?}"
                );
            }
        }
    }

    #[test]
    fn hue_of_middle_points_is_read_only_under_auto_hue() {
        let points = three_points();
        let auto_hue = InspectorOptions {
            is_hue_middle_interpolated: true,
            ..FREE
        };

        let middle = state(&points, 1, auto_hue, H);

        assert_eq!(middle.disabled_reason, Some(AUTO_HUE_HINT));
        assert_eq!(middle.value, Some(0.4 * 360.0));
        for end in [0, 2] {
            assert_eq!(state(&points, end, auto_hue, H).disabled_reason, None);
        }
        assert_eq!(state(&points, 1, FREE, H).disabled_reason, None);
    }

    #[test]
    fn end_points_pin_t() {
        let points = three_points();
        for end in [0, 2] {
            assert_eq!(
                state(&points, end, FREE, T).disabled_reason,
                Some(PINNED_T_HINT)
            );
        }
        assert_eq!(
            state(&points_with_t(&[0.0]), 0, FREE, T).disabled_reason,
            Some(PINNED_T_HINT)
        );
        let tight = points_with_t(&[0.0, 0.5, 0.5005, 0.501, 1.0]);
        assert_eq!(
            state(&tight, 2, FREE, T).disabled_reason,
            Some(NO_ROOM_T_HINT)
        );

        let mut pinned = three_points();
        apply_inspector_edit(&mut pinned, 0, T, 0.3, FREE);
        apply_inspector_edit(&mut pinned, 2, T, 0.3, FREE);
        assert_eq!(pinned, three_points());
    }

    #[test]
    fn t_edit_stays_strictly_between_neighbours() {
        let mut points = points_with_t(&[0.0, 0.4, 0.6, 1.0]);
        assert_eq!(
            state(&points, 1, FREE, T).range,
            MIN_T_GAP..=0.6 - MIN_T_GAP
        );

        apply_inspector_edit(&mut points, 1, T, 0.9, FREE);
        assert_eq!(*points[1].t(), 0.6 - MIN_T_GAP);
        apply_inspector_edit(&mut points, 2, T, -1.0, FREE);
        assert_eq!(*points[2].t(), *points[1].t() + MIN_T_GAP);
        apply_inspector_edit(&mut points, 1, T, 0.25, FREE);
        assert_eq!(*points[1].t(), 0.25);
        assert!(has_valid_t(&points));
    }

    #[test]
    fn hue_edit_keeps_the_turn() {
        let cases = [
            (1.25, 90.0, 1.25),
            (1.25, 360.0, 1.0 + HUE_DRAG_MAX_FRACTION),
            (-0.5, 180.0, -0.5),
            (0.1, -30.0, 0.0),
        ];
        for (stored, degrees, expected) in cases {
            let mut points = points_at(&[(0.5, 0.5, stored, 0.0), (0.5, 0.5, 0.3, 1.0)]);
            assert_eq!(
                state(&points, 0, FREE, H).value,
                Some(stored.rem_euclid(1.0) * 360.0)
            );

            apply_inspector_edit(&mut points, 0, H, degrees, FREE);

            assert_eq!(points[0].val()[2], expected, "{stored} {degrees}");
            assert_eq!(points[1].val()[2], 0.3);
        }
    }

    #[test]
    fn sv_edit_under_lock_moves_every_point_and_stops_at_the_border_with_clamp() {
        let locked = InspectorOptions {
            is_curve_locked: true,
            ..FREE
        };
        let clamped = InspectorOptions {
            is_window_lock: true,
            ..locked
        };
        let cases = [
            (S, 100.0, clamped, [(0.5, 0.3), (0.8, 0.6), (1.0, 0.9)]),
            (S, 100.0, locked, [(0.7, 0.3), (1.0, 0.6), (1.2, 0.9)]),
            (V, 10.0, clamped, [(0.2, 0.0), (0.5, 0.3), (0.7, 0.6)]),
            (S, 100.0, FREE, [(0.2, 0.3), (1.0, 0.6), (0.7, 0.9)]),
        ];
        for (field, value, opts, expected_sv) in cases {
            let mut points = three_points();
            let mut expected = three_points();
            for (cp, (s, v)) in expected.iter_mut().zip(expected_sv) {
                cp.val_mut().val[0] = s;
                cp.val_mut().val[1] = v;
            }

            apply_inspector_edit(&mut points, 1, field, value, opts);

            assert_values(&points, &expected);
        }
    }

    const SCREEN: Rect = Rect {
        min: Pos2::ZERO,
        max: Pos2::new(600.0, 300.0),
    };

    fn inspector_frame(
        ctx: &egui::Context,
        points: &mut [ControlPoint],
        selected: Option<usize>,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                point_inspector(ui, points, selected, FREE);
            });
        })
    }

    fn field_pos(output: &egui::FullOutput, field: InspectorField) -> Pos2 {
        let label = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == field.label() => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{field:?} label not painted"));
        label.right_center() + vec2(12.0, 0.0)
    }

    fn primary_click(pos: Pos2) -> [Vec<egui::Event>; 2] {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        [
            vec![egui::Event::PointerMoved(pos), button(true)],
            vec![button(false)],
        ]
    }

    fn key_press(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    fn click_field(
        ctx: &egui::Context,
        points: &mut [ControlPoint],
        selected: Option<usize>,
        field: InspectorField,
    ) {
        let output = inspector_frame(ctx, points, selected, Vec::new());
        for events in primary_click(field_pos(&output, field)) {
            inspector_frame(ctx, points, selected, events);
        }
    }

    fn type_into(
        ctx: &egui::Context,
        points: &mut [ControlPoint],
        selected: Option<usize>,
        field: InspectorField,
        text: &str,
    ) {
        click_field(ctx, points, selected, field);
        inspector_frame(ctx, points, selected, vec![egui::Event::Text(text.into())]);
    }

    #[test]
    fn typed_value_commits_on_enter_and_escape_cancels() {
        for (key, expected_s) in [(egui::Key::Enter, 0.9), (egui::Key::Escape, 0.5)] {
            let ctx = egui::Context::default();
            let mut points = three_points();

            type_into(&ctx, &mut points, Some(1), S, "90");
            assert_eq!(points[1].val()[0], 0.5, "{key:?}: applied while typing");
            inspector_frame(&ctx, &mut points, Some(1), vec![key_press(key)]);
            inspector_frame(&ctx, &mut points, Some(1), Vec::new());

            assert_eq!(points[1].val()[0], expected_s, "{key:?}");
            assert!(!ctx.wants_keyboard_input(), "{key:?}");
        }
    }

    #[test]
    fn clicking_a_field_takes_keyboard_focus() {
        let ctx = egui::Context::default();
        let mut points = three_points();
        inspector_frame(&ctx, &mut points, Some(1), Vec::new());
        assert!(!ctx.wants_keyboard_input());

        click_field(&ctx, &mut points, Some(1), V);
        inspector_frame(&ctx, &mut points, Some(1), Vec::new());

        assert!(ctx.wants_keyboard_input());
        assert_eq!(points, three_points());
    }

    #[test]
    fn clicking_a_disabled_field_takes_no_focus() {
        let ctx = egui::Context::default();
        let mut points = three_points();

        click_field(&ctx, &mut points, Some(0), T);
        click_field(&ctx, &mut points, None, S);
        inspector_frame(&ctx, &mut points, None, Vec::new());

        assert!(!ctx.wants_keyboard_input());
    }

    #[test]
    fn drawing_without_input_never_writes_the_points() {
        let original = points_at(&[
            (1.3, -0.2, 1.7, 0.0),
            (0.123_456, 0.654_321, 0.987_654, 0.333_333_3),
            (-0.4, 2.0, -0.25, 1.0),
        ]);
        let json = |points: &[ControlPoint]| serde_json::to_string(points).unwrap();
        let clamp_only = InspectorOptions {
            is_window_lock: true,
            ..FREE
        };
        let all_on = InspectorOptions {
            is_hue_middle_interpolated: true,
            is_curve_locked: true,
            is_window_lock: true,
        };
        for opts in [FREE, clamp_only, all_on] {
            for selected in [None, Some(0), Some(1), Some(2)] {
                let ctx = egui::Context::default();
                let mut points = original.clone();
                for _ in 0..3 {
                    let _ = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(SCREEN),
                            ..Default::default()
                        },
                        |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                point_inspector(ui, &mut points, selected, opts);
                            });
                        },
                    );
                }
                assert_eq!(json(&points), json(&original), "{opts:?} {selected:?}");
            }
        }
    }

    #[test]
    fn non_finite_edits_change_nothing() {
        let locked = InspectorOptions {
            is_curve_locked: true,
            ..FREE
        };
        for opts in [FREE, locked] {
            for field in InspectorField::ALL {
                for value in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
                    let mut points = three_points();
                    apply_inspector_edit(&mut points, 1, field, value, opts);
                    assert_eq!(points, three_points(), "{field:?} {value} {opts:?}");
                }
            }
        }
    }

    #[test]
    fn typing_inf_without_clamp_changes_nothing() {
        let ctx = egui::Context::default();
        let mut points = three_points();

        type_into(&ctx, &mut points, Some(1), S, "inf");
        inspector_frame(&ctx, &mut points, Some(1), vec![key_press(egui::Key::Enter)]);
        inspector_frame(&ctx, &mut points, Some(1), Vec::new());

        assert_eq!(points, three_points());
    }

    #[test]
    fn focusing_fields_and_leaving_them_unedited_writes_nothing() {
        let precise = points_at(&[
            (0.2, 0.3, 0.1, 0.0),
            (0.123_456, 0.654_321, 0.987_654, 0.333_333_3),
            (0.8, 0.8, 0.5, 1.0),
        ]);
        let click_away = |ctx: &egui::Context, points: &mut Vec<ControlPoint>| {
            for events in primary_click(Pos2::new(500.0, 250.0)) {
                inspector_frame(ctx, points, Some(1), events);
            }
            inspector_frame(ctx, points, Some(1), Vec::new());
        };
        for field in InspectorField::ALL {
            let ctx = egui::Context::default();
            let mut points = precise.clone();
            click_field(&ctx, &mut points, Some(1), field);
            click_away(&ctx, &mut points);
            assert_eq!(points, precise, "{field:?}");
        }

        let ctx = egui::Context::default();
        let mut points = precise.clone();
        click_field(&ctx, &mut points, Some(1), H);
        for _ in 0..3 {
            inspector_frame(&ctx, &mut points, Some(1), vec![key_press(egui::Key::Tab)]);
        }
        click_away(&ctx, &mut points);
        assert_eq!(points, precise, "tabbing through every field");
    }

    #[test]
    fn dragging_a_field_moves_its_value() {
        let ctx = egui::Context::default();
        let mut points = three_points();
        let start = field_pos(&inspector_frame(&ctx, &mut points, Some(1), Vec::new()), S);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let end = start + vec2(20.0, 0.0);
        for events in [
            vec![egui::Event::PointerMoved(start), button(start, true)],
            vec![egui::Event::PointerMoved(start + vec2(10.0, 0.0))],
            vec![egui::Event::PointerMoved(end)],
            vec![button(end, false)],
        ] {
            inspector_frame(&ctx, &mut points, Some(1), events);
        }

        assert!(points[1].val()[0] > 0.5, "{:?}", points[1].val());
    }

    #[test]
    fn pending_text_does_not_land_on_another_point() {
        let ctx = egui::Context::default();
        let mut points = three_points();

        type_into(&ctx, &mut points, Some(1), S, "90");
        inspector_frame(&ctx, &mut points, Some(2), Vec::new());
        for events in primary_click(Pos2::new(500.0, 250.0)) {
            inspector_frame(&ctx, &mut points, Some(2), events);
        }
        inspector_frame(&ctx, &mut points, Some(2), Vec::new());

        assert_eq!(points, three_points());
    }
}
