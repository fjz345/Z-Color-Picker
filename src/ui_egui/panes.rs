use std::{cell::RefCell, rc::Rc};

use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::{
    color_space::{BlendSpace, PickerSpace},
    common::{ColorStringCopy, SplineMode},
    datatypes::control_point::ControlPoint,
    preset::PresetHandler,
    ui_egui::{
        app::ZColorPickerAppContext,
        color_picker::{is_auto_comb_running, ZColorPickerWrapper},
        content_windows::copy_format_combo,
        theme,
        view_3d::{CurveSettings, View3d, View3dItem},
    },
};
pub struct TileUi;

impl egui_tiles::Behavior<Pane> for TileUi {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        theme::pane_card(ui, |ui| pane.ui(ui))
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        theme::PANE_GAP
    }

    fn resize_stroke(
        &self,
        style: &egui::Style,
        resize_state: egui_tiles::ResizeState,
    ) -> egui::Stroke {
        match resize_state {
            egui_tiles::ResizeState::Idle => egui::Stroke::NONE,
            egui_tiles::ResizeState::Hovering => style.visuals.widgets.hovered.bg_stroke,
            egui_tiles::ResizeState::Dragging => style.visuals.widgets.active.bg_stroke,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum Pane {
    ColorPicker(ColorPickerPane),
    ColorPickerOptionsPane(ColorPickerOptionsPane),
    View3d(View3dPane),
    Previewer(PreviewerPane),
    Log(LogPane),
}

impl ZAppPane for Pane {
    fn title(&self) -> String {
        match self {
            Pane::ColorPicker(pane) => pane.title().into(),
            Pane::ColorPickerOptionsPane(pane) => pane.title().into(),
            Pane::View3d(pane) => pane.title(),
            Pane::Previewer(pane) => pane.title().into(),
            Pane::Log(pane) => pane.title().into(),
        }
    }
    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        match self {
            Pane::ColorPicker(pane) => pane.update_ctx(new_ctx),
            Pane::ColorPickerOptionsPane(pane) => pane.update_ctx(new_ctx),
            Pane::View3d(pane) => pane.update_ctx(new_ctx),
            Pane::Previewer(pane) => pane.update_ctx(new_ctx),
            Pane::Log(pane) => pane.update_ctx(new_ctx),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        match self {
            Pane::ColorPicker(pane) => pane.ui(ui),
            Pane::ColorPickerOptionsPane(pane) => pane.ui(ui),
            Pane::View3d(pane) => pane.ui(ui),
            Pane::Previewer(pane) => pane.ui(ui),
            Pane::Log(pane) => pane.ui(ui),
        }
    }
}

pub trait ZAppPane {
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse;
    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>);
    fn title(&self) -> String {
        "Pane".to_string()
    }
}
#[derive(Serialize, Deserialize)]
pub struct ColorPickerPane {
    pub title: Option<String>,
    pub ctx: Rc<RefCell<ZColorPickerAppContext>>,
}

impl ZAppPane for ColorPickerPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        // TODO: Fix this borrowing stuff
        let mut color_picker = self.ctx.borrow().z_color_picker.borrow().clone();
        let mut mut_ctx = self.ctx.borrow_mut();
        let color_copy_format = mut_ctx.color_copy_format;
        let mut control_points = mut_ctx.control_points.clone();
        let mut spline_mode = mut_ctx.spline_mode;
        let mut picker_space = mut_ctx.picker_space;
        let mut blend_space = mut_ctx.blend_space;

        // ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::WHITE);
        ui.allocate_ui(ui.max_rect().size(), |ui| {
            let color_picker_response = color_picker.draw_ui(
                ui,
                &mut control_points,
                &mut spline_mode,
                &mut picker_space,
                &mut blend_space,
                &color_copy_format,
            );
            *mut_ctx.z_color_picker.borrow_mut() = color_picker;
            color_picker_response
        });

        mut_ctx.control_points = control_points;
        mut_ctx.spline_mode = spline_mode;
        mut_ctx.picker_space = picker_space;
        mut_ctx.blend_space = blend_space;

        return egui_tiles::UiResponse::None;
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx.clone();
    }
}

