use std::ops::RangeInclusive;

use eframe::egui::{DragValue, Ui};
use eframe::epaint::Vec2;

use crate::{
    color_space::{ab_to_square, square_to_ab, PickerSpace, OK_SQUARE_CHROMA},
    datatypes::control_point::{ControlPoint, ControlPointValue},
    ui_egui::{
        color_picker::{
            apply_group_delta_sv, limit_group_delta, limit_group_delta_to_srgb,
            HUE_DRAG_MAX_FRACTION, NO_SELECTION_HINT,
        },
        control_points::{clip_points_to_srgb, drag_t, t_drag_bounds},
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

    fn label(self, picker_space: PickerSpace) -> &'static str {
        let is_oklab = !picker_space.is_z_hue();
        match self {
            Self::H if is_oklab => "L",
            Self::H => "H",
            Self::S if is_oklab => "a",
            Self::S if matches!(picker_space, PickerSpace::Oklch { .. }) => "C",
            Self::S => "S",
            Self::V if is_oklab => "b",
            Self::V if picker_space.is_y_lightness() => "L",
            Self::V => "V",
            Self::T => "T",
        }
    }

    fn unit(self, picker_space: PickerSpace) -> FieldUnit {
        let is_oklab = !picker_space.is_z_hue();
        let is_oklch = matches!(picker_space, PickerSpace::Oklch { .. });
        match self {
            Self::H if is_oklab => FieldUnit::Lightness,
            Self::H => FieldUnit::Degrees,
            Self::S | Self::V if is_oklab => FieldUnit::OklabAxis,
            Self::S if is_oklch => FieldUnit::Chroma,
            Self::V if is_oklch => FieldUnit::Lightness,
            Self::S | Self::V => FieldUnit::Percent,
            Self::T => FieldUnit::Position,
        }
    }

    fn speed(self, picker_space: PickerSpace) -> f32 {
        let stored_per_point = match self {
            Self::H if picker_space.is_z_hue() => 1.0 / 360.0,
            Self::H => 0.01,
            Self::S | Self::V | Self::T => 0.005,
        };
        let full_range = self.unit(picker_space).full_range();
        stored_per_point * (full_range.end() - full_range.start())
    }

    fn hint(self, picker_space: PickerSpace) -> &'static str {
        let is_oklab = !picker_space.is_z_hue();
        match self {
            Self::H if is_oklab => {
                "Oklab lightness, 0 (black) to 1 (white): the point's place on the bar"
            }
            Self::H => "Hue in degrees",
            Self::S if is_oklab => {
                "a: the point's x in the square, green below 0 to red above 0; 0 is gray"
            }
            Self::V if is_oklab => {
                "b: the point's y in the square, blue below 0 to yellow above 0; 0 is gray"
            }
            Self::S if matches!(picker_space, PickerSpace::Oklch { .. }) => {
                "Chroma: the point's x in the square, 0 (gray) at its left edge"
            }
            Self::S => {
                "Saturation: the point's x in the square. The HSV readout derives S from the \
                 sRGB bytes, so the two can differ"
            }
            Self::V if picker_space.is_y_lightness() => "Lightness: the point's y in the square",
            Self::V => "Value: the point's y in the square",
            Self::T => "T: the point's position along the gradient, between its neighbours",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FieldUnit {
    Degrees,
    Percent,
    Position,
    Lightness,
    Chroma,
    OklabAxis,
}

impl FieldUnit {
    fn shown(self, stored: f32) -> f32 {
        match self {
            Self::Degrees => stored * 360.0,
            Self::Percent => stored * 100.0,
            Self::Position | Self::Lightness => stored,
            Self::Chroma => stored * OK_SQUARE_CHROMA,
            Self::OklabAxis => square_to_ab(stored),
        }
    }

    fn stored(self, shown: f32) -> f32 {
        match self {
            Self::Degrees => shown / 360.0,
            Self::Percent => shown / 100.0,
            Self::Position | Self::Lightness => shown,
            Self::Chroma => shown / OK_SQUARE_CHROMA,
            Self::OklabAxis => ab_to_square(shown),
        }
    }

    fn full_range(self) -> RangeInclusive<f32> {
        self.shown(0.0)..=self.shown(1.0)
    }

    fn suffix(self) -> &'static str {
        match self {
            Self::Degrees => "°",
            Self::Percent => "%",
            Self::Position | Self::Lightness | Self::Chroma | Self::OklabAxis => "",
        }
    }

    fn decimals(self) -> RangeInclusive<usize> {
        match self {
            Self::Degrees | Self::Percent => 0..=1,
            Self::Position => 0..=3,
            Self::Lightness | Self::Chroma | Self::OklabAxis => 3..=3,
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
    pub picker_space: PickerSpace,
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
    let [h_unit, s_unit, v_unit] = [InspectorField::H, InspectorField::S, InspectorField::V]
        .map(|f| f.unit(opts.picker_space));
    let sv_range = |unit: FieldUnit| {
        if opts.is_window_lock {
            unit.full_range()
        } else {
            f32::NEG_INFINITY..=f32::INFINITY
        }
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
            value: Some(h_unit.shown(opts.picker_space.z_on_bar(val[2]))),
            range: h_unit.full_range(),
            disabled_reason: (opts.is_hue_middle_interpolated && is_middle)
                .then_some(AUTO_HUE_HINT),
        },
        FieldState {
            value: Some(s_unit.shown(val[0])),
            range: sv_range(s_unit),
            disabled_reason: None,
        },
        FieldState {
            value: Some(v_unit.shown(val[1])),
            range: sv_range(v_unit),
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
    let stored = field.unit(opts.picker_space).stored(value);
    let z = &mut points[index].val_mut().val[2];
    match field {
        InspectorField::H if opts.picker_space.is_z_hue() => {
            *z = z.floor() + stored.clamp(0.0, HUE_DRAG_MAX_FRACTION);
        }
        InspectorField::H => *z = stored.clamp(0.0, 1.0),
        InspectorField::S => set_sv(points, index, 0, stored, opts),
        InspectorField::V => set_sv(points, index, 1, stored, opts),
        InspectorField::T => {
            drag_t(points, index, value);
            return;
        }
    }
    let moves_every_point = opts.is_curve_locked && field != InspectorField::H;
    let moved = if moves_every_point {
        &mut points[..]
    } else {
        &mut points[index..=index]
    };
    clip_points_to_srgb(moved, opts.picker_space);
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
    delta_sv = limit_group_delta_to_srgb(points, delta_sv, opts.picker_space);
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
                ui.label(field.label(opts.picker_space));
                let unit = field.unit(opts.picker_space);
                let max_decimals = *unit.decimals().end();
                let mut value = state.value.unwrap_or_default();
                let drag = DragValue::new(&mut value)
                    .range(state.range)
                    .speed(field.speed(opts.picker_space))
                    .min_decimals(*unit.decimals().start())
                    .max_decimals(max_decimals)
                    .clamp_existing_to_range(false)
                    .update_while_editing(false);
                let drag = match state.value {
                    Some(_) => drag.suffix(unit.suffix()),
                    None => drag.custom_formatter(|_, _| "-".to_owned()),
                };
                let mut response = ui
                    .add_enabled(state.disabled_reason.is_none(), drag)
                    .on_hover_text(format!("{}. {EDIT_HINT}", field.hint(opts.picker_space)));
                if let Some(reason) = state.disabled_reason {
                    response = response.on_disabled_hover_text(reason);
                }
                let drag_start_id = response.id.with("drag_start");
                if let (true, Some(index)) = (response.drag_started(), selected) {
                    ui.data_mut(|d| d.insert_temp(drag_start_id, *points[index].val()));
                }
                let is_edited = response.dragged()
                    || state
                        .value
                        .is_some_and(|shown| !is_shown_value(value, shown, max_decimals));
                if let (true, true, Some(index)) = (response.changed(), is_edited, selected) {
                    let edits_one_value = match field {
                        InspectorField::H => true,
                        InspectorField::S | InspectorField::V => !opts.is_curve_locked,
                        InspectorField::T => false,
                    };
                    let drag_start = (response.dragged() && edits_one_value)
                        .then(|| ui.data(|d| d.get_temp::<ControlPointValue>(drag_start_id)))
                        .flatten();
                    if let Some(start) = drag_start {
                        *points[index].val_mut() = start;
                    }
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
        picker_space: PickerSpace::Hsv,
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
        inspector_frame_in(ctx, points, selected, FREE, events)
    }

    fn inspector_frame_in(
        ctx: &egui::Context,
        points: &mut [ControlPoint],
        selected: Option<usize>,
        opts: InspectorOptions,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                point_inspector(ui, points, selected, opts);
            });
        })
    }

    const OKLCH: PickerSpace = PickerSpace::Oklch {
        clip_to_srgb: false,
    };
    const OKLAB: PickerSpace = PickerSpace::Oklab {
        clip_to_srgb: false,
    };

    #[test]
    fn oklab_bar_field_is_lightness_from_0_to_1() {
        let opts = InspectorOptions {
            picker_space: OKLAB,
            ..FREE
        };
        let mut points = points_at(&[(0.5, 0.5, 0.25, 0.0), (0.5, 0.5, 1.4, 1.0)]);
        assert_eq!(state(&points, 0, opts, H).value, Some(0.25));
        assert_eq!(state(&points, 0, opts, H).range, 0.0..=1.0);
        assert_eq!(state(&points, 1, opts, H).value, Some(1.0));

        apply_inspector_edit(&mut points, 0, H, 0.42, opts);
        assert_eq!(points[0].val()[2], 0.42);
        apply_inspector_edit(&mut points, 0, H, 1.4, opts);
        assert_eq!(points[0].val()[2], 1.0);
    }

    fn in_space(picker_space: PickerSpace, is_window_lock: bool) -> InspectorOptions {
        InspectorOptions {
            picker_space,
            is_window_lock,
            ..FREE
        }
    }

    #[test]
    fn oklch_and_oklab_fields_are_absolute_chroma_a_b_and_lightness() {
        let points = points_at(&[(0.3, 0.6, 0.4, 0.0), (0.5, 0.5, 0.5, 1.0)]);
        let ab = -OK_SQUARE_CHROMA..=OK_SQUARE_CHROMA;
        let cases = [
            (OKLCH, S, 0.3 * OK_SQUARE_CHROMA, 0.0..=OK_SQUARE_CHROMA),
            (OKLCH, V, 0.6, 0.0..=1.0),
            (OKLAB, H, 0.4, 0.0..=1.0),
            (OKLAB, S, -0.4 * OK_SQUARE_CHROMA, ab.clone()),
            (OKLAB, V, 0.2 * OK_SQUARE_CHROMA, ab),
        ];
        for (space, field, value, clamped_range) in cases {
            let shown = state(&points, 0, in_space(space, true), field);
            assert!(
                (shown.value.unwrap() - value).abs() < 1e-6,
                "{space:?} {field:?}: {shown:?}"
            );
            assert_eq!(shown.range, clamped_range, "{space:?} {field:?}");
            if field != H {
                let free_range = state(&points, 0, in_space(space, false), field).range;
                assert_eq!(free_range, f32::NEG_INFINITY..=f32::INFINITY);
            }
        }
        let hue = state(&points, 0, in_space(OKLCH, true), H);
        assert_eq!(hue.value, Some(144.0));
    }

    fn painted_texts(output: &egui::FullOutput) -> Vec<&str> {
        (output.shapes.iter())
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.text()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn oklch_and_oklab_fields_show_three_decimals() {
        let oklch = ["H", "144°", "C", "0.099", "L", "0.600", "T", "0.000"];
        let oklab = ["L", "0.400", "a", "-0.132", "b", "0.066", "T", "0.000"];
        for (space, expected) in [(OKLCH, oklch), (OKLAB, oklab)] {
            let mut points = points_at(&[(0.3, 0.6, 0.4, 0.0), (0.5, 0.5, 0.5, 1.0)]);
            let output = inspector_frame_in(
                &egui::Context::default(),
                &mut points,
                Some(0),
                in_space(space, false),
                Vec::new(),
            );
            assert_eq!(painted_texts(&output), expected, "{space:?}");
        }
    }

    fn label_pos(output: &egui::FullOutput, label: &str) -> Pos2 {
        let rect = output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{label:?} label not painted"));
        rect.right_center() + vec2(12.0, 0.0)
    }

    fn commit_text(
        points: &mut [ControlPoint],
        opts: InspectorOptions,
        label: &str,
        text: Option<&str>,
    ) -> egui::FullOutput {
        let ctx = egui::Context::default();
        let output = inspector_frame_in(&ctx, points, Some(0), opts, Vec::new());
        for events in primary_click(label_pos(&output, label)) {
            inspector_frame_in(&ctx, points, Some(0), opts, events);
        }
        let typed = text.map(|text| egui::Event::Text(text.into()));
        let enter = key_press(egui::Key::Enter);
        inspector_frame_in(&ctx, points, Some(0), opts, typed.into_iter().collect());
        inspector_frame_in(&ctx, points, Some(0), opts, vec![enter]);
        inspector_frame_in(&ctx, points, Some(0), opts, Vec::new())
    }

    #[test]
    fn typed_absolute_values_land_in_the_square_and_show_as_typed() {
        let cases = [
            (OKLCH, "C", "0.2", 0, 0.2 / OK_SQUARE_CHROMA),
            (OKLCH, "L", "0.7", 1, 0.7),
            (OKLAB, "a", "-0.1", 0, ab_to_square(-0.1)),
            (OKLAB, "b", "0.25", 1, ab_to_square(0.25)),
            (OKLAB, "L", "0.25", 2, 0.25),
        ];
        for (space, label, text, axis, stored) in cases {
            let opts = in_space(space, true);
            let mut points = points_at(&[(0.3, 0.6, 0.4, 0.0), (0.5, 0.5, 0.5, 1.0)]);

            let output = commit_text(&mut points, opts, label, Some(text));
            let committed = points.clone();
            assert!(
                (committed[0].val()[axis] - stored).abs() < 1e-6,
                "{space:?} {label}: {:?}",
                committed[0].val()
            );
            let shown = format!("{:.3}", text.parse::<f32>().unwrap());
            assert!(
                painted_texts(&output).contains(&shown.as_str()),
                "{space:?} {label}: {shown} not in {:?}",
                painted_texts(&output)
            );

            commit_text(&mut points, opts, label, None);
            assert_eq!(points, committed, "{space:?} {label}: recommitted");
        }
    }

    #[test]
    fn clip_pulls_inspector_edits_into_srgb() {
        let oklch_points = [(0.35, 0.5, 0.75, 0.0), (0.15, 0.55, 0.6, 1.0)];
        let oklab_points = [(0.5, 0.25, 0.5, 0.0), (0.45, 0.55, 0.6, 1.0)];
        let oklch_edits = [(S, OK_SQUARE_CHROMA), (V, 0.0), (H, 95.0)];
        let oklab_edits = [(S, OK_SQUARE_CHROMA), (V, -OK_SQUARE_CHROMA), (H, 0.95)];
        for (space, fixture, edits) in [
            (OKLCH, oklch_points, oklch_edits),
            (OKLAB, oklab_points, oklab_edits),
        ] {
            for is_curve_locked in [false, true] {
                for (field, value) in edits {
                    let unclipped = InspectorOptions {
                        picker_space: space,
                        is_curve_locked,
                        ..FREE
                    };
                    let clipped = InspectorOptions {
                        picker_space: space.with_clip_to_srgb(true),
                        ..unclipped
                    };
                    let mut points = points_at(&fixture);
                    assert!(points.iter().all(|cp| cp.val().is_in_srgb(space)));
                    apply_inspector_edit(&mut points, 0, field, value, unclipped);
                    assert!(!points[0].val().is_in_srgb(space), "{space:?} {field:?}");

                    let mut points = points_at(&fixture);
                    apply_inspector_edit(&mut points, 0, field, value, clipped);

                    for cp in &points {
                        assert!(
                            cp.val().is_in_srgb(space),
                            "{space:?} {field:?} lock {is_curve_locked}: {cp:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn clipped_sv_edit_under_lock_stops_the_group_at_the_srgb_edge_keeping_its_shape() {
        let oklch_points = [(0.35, 0.5, 0.75, 0.0), (0.15, 0.55, 0.6, 1.0)];
        let oklab_points = [(0.5, 0.25, 0.5, 0.0), (0.45, 0.55, 0.6, 1.0)];
        for (space, fixture) in [(OKLCH, oklch_points), (OKLAB, oklab_points)] {
            let locked = InspectorOptions {
                picker_space: space.with_clip_to_srgb(true),
                is_curve_locked: true,
                ..FREE
            };
            let before = points_at(&fixture);
            let mut points = before.clone();
            apply_inspector_edit(&mut points, 0, S, 100.0, locked);

            let offset = |points: &[ControlPoint]| points[1].val().vec2() - points[0].val().vec2();
            let shape_change = (offset(&points) - offset(&before)).length();
            assert!(shape_change < 1e-6, "{space:?}: {points:?}");
            assert!(points[0].val().s() > before[0].val().s(), "{space:?}");
            assert!(points.iter().all(|cp| cp.val().is_in_srgb(space)));
            let is_at_edge = |cp: &ControlPoint| {
                let [s, v, h] = cp.val().val;
                !ControlPointValue::new(s + 1e-3, v, h).is_in_srgb(space)
            };
            assert!(points.iter().any(is_at_edge), "{space:?}: {points:?}");
        }
    }

    #[test]
    fn clipped_hue_drag_out_of_srgb_and_back_restores_the_chroma_bit_for_bit() {
        let opts = InspectorOptions {
            picker_space: OKLCH.with_clip_to_srgb(true),
            ..FREE
        };
        let original = points_at(&[(0.35, 0.5, 0.75, 0.0), (0.15, 0.55, 0.6, 1.0)]);
        assert!(original[0].val().is_in_srgb(OKLCH));
        assert!(!ControlPointValue::new(0.35, 0.5, 0.55).is_in_srgb(OKLCH));
        let ctx = egui::Context::default();
        let mut points = original.clone();
        let start = field_pos(
            &inspector_frame_in(&ctx, &mut points, Some(0), opts, Vec::new()),
            H,
        );
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let [halfway, out] = [36.0, 72.0].map(|dx| start - vec2(dx, 0.0));
        for events in [
            vec![egui::Event::PointerMoved(start), button(start, true)],
            vec![egui::Event::PointerMoved(halfway)],
            vec![egui::Event::PointerMoved(out)],
            vec![egui::Event::PointerMoved(halfway)],
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, false)],
        ] {
            inspector_frame_in(&ctx, &mut points, Some(0), opts, events);
        }

        assert_eq!(points, original);
    }

    #[test]
    fn labels_follow_the_picker_space() {
        for (picker_space, expected) in [
            (PickerSpace::Hsv, ["H", "S", "V", "T"]),
            (PickerSpace::Hsl, ["H", "S", "L", "T"]),
            (PickerSpace::Okhsv, ["H", "S", "V", "T"]),
            (PickerSpace::Okhsl, ["H", "S", "L", "T"]),
            (OKLCH, ["H", "C", "L", "T"]),
            (OKLAB, ["L", "a", "b", "T"]),
        ] {
            let ctx = egui::Context::default();
            let opts = InspectorOptions {
                picker_space,
                ..FREE
            };
            let output = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    point_inspector(ui, &mut three_points(), Some(0), opts);
                });
            });
            let labels: Vec<&str> = output
                .shapes
                .iter()
                .filter_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) => Some(text.galley.text()),
                    _ => None,
                })
                .filter(|text| text.len() == 1 && text.chars().all(char::is_alphabetic))
                .collect();
            assert_eq!(labels, expected, "{picker_space:?}");
        }
    }

    fn field_pos(output: &egui::FullOutput, field: InspectorField) -> Pos2 {
        label_pos(output, field.label(FREE.picker_space))
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
            picker_space: PickerSpace::Okhsl,
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