// Parses the options tile of old saved state; ZApp::upgrade_layout drops it with the old tree.
#[derive(Serialize, Deserialize)]
pub struct ColorPickerOptionsPane {}
impl ZAppPane for ColorPickerOptionsPane {
    fn ui(&mut self, _ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        egui_tiles::UiResponse::None
    }

    fn update_ctx(&mut self, _new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {}
}
#[derive(Serialize, Deserialize)]
pub struct View3dPane {
    pub title: Option<String>,
    #[serde(skip)]
    pub ctx: Rc<RefCell<ZColorPickerAppContext>>,
    #[serde(default)]
    pub view: View3d,
}
impl ZAppPane for View3dPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| "Pane".to_owned())
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        let (mut control_points, curve, mut selected_index) = {
            let ctx = self.ctx.borrow();
            let color_picker = ctx.z_color_picker.borrow();
            let curve = CurveSettings {
                spline_mode: ctx.spline_mode,
                picker_space: ctx.picker_space,
                blend_space: ctx.blend_space,
                is_auto_comb_running: is_auto_comb_running(
                    ctx.spline_mode,
                    ctx.blend_space,
                    &color_picker.options,
                ),
                is_curve_locked: color_picker.options.is_curve_locked,
                is_window_lock: color_picker.options.is_window_lock,
                is_auto_hue: color_picker.options.is_auto_hue(ctx.picker_space),
            };
            (
                ctx.control_points.clone(),
                curve,
                color_picker.last_modifying_point_index,
            )
        };
        let dragged = self
            .view
            .draw_ui(ui, &mut control_points, curve, &mut selected_index);
        let mut ctx = self.ctx.borrow_mut();
        let ctx = &mut *ctx;
        let mut color_picker = ctx.z_color_picker.borrow_mut();
        color_picker.last_modifying_point_index = selected_index;
        match dragged {
            Some(View3dItem::Point(index)) => color_picker.dragging_index = Some(index),
            Some(View3dItem::Handle(index, side)) => {
                color_picker.dragging_tangent = Some((index, side))
            }
            None => {}
        }
        if control_points != ctx.control_points {
            color_picker.apply_control_point_constraints(&mut control_points, curve.picker_space);
            ctx.control_points = control_points;
        }
        egui_tiles::UiResponse::None
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx;
    }
}
#[derive(Serialize, Deserialize)]
pub struct PreviewerPane {
    pub title: Option<String>,
    pub ctx: Rc<RefCell<ZColorPickerAppContext>>,
}
impl ZAppPane for PreviewerPane {
    fn title(&self) -> String {
        self.title.clone().unwrap_or(format!("Pane"))
    }
    fn ui(&mut self, ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        // TODO: FIX this borrowing
        let control_points = self.ctx.borrow().control_points.clone();
        let spline_mode = self.ctx.borrow().spline_mode;
        let mut mut_ctx = self.ctx.borrow_mut();

        let mut previewer = mut_ctx.previewer.clone();

        previewer.update(
            &control_points,
            spline_mode,
            mut_ctx.picker_space,
            mut_ctx.blend_space,
        );
        let response = previewer.draw_ui(ui, ColorStringCopy::HEXNOA);
        if previewer.data.control_points != control_points {
            mut_ctx.control_points = previewer.data.control_points.clone();
        }

        mut_ctx.stored_ui_responses = response;
        mut_ctx.previewer = previewer;

        return egui_tiles::UiResponse::None;
    }

    fn update_ctx(&mut self, new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {
        self.ctx = new_ctx.clone();
    }
}

// Parses the Log tile of old saved state; ZApp::upgrade_layout drops it with the old tree.
#[derive(Serialize, Deserialize)]
pub struct LogPane {}
impl ZAppPane for LogPane {
    fn ui(&mut self, _ui: &mut egui::Ui) -> egui_tiles::UiResponse {
        egui_tiles::UiResponse::None
    }

    fn update_ctx(&mut self, _new_ctx: Rc<RefCell<ZColorPickerAppContext>>) {}
}

pub fn preset_bar_ui(app_ctx: &Rc<RefCell<ZColorPickerAppContext>>, ui: &mut egui::Ui) {
    let mut mut_ctx = app_ctx.borrow_mut();
    let mut preset_handler = mut_ctx.preset_handler.clone();
    let mut control_points = mut_ctx.control_points.clone();
    let mut spline_mode = mut_ctx.spline_mode;
    let mut picker_space = mut_ctx.picker_space;
    let mut blend_space = mut_ctx.blend_space;
    let mut presets_window = mut_ctx.presets_window.clone();
    presets_window.update();

    let presets_result = presets_window.draw_content(
        ui,
        &mut preset_handler,
        &mut control_points,
        &mut spline_mode,
        &mut picker_space,
        &mut blend_space,
    );
    if presets_result.preset_result.should_apply.is_some() {
        normalize_applied_preset(
            &mut mut_ctx.z_color_picker.borrow_mut(),
            &mut preset_handler,
            &mut control_points,
            spline_mode,
            picker_space,
            blend_space,
        );
    }

    mut_ctx.presets_window = presets_window;
    mut_ctx.control_points = control_points;
    mut_ctx.spline_mode = spline_mode;
    mut_ctx.picker_space = picker_space;
    mut_ctx.blend_space = blend_space;
    mut_ctx.preset_handler = preset_handler;
}

pub fn copy_format_bar_ui(app_ctx: &Rc<RefCell<ZColorPickerAppContext>>, ui: &mut egui::Ui) {
    let mut color_copy_format = app_ctx.borrow().color_copy_format;
    copy_format_combo(ui, &mut color_copy_format);
    app_ctx.borrow_mut().color_copy_format = color_copy_format;
}

fn normalize_applied_preset(
    color_picker: &mut ZColorPickerWrapper,
    preset_handler: &mut PresetHandler,
    control_points: &mut Vec<ControlPoint>,
    spline_mode: SplineMode,
    picker_space: PickerSpace,
    blend_space: BlendSpace,
) {
    // Like enabling Auto comb: curvatures a preset stored stale are reseeded, not chased.
    color_picker.was_auto_comb_running = false;
    color_picker.pre_draw_update(control_points, spline_mode, picker_space, blend_space);
    // Twice: the hue lerp runs before the clamp, so one pass lerps from unclamped endpoint hues.
    color_picker.apply_control_point_constraints(control_points, picker_space);
    color_picker.apply_control_point_constraints(control_points, picker_space);
    preset_handler.set_selected_data_in_memory(
        control_points,
        spline_mode,
        picker_space,
        blend_space,
    );
    color_picker.last_modifying_point_index = None;
    color_picker.dragging_index = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;
    use std::fs;
    use tempfile::TempDir;

    const MODE: SplineMode = SplineMode::Bezier;
    const SPACE: PickerSpace = PickerSpace::Hsv;
    const BLEND: BlendSpace = BlendSpace::Picker;

    fn unconstrained_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 1.2), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.2, 0.5, 0.9), 1.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.1), 2.0),
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.5, 0.4), 3.0),
        ]
    }

    fn next_picker_frame(color_picker: &mut ZColorPickerWrapper, points: &mut Vec<ControlPoint>) {
        color_picker.pre_draw_update(points, MODE, SPACE, BLEND);
        color_picker.apply_control_point_constraints(points, SPACE);
    }

    struct SelectedPreset {
        tmp: TempDir,
        handler: PresetHandler,
        color_picker: ZColorPickerWrapper,
        points: Vec<ControlPoint>,
    }

    impl SelectedPreset {
        fn file_bytes(&self) -> Vec<u8> {
            fs::read(self.tmp.path().join("A.json")).unwrap()
        }
    }

    fn select_preset_with_auto_save() -> SelectedPreset {
        select_preset_with_auto_save_from(unconstrained_points())
    }

    fn select_preset_with_auto_save_from(preset_points: Vec<ControlPoint>) -> SelectedPreset {
        let tmp = TempDir::new().unwrap();
        let mut handler = PresetHandler::new(tmp.path().into());
        handler.auto_save_presets = true;
        handler
            .create_preset(&"A".to_string(), &preset_points, &MODE, SPACE, BLEND)
            .unwrap();
        handler.preset_selected_index = Some(0);
        let mut points = Vec::new();
        let mut spline_mode = SplineMode::Linear;
        let mut picker_space = PickerSpace::Okhsl;
        let mut blend_space = BlendSpace::Oklch;
        handler.apply_selected_preset(
            &mut points,
            &mut spline_mode,
            &mut picker_space,
            &mut blend_space,
        );
        assert_eq!(picker_space, SPACE);
        assert_eq!(blend_space, BLEND);
        assert_eq!(spline_mode, MODE);
        let mut color_picker = ZColorPickerWrapper::default();
        normalize_applied_preset(
            &mut color_picker,
            &mut handler,
            &mut points,
            spline_mode,
            picker_space,
            blend_space,
        );
        SelectedPreset {
            tmp,
            handler,
            color_picker,
            points,
        }
    }

    #[test]
    fn selecting_preset_with_auto_save_does_not_rewrite_it() {
        let mut selected = select_preset_with_auto_save();
        assert!(!selected
            .handler
            .is_selected_dirty(&selected.points, MODE, SPACE, BLEND));
        let before = selected.file_bytes();

        next_picker_frame(&mut selected.color_picker, &mut selected.points);
        assert!(!selected
            .handler
            .auto_save(&selected.points, MODE, SPACE, BLEND)
            .unwrap());
        assert!(!selected
            .handler
            .auto_save(&selected.points, MODE, SPACE, BLEND)
            .unwrap());
        assert_eq!(selected.file_bytes(), before);
    }

    #[test]
    fn editing_selected_preset_with_auto_save_rewrites_it() {
        let mut selected = select_preset_with_auto_save();
        let before = selected.file_bytes();

        selected.points[1].val_mut()[0] = 0.25;
        next_picker_frame(&mut selected.color_picker, &mut selected.points);
        assert!(selected
            .handler
            .auto_save(&selected.points, MODE, SPACE, BLEND)
            .unwrap());
        assert_ne!(selected.file_bytes(), before);
    }

    #[test]
    fn applying_a_preset_clears_the_selection() {
        let mut selected = select_preset_with_auto_save();
        selected.color_picker.last_modifying_point_index = Some(2);
        selected.color_picker.dragging_index = Some(1);

        normalize_applied_preset(
            &mut selected.color_picker,
            &mut selected.handler,
            &mut selected.points,
            MODE,
            SPACE,
            BLEND,
        );

        assert_eq!(selected.color_picker.last_modifying_point_index, None);
        assert_eq!(selected.color_picker.dragging_index, None);
    }

    #[test]
    fn applying_a_preset_with_auto_comb_on_reseeds_stale_curvatures_without_rewriting_it() {
        let mut combed: Vec<ControlPoint> = [(0.2, 0.5), (0.5, 0.2), (0.8, 0.5)]
            .into_iter()
            .zip([
                [[0.0, 0.1], [0.0, -0.1]],
                [[-0.1, 0.0], [0.1, 0.0]],
                [[0.0, -0.1], [0.0, 0.1]],
            ])
            .zip([0.0, 0.5, 1.0])
            .map(|(((s, v), handles), t)| {
                let mut cp = ControlPoint::new_simple(ControlPointValue::new(s, v, 0.3), t);
                *cp.tangents_mut() = handles.map(|[ds, dv]| Some([ds, dv, 0.0].into()));
                cp
            })
            .collect();
        crate::ui_egui::auto_comb::apply_auto_comb(&mut combed, Default::default());
        let mut stale = combed.clone();
        for cp in &mut stale {
            *cp.curvature_mut() = cp.curvature().map(|k| 3.0 * k);
        }
        let mut selected = select_preset_with_auto_save_from(stale);
        let before = selected.file_bytes();
        selected.color_picker.options.is_auto_comb = true;
        selected.color_picker.was_auto_comb_running = true;
        normalize_applied_preset(
            &mut selected.color_picker,
            &mut selected.handler,
            &mut selected.points,
            MODE,
            SPACE,
            BLEND,
        );

        for (applied, original) in selected.points.iter().zip(&combed) {
            assert_eq!(applied.tangents(), original.tangents());
            let [applied, original] = [applied, original].map(|cp| cp.curvature().unwrap());
            assert!(
                (applied - original).abs() <= 1e-3 * original.abs(),
                "{applied} {original}"
            );
        }
        next_picker_frame(&mut selected.color_picker, &mut selected.points);
        assert!(!selected
            .handler
            .auto_save(&selected.points, MODE, SPACE, BLEND)
            .unwrap());
        assert_eq!(selected.file_bytes(), before);
    }

    #[test]
    fn applying_a_preset_with_old_t_spaces_it_evenly_without_rewriting_it() {
        let mut all_zero_t = unconstrained_points();
        for cp in &mut all_zero_t {
            *cp.t_mut() = 0.0;
        }
        for preset_points in [unconstrained_points(), all_zero_t] {
            let mut selected = select_preset_with_auto_save_from(preset_points);
            let ts: Vec<f32> = selected.points.iter().map(|cp| *cp.t()).collect();
            assert_eq!(ts, [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]);
            assert!(!selected
                .handler
                .is_selected_dirty(&selected.points, MODE, SPACE, BLEND));
            let before = selected.file_bytes();

            next_picker_frame(&mut selected.color_picker, &mut selected.points);
            assert!(!selected
                .handler
                .auto_save(&selected.points, MODE, SPACE, BLEND)
                .unwrap());
            assert_eq!(selected.file_bytes(), before);
        }
    }

    #[test]
    fn dragging_a_previewer_band_writes_its_t_back_to_the_points() {
        let ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        ctx.borrow_mut().control_points = [0.0, 0.4, 1.0]
            .into_iter()
            .map(|t| ControlPoint::new_simple(ControlPointValue::new(t, 0.5, 0.2), t))
            .collect();
        let mut pane = PreviewerPane {
            title: None,
            ctx: ctx.clone(),
        };
        let egui_ctx = egui::Context::default();
        let mut run = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                events,
                ..Default::default()
            };
            let _ = egui_ctx.run(input, |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| pane.ui(ui));
            });
        };
        run(Vec::new());
        let band_strip = ctx.borrow().stored_ui_responses.get_rects()[0];
        let start = band_strip.lerp_inside(egui::vec2(0.45, 0.5));
        let end = start + egui::vec2(0.2 * band_strip.width(), 0.0);
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        run(vec![egui::Event::PointerMoved(start)]);
        run(vec![button(start, true)]);
        for step in 1..=3 {
            run(vec![egui::Event::PointerMoved(
                start.lerp(end, step as f32 / 3.0),
            )]);
        }
        run(vec![button(end, false)]);

        let t = *ctx.borrow().control_points[1].t();
        assert!((t - 0.6).abs() < 1e-4, "{t}");
    }

    #[test]
    fn applying_a_preset_with_auto_comb_does_not_rewrite_it() {
        let tmp = TempDir::new().unwrap();
        let mut handler = PresetHandler::new(tmp.path().into());
        handler.auto_save_presets = true;
        let outside_until_clamped = [(0.2, 0.2), (1.2, 0.6), (0.6, 0.9), (0.3, 0.6)]
            .map(|(s, v)| ControlPoint::new_simple(ControlPointValue::new(s, v, 0.3), 0.0));
        handler
            .create_preset(
                &"A".to_string(),
                &outside_until_clamped.to_vec(),
                &MODE,
                SPACE,
                BLEND,
            )
            .unwrap();
        handler.preset_selected_index = Some(0);
        let before = fs::read(tmp.path().join("A.json")).unwrap();
        let mut points = Vec::new();
        let mut spline_mode = SplineMode::Linear;
        let mut picker_space = PickerSpace::Okhsl;
        let mut blend_space = BlendSpace::Oklch;
        handler.apply_selected_preset(
            &mut points,
            &mut spline_mode,
            &mut picker_space,
            &mut blend_space,
        );
        let mut color_picker = ZColorPickerWrapper {
            options: crate::ui_egui::app::ZColorPickerOptions {
                is_auto_comb: true,
                ..Default::default()
            },
            ..Default::default()
        };
        normalize_applied_preset(
            &mut color_picker,
            &mut handler,
            &mut points,
            spline_mode,
            picker_space,
            blend_space,
        );

        next_picker_frame(&mut color_picker, &mut points);
        assert!(!handler.auto_save(&points, MODE, SPACE, BLEND).unwrap());
        assert_eq!(fs::read(tmp.path().join("A.json")).unwrap(), before);
    }

    #[test]
    fn choosing_a_picker_space_converts_the_points_and_keeps_their_colors() {
        let ctx = Rc::new(RefCell::new(<ZColorPickerAppContext as Default>::default()));
        ctx.borrow_mut().control_points = [(0.0, 0.3, 0.1), (0.4, 0.5, 0.9)]
            .into_iter()
            .map(|(t, s, h)| ControlPoint::new_simple(ControlPointValue::new(s, 0.7, h), t))
            .collect();
        let colors = |space| -> Vec<[f32; 3]> {
            let ctx = ctx.borrow();
            ctx.control_points
                .iter()
                .map(|cp| cp.val().linear(space))
                .collect()
        };
        let before = colors(PickerSpace::Hsv);
        let mut pane = ColorPickerPane {
            title: None,
            ctx: ctx.clone(),
        };
        let egui_ctx = egui::Context::default();
        let mut run = |events: Vec<egui::Event>| {
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            egui_ctx.run(input, |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| pane.ui(ui));
            })
        };
        let text_center = |output: &egui::FullOutput, wanted: &str| {
            output
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::Shape::Text(text) if text.galley.text() == wanted => {
                        Some(text.galley.rect.translate(text.pos.to_vec2()).center())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{wanted:?} not painted"))
        };
        let click = |pos| {
            let button = move |pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            };
            [
                vec![egui::Event::PointerMoved(pos), button(true)],
                vec![button(false)],
            ]
        };

        let output = run(Vec::new());
        for events in click(text_center(&output, "HSV")) {
            run(events);
        }
        let popup = run(Vec::new());
        for events in click(text_center(&popup, "OKHSL")) {
            run(events);
        }

        assert_eq!(ctx.borrow().picker_space, PickerSpace::Okhsl);
        for (after, before) in colors(PickerSpace::Okhsl).into_iter().zip(before) {
            for c in 0..3 {
                assert!(
                    (after[c] - before[c]).abs() < 1e-4,
                    "{after:?} != {before:?}"
                );
            }
        }
    }

    #[test]
    fn old_log_pane_json_still_loads() {
        let json = r#"{"title":"Log","log_buffer":[],"scroll_to_bottom":true}"#;
        assert!(serde_json::from_str::<LogPane>(json).is_ok());
    }
}
