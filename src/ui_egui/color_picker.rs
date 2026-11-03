use crate::{
    color_space::{
        linear_to_oklab, oklab_to_oklch, srgb_to_hsl, srgb_to_linear, BlendSpace, PickerSpace,
    },
    common::{ColorStringCopy, SplineMode},
    datatypes::{
        control_point::{create_tangent_for_control_point, ControlPoint},
        hsv_key_value::HsvKeyValue,
    },
    ui_egui::{
        app::ZColorPickerOptions,
        auto_comb::apply_auto_comb,
        color_readout::{color_readout, color_readout_height},
        content_windows::{comb_row, picker_space_combo, points_row},
        control_points::{
            control_point_outline, control_point_pos, convert_picker_space, drag_grab_offset_x,
            drag_t, gradient_color_at, insert_point, normalize_t, remove_point, t_at_pointer,
            ui_ordered_control_points, ControlPointUiResult, CONTROL_POINT_FILL_RADIUS,
        },
        curvature::draw_curvature_comb,
        gradient::color_function_gradient,
        inspector::{point_inspector, InspectorOptions},
        ui_common::black_or_white_against,
    },
};
use eframe::{
    egui::{
        self, color_picker::show_color, lerp, pos2, CursorIcon, Frame, Label, Layout, Margin,
        PointerButton, Pos2, Rect, Response, RichText, Sense, Shape, Spacing, Stroke, StrokeKind,
        TextStyle, Ui, UiBuilder, Widget, WidgetInfo,
    },
    epaint::{vec2, Color32, Vec2},
};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

use crate::{
    math::hue_lerp,
    ui_egui::curves::ui_ordered_spline_gradient,
    ui_egui::ui_common::{color_field, color_strip},
};

pub struct MainColorPickerCtx<'a> {
    pub control_points: &'a mut Vec<ControlPoint>,
    pub spline_mode: SplineMode,
    pub picker_space: &'a mut PickerSpace,
    pub blend_space: BlendSpace,
    pub color_copy_format: ColorStringCopy,
    pub last_modifying_point_index: &'a mut Option<usize>,
    pub dragging_index: &'a mut Option<usize>,
    pub dragging_tangent: &'a mut Option<(usize, usize)>,
    pub control_point_right_clicked: &'a mut Option<usize>,
    pub copy_request: &'a mut Option<(Pos2, ColorStringCopy)>,
    pub options: &'a mut ZColorPickerOptions,
    pub auto_comb_unsolved: usize,
}

pub struct ZColorPicker<'a> {
    pub ctx: &'a mut MainColorPickerCtx<'a>,
}

impl<'a> Widget for ZColorPicker<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let points_before_draw = self.ctx.control_points.clone();
        let mut response = self.picker_ui(ui);
        if *self.ctx.control_points != points_before_draw {
            response.mark_changed();
        }
        response.widget_info(|| WidgetInfo::new(egui::WidgetType::Other));
        response
    }
}

impl<'a> ZColorPicker<'a> {
    pub fn new(ctx: &'a mut MainColorPickerCtx<'a>) -> Self {
        Self { ctx }
    }

    fn picker_ui(&mut self, ui: &mut Ui) -> Response {
        let spacing = ui.spacing();
        let strip_ratio = spacing.interact_size.y / spacing.slider_width;
        let option_rows_id = ui.id().with("option_rows_height");
        let option_rows_height = ui
            .data(|data| data.get_temp(option_rows_id))
            .unwrap_or(2.0 * spacing.interact_size.y + spacing.item_spacing.y);
        // Must match main_color_picker's stack inside the margins: header, inspector, color readout, hue, T, square, and the overlays' add_space(8.0); then the option rows.
        let row_count = 1.0;
        let gap_count = 6.0;
        let overlay_space = 2.0 * 8.0;
        let fixed_height = header_height(spacing)
            + row_count * spacing.interact_size.y
            + color_readout_height(spacing)
            + gap_count * spacing.item_spacing.y
            + overlay_space
            + option_rows_height;
        // The width-limited inset lines the rows up with a full-width square and keeps their wrapping independent of the square's size.
        let (_, rows_margin) =
            picker_layout(vec2(ui.available_width(), f32::INFINITY), strip_ratio, 0.0);
        let (side, margin) = picker_layout(ui.available_size(), strip_ratio, fixed_height);
        let desired_size = vec2(side, side);
        if let Some(last_modified_index) = *self.ctx.last_modifying_point_index {
            if self.ctx.control_points.len() == 0 {
                *self.ctx.last_modifying_point_index = None;
            } else {
                *self.ctx.last_modifying_point_index =
                    Some(last_modified_index.clamp(0, self.ctx.control_points.len() - 1));
            }
        }

        let centering = ((ui.available_width() - side) / 2.0 - margin.x).max(0.0);
        let column = ui.available_rect_before_wrap();
        let column = column.with_min_x(column.min.x + centering);
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(column), |ui| {
                Frame::NONE
                    .inner_margin(Margin::symmetric(margin.x as i8, margin.y as i8))
                    .show(ui, |ui| main_color_picker(ui, desired_size, self.ctx))
                    .inner
            })
            .inner;

        let laid_out_rows_height = Frame::NONE
            .inner_margin(Margin::symmetric(rows_margin.x as i8, 0))
            .show(ui, |ui| {
                points_row(ui, self.ctx.options);
                comb_row(
                    ui,
                    self.ctx.options,
                    self.ctx.spline_mode,
                    self.ctx.blend_space,
                    self.ctx.auto_comb_unsolved,
                );
            })
            .response
            .rect
            .height();
        if (laid_out_rows_height - option_rows_height).abs() > 0.5 {
            ui.data_mut(|data| data.insert_temp(option_rows_id, laid_out_rows_height));
            ui.ctx()
                .request_discard("picker option rows changed height");
        }
        response
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZColorPickerWrapper {
    pub last_modifying_point_index: Option<usize>,
    pub dragging_index: Option<usize>,
    pub right_clicked_on_index: Option<usize>,
    pub options: ZColorPickerOptions,
    #[serde(skip)]
    pub copy_request: Option<(Pos2, ColorStringCopy)>,
    #[serde(skip)]
    pub dragging_tangent: Option<(usize, usize)>,
    #[serde(skip)]
    pub auto_comb_unsolved: usize,
}

impl Default for ZColorPickerWrapper {
    fn default() -> Self {
        let new_color_picker = Self {
            last_modifying_point_index: None,
            dragging_index: None,
            right_clicked_on_index: None,
            options: ZColorPickerOptions::default(),
            copy_request: None,
            dragging_tangent: None,
            auto_comb_unsolved: 0,
        };

        new_color_picker
    }
}

impl ZColorPickerWrapper {
    pub fn pre_draw_update(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: SplineMode,
        blend_space: BlendSpace,
    ) {
        normalize_t_with_warning(control_points);
        if spline_mode == SplineMode::Bezier {
            // Force init tangents
            for control_point in control_points.iter_mut() {
                for tang in control_point.tangents_mut().iter_mut() {
                    if tang.is_none() {
                        *tang = Some(create_tangent_for_control_point());
                    }
                }
            }
        }
        self.auto_comb_unsolved = 0;
        if spline_mode == SplineMode::Bezier
            && blend_space == BlendSpace::Picker
            && self.options.is_auto_comb
        {
            // Constrain first: a later clamp would move solved points and unsettle a loaded preset.
            self.apply_control_point_constraints(control_points);
            self.auto_comb_unsolved = apply_auto_comb(control_points, self.dragging_tangent);
        }
    }

    pub fn draw_ui(
        &mut self,
        ui: &mut Ui,
        control_points: &mut Vec<ControlPoint>,
        spline_mode: SplineMode,
        picker_space: &mut PickerSpace,
        blend_space: BlendSpace,
        color_copy_format: &ColorStringCopy,
    ) -> Response {
        let inner_response = ui.vertical(|ui| {
            self.pre_draw_update(control_points, spline_mode, blend_space);

            let mut ctx = MainColorPickerCtx {
                control_points: control_points,
                spline_mode: spline_mode,
                picker_space,
                blend_space,
                color_copy_format: *color_copy_format,
                last_modifying_point_index: &mut self.last_modifying_point_index,
                dragging_index: &mut self.dragging_index,
                dragging_tangent: &mut self.dragging_tangent,
                control_point_right_clicked: &mut self.right_clicked_on_index,
                copy_request: &mut self.copy_request,
                options: &mut self.options,
                auto_comb_unsolved: self.auto_comb_unsolved,
            };

            let color_picker_widget: ZColorPicker<'_> = ZColorPicker::new(&mut ctx);
            let main_color_picker_response = ui.add(color_picker_widget);

            self.post_draw(control_points, &main_color_picker_response);

            main_color_picker_response
        });

        inner_response.inner
    }

    // TODO: SHOULD MOVE THIS REMOVE LOGIC OUT SIDE OF THE COLOR PICKER
    pub fn remove_control_point(&mut self, control_points: &mut Vec<ControlPoint>, index: usize) {
        remove_point(control_points, index);
        log::info!("CP {} removed, new len {}", index, control_points.len());
        self.last_modifying_point_index =
            selection_after_removal(self.last_modifying_point_index, index, control_points.len());
        self.dragging_index = None;
    }

    // TODO: THIS TOO
    pub fn spawn_control_point(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        cp: ControlPoint,
    ) {
        let new_index = self.last_modifying_point_index.map_or_else(
            || {
                if self.options.is_insert_right {
                    control_points.len()
                } else {
                    0
                }
            },
            |idx| {
                if self.options.is_insert_right {
                    idx + 1
                } else {
                    idx
                }
            },
        );

        self.dragging_index = None;

        insert_point(control_points, new_index, cp);
        let cp = &control_points[new_index];
        log::info!(
            "ControlPoint#{} spawned @[{}]{},{},{}",
            control_points.len() - 1,
            cp.t(),
            cp.val()[0],
            cp.val()[1],
            cp.val()[2],
        );
        // Adding keys messes with the indicies
        self.last_modifying_point_index = Some(new_index);
    }

    fn apply_interpolate_hue_middle(&mut self, control_points: &mut Vec<ControlPoint>) {
        let num_points = control_points.len();
        if num_points >= 2 {
            let points = &mut control_points[..];

            let first_index = 0;
            let last_index = points.len() - 1;
            let first_hue = points[first_index].val()[2];
            let last_hue: f32 = points[last_index].val()[2];

            for cp in &mut points[1..last_index] {
                let hue = hue_lerp(first_hue, last_hue, *cp.t());
                cp.val_mut()[2] = hue;
            }
        }
    }
    fn clamp_control_points(&mut self, control_points: &mut Vec<ControlPoint>) {
        for cp in control_points.iter_mut() {
            cp.val_mut()[0] = cp.val()[0].clamp(0.0, 1.0);
            cp.val_mut()[1] = cp.val()[1].clamp(0.0, 1.0);
            cp.val_mut()[2] = cp.val()[2].rem_euclid(1.0);
        }
    }

    pub fn apply_control_point_constraints(&mut self, control_points: &mut Vec<ControlPoint>) {
        normalize_t_with_warning(control_points);
        if self.options.is_hue_middle_interpolated {
            self.apply_interpolate_hue_middle(control_points);
        }
        if self.options.is_window_lock {
            self.clamp_control_points(control_points);
        }
    }

    fn post_draw(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        z_color_picker_response: &Response,
    ) {
        self.apply_control_point_constraints(control_points);

        match self.right_clicked_on_index {
            Some(index) => {
                self.remove_control_point(control_points, index);
            }
            _ => {}
        }
        self.handle_doubleclick_event(control_points, z_color_picker_response);
    }

    pub fn handle_doubleclick_event(
        &mut self,
        control_points: &mut Vec<ControlPoint>,
        z_color_picker_response: &Response,
    ) {
        if !z_color_picker_response.double_clicked_by(PointerButton::Primary) {
            return;
        }
        let Some(pos) = z_color_picker_response.interact_pointer_pos() else {
            return;
        };
        let rect = z_color_picker_response.rect;
        if !rect.contains(pos) {
            return;
        }
        let click_normalized = ((pos - rect.min) / rect.size()).to_pos2();
        if let Some(cp) = point_to_spawn(
            control_points,
            click_normalized,
            rect.size(),
            CONTROL_POINT_FILL_RADIUS,
        ) {
            self.spawn_control_point(control_points, cp);
            self.apply_control_point_constraints(control_points);
        }
    }
}

fn normalize_t_with_warning(control_points: &mut [ControlPoint]) {
    if normalize_t(control_points) {
        log::warn!("Control point T was not increasing from 0 to 1; spaced the points evenly");
    }
}

fn selection_after_removal(
    selected: Option<usize>,
    removed: usize,
    new_len: usize,
) -> Option<usize> {
    let last = new_len.checked_sub(1)?;
    let selected = selected?;
    let kept = match selected.cmp(&removed) {
        Ordering::Less => selected,
        Ordering::Greater => selected - 1,
        Ordering::Equal => removed.saturating_sub(1),
    };
    Some(kept.min(last))
}

fn point_to_spawn(
    control_points: &[ControlPoint],
    click_normalized: Pos2,
    rect_size: Vec2,
    min_dist_px: f32,
) -> Option<ControlPoint> {
    let click = click_normalized.clamp(Pos2::ZERO, pos2(1.0, 1.0));
    let screen_dist = |cp: &ControlPoint| ((control_point_pos(cp) - click) * rect_size).length();
    let nearest = control_points
        .iter()
        .map(|cp| (cp, screen_dist(cp)))
        .min_by(|(_, a), (_, b)| a.total_cmp(b));
    let hue = match nearest {
        None => 0.0,
        Some((_, dist)) if dist <= min_dist_px => return None,
        Some((cp, _)) => cp.val().h(),
    };
    Some(ControlPoint::new_simple(
        [click.x, 1.0 - click.y, hue].into(),
        0.0,
    ))
}

pub fn format_color_as(
    color: Color32,
    format_type: ColorStringCopy,
    no_alpha: Option<bool>,
) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let alpha = a as f32 / 255.0;
    let srgb = [r, g, b].map(|c| c as f32 / 255.0);
    let linear = srgb_to_linear(srgb);
    let [linear_r, linear_g, linear_b] = linear;
    let [hue, saturation, value] = srgb_to_rounded_hsv(r, g, b);
    match format_type {
        ColorStringCopy::HEX if no_alpha == Some(true) => format!("{r:02X}{g:02X}{b:02X}"),
        ColorStringCopy::HEX => format!("{a:02X}{r:02X}{g:02X}{b:02X}"),
        ColorStringCopy::HEXNOA => format!("{r:02X}{g:02X}{b:02X}"),
        ColorStringCopy::SRGBHEX => format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
        ColorStringCopy::SRGB => format!("rgb({r}, {g}, {b})"),
        ColorStringCopy::SRGBA => format!("rgba({r}, {g}, {b}, {alpha:.2})"),
        ColorStringCopy::RGB => format!("rgb({linear_r:.3}, {linear_g:.3}, {linear_b:.3})"),
        ColorStringCopy::RGBA => {
            format!("rgba({linear_r:.3}, {linear_g:.3}, {linear_b:.3}, {alpha:.3})")
        }
        ColorStringCopy::HSV => format!("hsv({hue}, {saturation}%, {value}%)"),
        ColorStringCopy::HSVA => format!("hsva({hue}, {saturation}%, {value}%, {alpha:.2})"),
        ColorStringCopy::INT => u32::from_be_bytes([a, r, g, b]).to_string(),
        ColorStringCopy::FLOAT => format!(
            "{:.3}, {:.3}, {:.3}, {alpha:.3}",
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0
        ),
        // Fewest decimals that map every 8-bit color back to itself.
        ColorStringCopy::HSL => {
            let [h, s, l] = srgb_to_hsl(srgb);
            let [h, s, l] = [h * 360.0, s * 100.0, l * 100.0].map(|v| css_number(v, 1));
            format!("hsl({h} {s}% {l}%)")
        }
        ColorStringCopy::OKLAB => {
            let [ok_l, ok_a, ok_b] = linear_to_oklab(linear).map(|v| css_number(v, 5));
            format!("oklab({ok_l} {ok_a} {ok_b})")
        }
        ColorStringCopy::OKLCH => {
            let [l, c, h] = oklab_to_oklch(linear_to_oklab(linear));
            let [l, c, h] = [css_number(l, 5), css_number(c, 5), css_number(h * 360.0, 2)];
            format!("oklch({l} {c} {h})")
        }
    }
}

fn css_number(value: f32, decimals: usize) -> String {
    assert!(decimals > 0);
    let fixed = format!("{value:.decimals$}");
    match fixed.trim_end_matches('0').trim_end_matches('.') {
        "-0" => "0".to_owned(),
        trimmed => trimmed.to_owned(),
    }
}

const NO_SELECTION_READOUT: &str = "No point selected";

fn readout_text(color: Option<Color32>, format: ColorStringCopy) -> String {
    match color {
        Some(color) => format_color_as(color, format, None),
        None => NO_SELECTION_READOUT.to_owned(),
    }
}

fn srgb_to_rounded_hsv(r: u8, g: u8, b: u8) -> [u32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = (max - min) as f32;
    let [r_f, g_f, b_f] = [r, g, b].map(f32::from);
    let hue_sector = if max == min {
        0.0
    } else if max == r {
        ((g_f - b_f) / delta).rem_euclid(6.0)
    } else if max == g {
        (b_f - r_f) / delta + 2.0
    } else {
        (r_f - g_f) / delta + 4.0
    };
    let saturation = if max == 0 { 0.0 } else { delta / max as f32 };
    [
        (hue_sector * 60.0).round() as u32 % 360,
        (saturation * 100.0).round() as u32,
        (max as f32 / 255.0 * 100.0).round() as u32,
    ]
}

pub fn main_color_picker(
    ui: &mut Ui,
    desired_size: Vec2,
    ctx: &mut MainColorPickerCtx,
) -> Response {
    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
        let scale_factor = desired_size.x / ui.spacing().slider_width;
        let desired_size_slider_2d = scale_factor * Vec2::splat(ui.spacing().slider_width);
        let slider_2d_rect_size =
            Vec2::new(ui.spacing().slider_width, ui.spacing().interact_size.y) * scale_factor;

        let mut is_modifying_index = ctx.dragging_index.or(*ctx.last_modifying_point_index);
        let selected_color = is_modifying_index
            .and_then(|i| ctx.control_points.get(i))
            .map(|cp| cp.val().color(*ctx.picker_space));

        if let Some(anchor) = picker_header(ui, selected_color, ctx) {
            *ctx.copy_request = Some((anchor, ctx.color_copy_format));
        }
        let selected_value = is_modifying_index
            .and_then(|i| ctx.control_points.get(i))
            .map(|cp| *cp.val());
        point_inspector(
            ui,
            ctx.control_points,
            is_modifying_index,
            InspectorOptions {
                is_hue_middle_interpolated: ctx.options.is_hue_middle_interpolated,
                is_curve_locked: ctx.options.is_curve_locked,
                is_window_lock: ctx.options.is_window_lock,
                picker_space: *ctx.picker_space,
            },
        );
        if let Some(request) = color_readout(ui, selected_color) {
            *ctx.copy_request = Some(request);
        }

        let hue_to_show = selected_value.map_or(0.0, |value| value.h().rem_euclid(1.0));

        let delta_hue = handle_hue_slider(
            ui,
            hue_to_show,
            slider_2d_rect_size,
            ctx,
            &mut is_modifying_index,
        );
        if let Some(h) = delta_hue {
            if let Some(_index) = is_modifying_index {
                for cp in ctx.control_points.iter_mut() {
                    let val_mut_ref = cp.val_mut();
                    val_mut_ref.val[2] = (val_mut_ref.h() - h).rem_euclid(1.0);
                }
            }
        }

        handle_t_strip(ui, slider_2d_rect_size, ctx, &mut is_modifying_index);

        let slider_2d_response = color_field(
            ui,
            desired_size_slider_2d,
            main_color_picker_color_at_function(*ctx.picker_space, hue_to_show),
        );
        if ctx.control_points.is_empty() {
            paint_empty_picker_hint(ui, slider_2d_response.rect);
        }

        if ctx.options.is_show_curvature_comb && ctx.blend_space == BlendSpace::Picker {
            draw_curvature_comb(
                ui,
                slider_2d_response.rect,
                ctx.control_points,
                ctx.spline_mode,
                ctx.options.curvature_comb_scale,
            );
        }

        let _ = ui_ordered_spline_gradient(
            ui,
            ctx.control_points,
            ctx.spline_mode,
            *ctx.picker_space,
            ctx.blend_space,
            &slider_2d_response,
        );
        handle_control_points_ui(ui, ctx, &slider_2d_response, &mut is_modifying_index);

        slider_2d_response
    })
    .inner
}

// -------------------- Helper Functions --------------------
const SWATCH_ASPECT: f32 = 2.0;

fn header_height(spacing: &Spacing) -> f32 {
    2.0 * spacing.interact_size.y + spacing.item_spacing.y
}

fn picker_header(
    ui: &mut Ui,
    color: Option<Color32>,
    ctx: &mut MainColorPickerCtx,
) -> Option<Pos2> {
    let height = header_height(ui.spacing());
    ui.horizontal(|ui| {
        selected_color_swatch(ui, color, vec2(SWATCH_ASPECT * height, height));
        ui.vertical(|ui| {
            let copy_anchor = selected_color_readout(ui, color, ctx.color_copy_format);
            ui.horizontal(|ui| {
                let mut chosen = *ctx.picker_space;
                picker_space_combo(ui, &mut chosen);
                convert_picker_space(ctx.control_points, *ctx.picker_space, chosen);
                *ctx.picker_space = chosen;
            });
            copy_anchor
        })
        .inner
    })
    .inner
}

fn picker_side(available: Vec2, strip_ratio: f32, fixed_height: f32) -> f32 {
    let height_limited_side = (available.y - fixed_height) / (1.0 + 2.0 * strip_ratio);
    available.x.min(height_limited_side).clamp(160.0, 800.0)
}

fn picker_layout(available: Vec2, strip_ratio: f32, fixed_height: f32) -> (f32, Vec2) {
    let gizmo_ratio = strip_ratio / 4.0;
    let fill_margin = CONTROL_POINT_FILL_RADIUS.ceil();
    let gizmo_margins_ceil_slack = 2.0;
    let width_limited_side = (available.x - 2.0 * fill_margin)
        .min((available.x - gizmo_margins_ceil_slack) / (1.0 + 2.0 * gizmo_ratio));
    let side = picker_side(
        vec2(width_limited_side, available.y - 2.0 * fill_margin),
        strip_ratio,
        fixed_height,
    );
    let margin = vec2(fill_margin.max((side * gizmo_ratio).ceil()), fill_margin);
    (side, margin)
}

fn selected_color_swatch(ui: &mut Ui, color: Option<Color32>, size: Vec2) {
    match color {
        Some(color) => {
            show_color(ui, color, size).on_hover_text("Selected color");
        }
        None => {
            let (rect, response) = ui.allocate_at_least(size, Sense::hover());
            ui.painter().rect_stroke(
                rect,
                0.0,
                ui.visuals().widgets.noninteractive.bg_stroke,
                StrokeKind::Inside,
            );
            response.on_hover_text(NO_SELECTION_READOUT);
        }
    }
}

fn selected_color_readout(
    ui: &mut Ui,
    color: Option<Color32>,
    format: ColorStringCopy,
) -> Option<Pos2> {
    let text = RichText::new(readout_text(color, format));
    let hover_text = format!("Selected color as {format:?}");
    ui.horizontal(|ui| {
        if color.is_none() {
            ui.add(Label::new(text.weak()).truncate())
                .on_hover_text(hover_text);
            return None;
        }
        let response = ui
            .add(
                Label::new(text)
                    .truncate()
                    .selectable(false)
                    .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("{hover_text}. Click or Ctrl+C: copy"));
        response
            .clicked_by(PointerButton::Primary)
            .then(|| response.rect.left_bottom())
    })
    .inner
}

const EMPTY_PICKER_HINT: &str = "Double-click to add a control point";

fn paint_empty_picker_hint(ui: &Ui, square: Rect) {
    let padding = 6.0;
    let painter = ui.painter_at(square);
    let galley = painter.layout(
        EMPTY_PICKER_HINT.to_owned(),
        TextStyle::Body.resolve(ui.style()),
        Color32::WHITE,
        square.width() - 4.0 * padding,
    );
    let text_rect = Rect::from_center_size(square.center(), galley.size());
    painter.rect_filled(
        text_rect.expand(padding),
        4.0,
        Color32::from_black_alpha(170),
    );
    painter.galley(text_rect.min, galley, Color32::WHITE);
}

pub const NO_SELECTION_HINT: &str = "Select a control point first";

#[derive(Debug, Clone, Copy)]
enum Strip {
    Hue,
    T,
}

fn strip_hint(strip: Strip, has_selection: bool) -> &'static str {
    match (strip, has_selection) {
        (Strip::T, _) => {
            "T: where each point sits along the gradient. Drag a middle point's marker to move it; \
             the first and last points stay at 0 and 1"
        }
        (Strip::Hue, false) => NO_SELECTION_HINT,
        (Strip::Hue, true) => {
            "Click or drag: set the selected point's hue; the other points rotate with it"
        }
    }
}

fn t_gizmo_hint(index: usize, count: usize, t: f32) -> String {
    let point = format!("Point {} of {count}: T {t:.3}", index + 1);
    if index == 0 || index + 1 == count {
        format!("{point} (fixed). Click: select")
    } else {
        format!("{point}. Drag: move it along the gradient. Click: select")
    }
}

fn handle_hue_slider(
    ui: &mut Ui,
    shown_hue: f32,
    slider_size: Vec2,
    ctx: &mut MainColorPickerCtx,
    is_modifying_index: &mut Option<usize>,
) -> Option<f32> {
    let (hue_response, picked_hue) = color_strip(
        ui,
        slider_size,
        hue_strip_color_at_function(*ctx.picker_space),
    );
    let hue_response =
        hue_response.on_hover_text(strip_hint(Strip::Hue, is_modifying_index.is_some()));

    let picking = hue_response.clicked_by(PointerButton::Primary)
        || hue_response.dragged_by(PointerButton::Primary);
    let delta_hue = picked_hue
        .filter(|_| picking)
        .map(|picked| shown_hue - picked);

    let (_hue_response_cp, hue_selected_index) = ui_hue_control_points_overlay(
        ui,
        &hue_response,
        ctx.control_points,
        *is_modifying_index,
        ctx.options.is_hue_middle_interpolated,
        *ctx.picker_space,
    );

    if let Some(new_index) = hue_selected_index {
        *is_modifying_index = Some(new_index);
    }

    delta_hue
}

pub fn limit_group_delta(points: &[ControlPoint], delta_sv: Vec2) -> Vec2 {
    let mut min_sv = Vec2::splat(f32::INFINITY);
    let mut max_sv = Vec2::splat(f32::NEG_INFINITY);
    for cp in points {
        let sv = vec2(cp.val()[0], cp.val()[1]);
        min_sv = min_sv.min(sv);
        max_sv = max_sv.max(sv);
    }
    let lo = (-min_sv).min(Vec2::ZERO);
    let hi = (Vec2::splat(1.0) - max_sv).max(Vec2::ZERO);
    delta_sv.clamp(lo, hi)
}

pub fn apply_group_delta_sv(points: &mut [ControlPoint], delta_sv: Vec2) {
    for cp in points {
        let val = &mut cp.val_mut().val;
        val[0] += delta_sv.x;
        val[1] += delta_sv.y;
    }
}

fn drag_target_sv(pointer: Pos2, grab_offset: Vec2, rect: Rect) -> Vec2 {
    let normalized = (pointer - grab_offset - rect.min) / rect.size();
    vec2(normalized.x, 1.0 - normalized.y)
}

fn handle_control_points_ui(
    ui: &mut Ui,
    ctx: &mut MainColorPickerCtx,
    slider_2d_response: &Response,
    is_modifying_index: &mut Option<usize>,
) {
    let control_point_ui_result = ui_ordered_control_points(
        ui,
        ctx.control_points,
        *is_modifying_index,
        ctx.options.is_hue_middle_interpolated,
        &slider_2d_response,
        ctx.spline_mode == SplineMode::Bezier,
        *ctx.picker_space,
    );

    let ControlPointUiResult {
        selected_index,
        hovering_control_point,
        dragged_point,
        selected_tangent,
        dragged_tangent,
        dragged_tangent_offset,
    } = control_point_ui_result;

    // Right click
    *ctx.control_point_right_clicked = hovering_control_point.and_then(|(r, idx)| {
        if r.clicked_by(PointerButton::Secondary) {
            Some(idx)
        } else {
            None
        }
    });

    if dragged_point.is_none() {
        *ctx.dragging_index = None;
    }

    if let Some(index) = selected_index {
        *ctx.last_modifying_point_index = Some(index);
    }

    // Drag control point
    if let (Some(r), Some(index)) = (dragged_point, selected_index) {
        if r.dragged_by(PointerButton::Primary) {
            *ctx.dragging_index = selected_index;
            let rect = slider_2d_response.rect;
            let grab_offset_id = r.id.with("grab_offset");
            if r.drag_started_by(PointerButton::Primary) {
                let press_origin = ui
                    .input(|i| i.pointer.press_origin())
                    .or(r.interact_pointer_pos());
                if let Some(press_origin) = press_origin {
                    let drawn =
                        rect.lerp_inside(control_point_pos(&ctx.control_points[index]).to_vec2());
                    ui.data_mut(|d| d.insert_temp(grab_offset_id, press_origin - drawn));
                }
            }
            let grab_offset = ui
                .data(|d| d.get_temp::<Vec2>(grab_offset_id))
                .unwrap_or_default();
            if let Some(pointer) = r.interact_pointer_pos() {
                let target = drag_target_sv(pointer, grab_offset, rect);
                if ctx.options.is_curve_locked {
                    let dragged = ctx.control_points[index].val();
                    let delta_sv = target - vec2(dragged[0], dragged[1]);
                    let delta_sv = if ctx.options.is_window_lock {
                        limit_group_delta(ctx.control_points, delta_sv)
                    } else {
                        delta_sv
                    };
                    apply_group_delta_sv(ctx.control_points, delta_sv);
                } else {
                    let val = &mut ctx.control_points[index].val_mut().val;
                    val[0] = target.x;
                    val[1] = target.y;
                }
            }
        }
    }

    // Drag tangent
    *ctx.dragging_tangent = None;
    if let Some(r) = dragged_tangent {
        if r.dragged_by(PointerButton::Primary) {
            if let Some(index) = *ctx.last_modifying_point_index {
                if let (Some(tang_idx), Some(offset)) = (selected_tangent, dragged_tangent_offset) {
                    if let Some(tang) = &mut ctx.control_points[index].tangents_mut()[tang_idx] {
                        tang.val[0] = offset.x;
                        tang.val[1] = offset.y;
                        *ctx.dragging_tangent = Some((index, tang_idx));
                    }
                }
            }
        }
    }
}

fn main_color_picker_color_at_function(
    picker_space: PickerSpace,
    hue: f32,
) -> impl Fn(f32, f32) -> Color32 {
    move |x, y| HsvKeyValue::new(x, y, hue).color(picker_space)
}

fn hue_strip_color_at_function(picker_space: PickerSpace) -> impl Fn(f32) -> Color32 {
    let y = if picker_space.is_y_lightness() {
        0.5
    } else {
        1.0
    };
    move |hue| HsvKeyValue::new(1.0, y, hue).color(picker_space)
}

pub const HUE_DRAG_MAX_FRACTION: f32 = 1.0 - 1e-4;

fn hue_after_drag(
    stored_hue: f32,
    pointer_x: f32,
    grab_offset_x: f32,
    strip_left: f32,
    strip_width: f32,
) -> f32 {
    let fraction =
        ((pointer_x - grab_offset_x - strip_left) / strip_width).clamp(0.0, HUE_DRAG_MAX_FRACTION);
    stored_hue.floor() + fraction
}

pub fn ui_hue_control_points_overlay(
    ui: &mut Ui,
    parent_response: &Response,
    control_points: &mut [ControlPoint],
    modifying_control_point_index: Option<usize>,
    is_hue_middle_interpolated: bool,
    picker_space: PickerSpace,
) -> (Response, Option<usize>) {
    let container_response =
        ui.allocate_rect(parent_response.rect, Sense::focusable_noninteractive());
    const Y_OFFSET: f32 = 5.0;
    const Y_OFFSET_SELECTED: f32 = -14.0;
    ui.add_space(8.0);
    let visuals = ui.style().interact(&parent_response);

    let r = container_response.rect.height() / 4.0;

    let mut selected_key_frame = None;
    for i in 0..control_points.len() {
        let gizmo = hue_gizmo(i, control_points.len(), is_hue_middle_interpolated);
        let val = hue_strip_x_fraction(control_points[i].val().h());
        let picked_color = control_points[i].val().color(picker_space);
        // Show where the slider is at:
        let x = lerp(
            container_response.rect.left()..=container_response.rect.right(),
            val,
        );

        if gizmo == HueGizmo::Marker {
            let is_selected = modifying_control_point_index == Some(i);
            let y_offset = if is_selected {
                Y_OFFSET_SELECTED
            } else {
                Y_OFFSET
            };
            let marker_half_width = r / 4.0;
            let marker_rect = Rect::from_x_y_ranges(
                x - marker_half_width..=x + marker_half_width,
                y_offset + container_response.rect.center().y
                    ..=y_offset + container_response.rect.bottom(),
            );
            ui.painter().add(Shape::convex_polygon(
                vec![
                    marker_rect.left_top(),
                    marker_rect.right_top(),
                    marker_rect.right_bottom(),
                    marker_rect.left_bottom(),
                ],
                picked_color,
                control_point_outline(picked_color, is_selected),
            ));
            continue;
        }

        let y_offset_to_use = if let Some(index) = modifying_control_point_index {
            if i == index {
                Y_OFFSET_SELECTED
            } else {
                Y_OFFSET
            }
        } else {
            Y_OFFSET
        };

        let gizmo_rect: Vec<Pos2> = if i == 0 {
            // First
            vec![
                pos2(
                    x + r,
                    y_offset_to_use + container_response.rect.center().y + r,
                ),
                pos2(
                    x - r,
                    y_offset_to_use + container_response.rect.bottom() - r * 2.0,
                ),
                pos2(x - r, y_offset_to_use + container_response.rect.bottom()),
            ]
        } else if i == (control_points.len() - 1) {
            // Last
            vec![
                pos2(
                    x - r,
                    y_offset_to_use + container_response.rect.center().y + r,
                ),
                pos2(
                    x + r,
                    y_offset_to_use + container_response.rect.bottom() - r * 2.0,
                ),
                pos2(x + r, y_offset_to_use + container_response.rect.bottom()),
            ]
        } else {
            // Other
            vec![
                pos2(x + r, y_offset_to_use + container_response.rect.center().y),
                pos2(x + r, y_offset_to_use + container_response.rect.bottom()), // right bottom
                pos2(x - r, y_offset_to_use + container_response.rect.bottom()), // left bottom
                pos2(x - r, y_offset_to_use + container_response.rect.center().y),
            ]
        };

        let response = ui
            .interact(
                Rect::from_points(&gizmo_rect),
                container_response.id.with(i),
                Sense::click_and_drag(),
            )
            .on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);

        if response.clicked_by(PointerButton::Primary)
            || response.dragged_by(PointerButton::Primary)
        {
            selected_key_frame = Some(i);
        }
        if response.dragged_by(PointerButton::Primary) {
            let grab_offset_id = response.id.with("grab_offset");
            if response.drag_started_by(PointerButton::Primary) {
                let press_origin = ui
                    .input(|i| i.pointer.press_origin())
                    .or(response.interact_pointer_pos());
                if let Some(press_origin) = press_origin {
                    ui.data_mut(|d| d.insert_temp(grab_offset_id, press_origin.x - x));
                }
            }
            let grab_offset_x = ui
                .data(|d| d.get_temp::<f32>(grab_offset_id))
                .unwrap_or_default();
            if let Some(pointer) = response.interact_pointer_pos() {
                let strip = container_response.rect;
                let hue = &mut control_points[i].val_mut()[2];
                *hue = hue_after_drag(*hue, pointer.x, grab_offset_x, strip.left(), strip.width());
            }
        }

        ui.painter().add(Shape::convex_polygon(
            gizmo_rect,
            picked_color,
            Stroke::new(
                visuals.fg_stroke.width,
                black_or_white_against(picked_color),
            ),
        ));
    }

    (container_response, selected_key_frame)
}

fn handle_t_strip(
    ui: &mut Ui,
    slider_size: Vec2,
    ctx: &mut MainColorPickerCtx,
    is_modifying_index: &mut Option<usize>,
) {
    let gradient = gradient_color_at(
        ctx.control_points,
        ctx.spline_mode,
        *ctx.picker_space,
        ctx.blend_space,
    );
    let t_response = color_function_gradient(ui, slider_size, gradient)
        .on_hover_text(strip_hint(Strip::T, is_modifying_index.is_some()));
    let (_, t_selected_index) = ui_t_control_points_overlay(
        ui,
        &t_response,
        ctx.control_points,
        *is_modifying_index,
        *ctx.picker_space,
    );
    if let Some(new_index) = t_selected_index {
        *is_modifying_index = Some(new_index);
    }
}

fn hue_strip_x_fraction(hue: f32) -> f32 {
    hue.rem_euclid(1.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum HueGizmo {
    Editable,
    Marker,
}

fn hue_gizmo(index: usize, count: usize, auto_hue: bool) -> HueGizmo {
    let is_middle = index != 0 && index + 1 != count;
    if auto_hue && is_middle {
        HueGizmo::Marker
    } else {
        HueGizmo::Editable
    }
}

fn t_gizmo_shape(strip: Rect, x: f32, half_width: f32, index: usize, count: usize) -> Vec<Pos2> {
    let (top, middle, bottom) = (strip.top(), strip.center().y, strip.bottom());
    if index == 0 {
        vec![
            pos2(x - half_width, top),
            pos2(x + half_width / 2.0, middle),
            pos2(x - half_width, bottom),
        ]
    } else if index + 1 == count {
        vec![
            pos2(x + half_width, bottom),
            pos2(x - half_width / 2.0, middle),
            pos2(x + half_width, top),
        ]
    } else {
        let bar = Rect::from_x_y_ranges(x - half_width / 2.0..=x + half_width / 2.0, top..=bottom);
        vec![
            bar.left_top(),
            bar.right_top(),
            bar.right_bottom(),
            bar.left_bottom(),
        ]
    }
}

pub fn ui_t_control_points_overlay(
    ui: &mut Ui,
    parent_response: &Response,
    control_points: &mut [ControlPoint],
    selected: Option<usize>,
    picker_space: PickerSpace,
) -> (Response, Option<usize>) {
    let container_response =
        ui.allocate_rect(parent_response.rect, Sense::focusable_noninteractive());
    ui.add_space(8.0);
    let strip = container_response.rect;
    let half_width = strip.height() / 4.0;
    let count = control_points.len();

    let mut selected_key_frame = None;
    for i in 0..count {
        let is_end = i == 0 || i + 1 == count;
        let x = lerp(strip.x_range(), *control_points[i].t());
        let sense = if is_end {
            Sense::click()
        } else {
            Sense::click_and_drag()
        };
        let hit_rect = Rect::from_x_y_ranges(x - half_width..=x + half_width, strip.y_range());
        let mut response = ui.interact(hit_rect, container_response.id.with(i), sense);
        if !is_end {
            response = response.on_hover_and_drag_cursor(CursorIcon::ResizeHorizontal);
        }
        let t = *control_points[i].t();
        let response = response.on_hover_ui(|ui| {
            ui.label(t_gizmo_hint(i, count, t));
        });

        if response.clicked_by(PointerButton::Primary)
            || response.dragged_by(PointerButton::Primary)
        {
            selected_key_frame = Some(i);
        }
        if response.dragged_by(PointerButton::Primary) {
            let grab_offset_x = drag_grab_offset_x(ui, &response, x);
            if let Some(pointer) = response.interact_pointer_pos() {
                let target = t_at_pointer(pointer.x, grab_offset_x, strip.left(), strip.width());
                drag_t(control_points, i, target);
            }
        }

        let color = control_points[i].val().color(picker_space);
        let x = lerp(strip.x_range(), *control_points[i].t());
        let emphasized = selected == Some(i) || selected_key_frame == Some(i);
        ui.painter().add(Shape::convex_polygon(
            t_gizmo_shape(strip, x, half_width, i, count),
            color,
            control_point_outline(color, emphasized),
        ));
    }

    (container_response, selected_key_frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datatypes::control_point::ControlPointValue;
    use crate::ui_egui::control_points::MIN_T_GAP;

    fn orange() -> Color32 {
        Color32::from_rgba_unmultiplied(255, 128, 0, 204)
    }

    fn format_orange(format: ColorStringCopy) -> String {
        format_color_as(orange(), format, None)
    }

    const STRIP_RATIO: f32 = 0.18;
    const FIXED_HEIGHT: f32 = 46.0;

    fn picker_height(side: f32) -> f32 {
        side * (1.0 + 2.0 * STRIP_RATIO) + FIXED_HEIGHT
    }

    #[test]
    fn picker_side_width_limited_fills_width() {
        assert_eq!(
            picker_side(vec2(300.0, 2000.0), STRIP_RATIO, FIXED_HEIGHT),
            300.0
        );
    }

    #[test]
    fn picker_side_height_limited_fills_height() {
        let side = picker_side(vec2(2000.0, 400.0), STRIP_RATIO, FIXED_HEIGHT);
        assert!((picker_height(side) - 400.0).abs() < 1e-3, "side {side}");
    }

    #[test]
    fn picker_side_tiny_pane_clamps_to_min() {
        assert_eq!(
            picker_side(vec2(50.0, 50.0), STRIP_RATIO, FIXED_HEIGHT),
            160.0
        );
    }

    #[test]
    fn picker_side_huge_pane_clamps_to_max() {
        assert_eq!(
            picker_side(vec2(5000.0, 5000.0), STRIP_RATIO, FIXED_HEIGHT),
            800.0
        );
    }

    #[test]
    fn picker_side_unclamped_fits_available() {
        for w in [170.0, 250.0, 480.0, 799.0, 1200.0] {
            for h in [300.0, 420.0, 700.0, 1300.0] {
                let side = picker_side(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                if side > 160.0 && side < 800.0 {
                    assert!(side <= w, "{w}x{h}: side {side}");
                    assert!(picker_height(side) <= h + 1e-3, "{w}x{h}: side {side}");
                }
            }
        }
    }

    const LAYOUT_WIDTHS: [f32; 6] = [170.0, 250.0, 480.0, 799.0, 900.0, 1200.0];
    const LAYOUT_HEIGHTS: [f32; 5] = [300.0, 420.0, 700.0, 1000.0, 1400.0];

    #[test]
    fn picker_layout_margins_cover_control_point_fill() {
        for w in LAYOUT_WIDTHS {
            for h in LAYOUT_HEIGHTS {
                let (_, margin) = picker_layout(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                assert!(margin.x >= CONTROL_POINT_FILL_RADIUS, "{w}x{h}: {margin:?}");
                assert!(margin.y >= CONTROL_POINT_FILL_RADIUS, "{w}x{h}: {margin:?}");
            }
        }
    }

    #[test]
    fn picker_layout_horizontal_margin_covers_strip_gizmos() {
        for w in LAYOUT_WIDTHS {
            for h in LAYOUT_HEIGHTS {
                let (side, margin) = picker_layout(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                assert!(
                    margin.x >= side * STRIP_RATIO / 4.0,
                    "{w}x{h}: side {side}, {margin:?}"
                );
            }
        }
    }

    #[test]
    fn picker_layout_unclamped_fits_available() {
        let mut unclamped_count = 0;
        for w in LAYOUT_WIDTHS {
            for h in LAYOUT_HEIGHTS {
                let (side, margin) = picker_layout(vec2(w, h), STRIP_RATIO, FIXED_HEIGHT);
                if side > 160.0 && side < 800.0 {
                    unclamped_count += 1;
                    assert!(side + 2.0 * margin.x <= w + 1e-3, "{w}x{h}: side {side}");
                    assert!(
                        picker_height(side) + 2.0 * margin.y <= h + 1e-3,
                        "{w}x{h}: side {side}"
                    );
                }
            }
        }
        assert!(unclamped_count >= 10, "{unclamped_count}");
    }

    struct LaidOutPicker {
        square: Rect,
        pane: Rect,
        used: Rect,
        output: egui::FullOutput,
    }

    fn lay_out_picker(screen_size: Vec2) -> LaidOutPicker {
        let ctx = egui::Context::default();
        crate::ui_egui::theme::apply(&ctx);
        let mut picker = ZColorPickerWrapper::default();
        let mut points = vec![
            ControlPoint::new_simple(ControlPointValue::new(0.0, 0.0, 0.0), 0.0),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 0.0, 0.3), 0.3),
            ControlPoint::new_simple(ControlPointValue::new(0.0, 1.0, 0.6), 0.6),
            ControlPoint::new_simple(ControlPointValue::new(1.0, 1.0, 1.0), 1.0),
        ];
        let mut rects = None;
        let mut output = None;
        for _ in 0..2 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen_size)),
                ..Default::default()
            };
            output = Some(ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let pane = ui.max_rect();
                    let picker_scope = ui.scope(|ui| {
                        picker
                            .draw_ui(
                                ui,
                                &mut points,
                                SplineMode::HermiteBezier,
                                &mut PickerSpace::Hsv,
                                BlendSpace::Picker,
                                &ColorStringCopy::HEXNOA,
                            )
                            .rect
                    });
                    rects = Some((picker_scope.inner, pane, picker_scope.response.rect));
                });
            }));
        }
        let (square, pane, used) = rects.unwrap();
        LaidOutPicker {
            square,
            pane,
            used,
            output: output.unwrap(),
        }
    }

    fn assert_square_inset(screen_size: Vec2) -> LaidOutPicker {
        let picker = lay_out_picker(screen_size);
        let LaidOutPicker {
            square, pane, used, ..
        } = picker;
        let horizontal = CONTROL_POINT_FILL_RADIUS.max(square.width() * STRIP_RATIO / 4.0) - 0.5;
        let vertical = CONTROL_POINT_FILL_RADIUS - 0.5;
        assert!(
            square.width() > 160.0 && square.width() < 800.0,
            "{square:?}"
        );
        assert!(
            square.left() - pane.left() >= horizontal,
            "{square:?} in {pane:?}"
        );
        assert!(
            pane.right() - square.right() >= horizontal,
            "{square:?} in {pane:?}"
        );
        assert!(
            pane.bottom() - square.bottom() >= vertical,
            "{square:?} in {pane:?}"
        );
        assert!(used.bottom() <= pane.bottom() + 0.5, "{used:?} in {pane:?}");
        picker
    }

    #[test]
    fn picker_insets_square_in_width_limited_pane() {
        assert_square_inset(vec2(260.0, 900.0));
    }

    #[test]
    fn picker_insets_square_in_height_limited_pane() {
        let LaidOutPicker { pane, used, .. } = assert_square_inset(vec2(1000.0, 520.0));
        assert!(pane.bottom() - used.bottom() < 1.0, "{used:?} in {pane:?}");
    }

    #[test]
    fn picker_option_rows_span_the_pane_not_the_centered_column() {
        for size in [vec2(540.0, 520.0), vec2(580.0, 560.0), vec2(620.0, 640.0)] {
            let LaidOutPicker { pane, used, .. } = assert_square_inset(size);
            assert!(used.right() <= pane.right() + 0.5, "{used:?} in {pane:?}");
        }
    }

    #[test]
    fn picker_fits_option_rows_that_wrap_in_a_narrow_pane() {
        for height in (600..=800).step_by(4) {
            assert_square_inset(vec2(300.0, height as f32));
        }
    }

    #[test]
    fn picker_centers_the_square_in_a_height_limited_pane() {
        let LaidOutPicker { square, pane, .. } = assert_square_inset(vec2(1000.0, 520.0));
        let left = square.left() - pane.left();
        let right = pane.right() - square.right();
        assert!((left - right).abs() < 1.0, "{square:?} in {pane:?}");
    }

    fn painted_text_rect(output: &egui::FullOutput, wanted: &str) -> Rect {
        output
            .shapes
            .iter()
            .find_map(|clipped| match &clipped.shape {
                Shape::Text(text) if text.galley.text() == wanted => {
                    Some(text.galley.rect.translate(text.pos.to_vec2()))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{wanted:?} not painted"))
    }

    #[test]
    fn picker_column_has_space_over_the_inspector_and_option_rows_under_the_square() {
        let picker = lay_out_picker(vec2(600.0, 1000.0));
        let text = |wanted| painted_text_rect(&picker.output, wanted);

        assert!(
            text("Space").bottom() <= text("H").top(),
            "{:?}",
            text("Space")
        );
        assert!(
            text("POINTS").top() >= picker.square.bottom(),
            "{:?} under {:?}",
            text("POINTS"),
            picker.square
        );
        assert!(text("COMB").top() >= text("POINTS").bottom());
        assert!(text("COMB").bottom() <= picker.used.bottom());
    }

    const STRIP_RECT: Rect = Rect {
        min: Pos2::new(100.0, 100.0),
        max: Pos2::new(300.0, 140.0),
    };

    fn two_strip_points() -> Vec<ControlPoint> {
        vec![
            ControlPoint::new_simple(ControlPointValue::new(0.2, 0.3, 0.25), 0.4),
            ControlPoint::new_simple(ControlPointValue::new(0.8, 0.9, 0.75), 0.6),
        ]
    }

    fn primary_click_frames(pos: Pos2) -> Vec<Vec<egui::Event>> {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        vec![
            Vec::new(),
            vec![egui::Event::PointerMoved(pos)],
            vec![button(true)],
            vec![button(false)],
        ]
    }

    fn run_strip_overlay(
        points: &mut [ControlPoint],
        frames: Vec<Vec<egui::Event>>,
        overlay: impl Fn(&mut Ui, &Response, &mut [ControlPoint]) -> Option<usize>,
    ) -> Option<usize> {
        let ctx = egui::Context::default();
        let mut selected = None;
        for events in frames {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let strip = ui.allocate_rect(STRIP_RECT, Sense::click_and_drag());
                    selected = overlay(ui, &strip, points);
                });
            });
        }
        selected
    }

    #[test]
    fn hsv_square_and_hue_strip_colors_are_the_hsva_gamma_colors() {
        let steps: Vec<f32> = (0..=12).map(|i| i as f32 / 12.0).collect();
        let hsva = |h, s, v| Color32::from(ecolor::HsvaGamma { h, s, v, a: 1.0 });
        let strip_color_at = hue_strip_color_at_function(PickerSpace::Hsv);
        for &h in &steps {
            assert_eq!(strip_color_at(h), hsva(h, 1.0, 1.0), "h {h}");
            let square_color_at = main_color_picker_color_at_function(PickerSpace::Hsv, h);
            for &s in &steps {
                for &v in &steps {
                    assert_eq!(square_color_at(s, v), hsva(h, s, v), "h {h} s {s} v {v}");
                }
            }
        }
    }

    fn hue_overlay(
        ui: &mut Ui,
        strip: &Response,
        points: &mut [ControlPoint],
        selected: Option<usize>,
        auto_hue: bool,
    ) -> Option<usize> {
        ui_hue_control_points_overlay(ui, strip, points, selected, auto_hue, PickerSpace::Hsv).1
    }

    #[test]
    fn clicking_hue_gizmo_selects_its_point_without_changing_hue() {
        let mut points = two_strip_points();
        let r = STRIP_RECT.height() / 4.0;
        let first_gizmo_center = pos2(
            STRIP_RECT.left() + 0.25 * STRIP_RECT.width(),
            STRIP_RECT.top() + 3.0 * r + 5.0,
        );
        let selected = run_strip_overlay(
            &mut points,
            primary_click_frames(first_gizmo_center),
            |ui, strip, points| hue_overlay(ui, strip, points, Some(1), false),
        );
        assert_eq!(selected, Some(0));
        assert_eq!(points[0].val().h(), 0.25);
    }

    fn three_t_strip_points() -> Vec<ControlPoint> {
        [(0.0, 0.1), (0.4, 0.5), (1.0, 0.9)]
            .into_iter()
            .map(|(t, sv)| ControlPoint::new_simple(ControlPointValue::new(sv, sv, 0.3), t))
            .collect()
    }

    fn t_strip_pos(t: f32) -> Pos2 {
        pos2(lerp(STRIP_RECT.x_range(), t), STRIP_RECT.center().y)
    }

    fn run_t_overlay(points: &mut [ControlPoint], frames: Vec<Vec<egui::Event>>) -> Option<usize> {
        run_strip_overlay(points, frames, |ui, strip, points| {
            ui_t_control_points_overlay(ui, strip, points, None, PickerSpace::Hsv).1
        })
    }

    #[test]
    fn clicking_t_gizmos_selects_their_point_without_changing_t() {
        for index in 0..3 {
            let mut points = three_t_strip_points();
            let t = *points[index].t();
            let selected = run_t_overlay(&mut points, primary_click_frames(t_strip_pos(t)));
            assert_eq!(selected, Some(index));
            assert_eq!(*points[index].t(), t);
        }
    }

    #[test]
    fn hue_strip_x_fraction_wraps_every_turn_onto_the_strip() {
        assert_eq!(hue_strip_x_fraction(-0.25), 0.75);
        assert_eq!(hue_strip_x_fraction(1.25), 0.25);
        assert_eq!(hue_strip_x_fraction(0.5), 0.5);
        for hue in [-2.5, -1.0, -0.25, 0.0, 0.5, 1.0, 1.25, 3.75] {
            let x = hue_strip_x_fraction(hue);
            assert!((0.0..1.0).contains(&x), "{hue}: {x}");
        }
    }

    #[test]
    fn clicking_wrapped_hue_gizmo_selects_its_point() {
        let mut points = two_strip_points();
        points[0].val_mut()[2] = -0.75;
        let r = STRIP_RECT.height() / 4.0;
        let first_gizmo_center = pos2(
            STRIP_RECT.left() + 0.25 * STRIP_RECT.width(),
            STRIP_RECT.top() + 3.0 * r + 5.0,
        );
        let selected = run_strip_overlay(
            &mut points,
            primary_click_frames(first_gizmo_center),
            |ui, strip, points| hue_overlay(ui, strip, points, Some(1), false),
        );
        assert_eq!(selected, Some(0));
        assert_eq!(points[0].val().h(), -0.75);
    }

    #[test]
    fn hue_gizmo_marks_every_middle_point_under_auto_hue() {
        use HueGizmo::{Editable, Marker};
        let cases = [
            (0, 3, true, Editable),
            (1, 3, true, Marker),
            (2, 3, true, Editable),
            (1, 4, true, Marker),
            (2, 4, true, Marker),
            (0, 3, false, Editable),
            (1, 3, false, Editable),
            (2, 3, false, Editable),
            (0, 2, true, Editable),
            (1, 2, true, Editable),
            (0, 1, true, Editable),
            (0, 1, false, Editable),
        ];
        for (index, count, auto_hue, expected) in cases {
            assert_eq!(
                hue_gizmo(index, count, auto_hue),
                expected,
                "index {index} of {count}, auto hue {auto_hue}"
            );
        }
    }

    fn three_hue_strip_points() -> Vec<ControlPoint> {
        let mut points = two_strip_points();
        points.insert(
            1,
            ControlPoint::new_simple(ControlPointValue::new(0.5, 0.6, 0.5), 0.5),
        );
        points
    }

    #[test]
    fn hue_markers_of_middle_points_are_not_clickable() {
        let r = STRIP_RECT.height() / 4.0;
        let raised_middle_center = pos2(STRIP_RECT.center().x, STRIP_RECT.center().y + r - 14.0);
        let lowered_middle_center = pos2(STRIP_RECT.center().x, STRIP_RECT.center().y + r + 5.0);
        let click = |pos, selected, auto_hue| {
            run_strip_overlay(
                &mut three_hue_strip_points(),
                primary_click_frames(pos),
                move |ui, strip, points| hue_overlay(ui, strip, points, selected, auto_hue),
            )
        };
        assert_eq!(click(raised_middle_center, Some(1), false), Some(1));
        assert_eq!(click(raised_middle_center, Some(1), true), None);
        assert_eq!(click(lowered_middle_center, None, false), Some(1));
        assert_eq!(click(lowered_middle_center, None, true), None);
    }

    fn painted_hue_gizmos(selected: Option<usize>, auto_hue: bool) -> Vec<egui::epaint::PathShape> {
        let ctx = egui::Context::default();
        let mut points = three_hue_strip_points();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let strip = ui.allocate_rect(STRIP_RECT, Sense::hover());
                hue_overlay(ui, &strip, &mut points, selected, auto_hue);
            });
        });
        output
            .shapes
            .into_iter()
            .filter_map(|clipped| match clipped.shape {
                Shape::Path(path) if path.closed => Some(path),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn auto_hue_paints_middle_markers_without_a_selection() {
        let gizmos = painted_hue_gizmos(None, true);
        assert_eq!(gizmos.len(), 3);
        let middle_x = lerp(STRIP_RECT.x_range(), 0.5);
        assert!(
            gizmos.iter().any(|g| {
                let rect = Rect::from_points(&g.points);
                (rect.center().x - middle_x).abs() < 1e-3
                    && rect.width() < STRIP_RECT.height() / 4.0
            }),
            "{gizmos:?}"
        );
    }

    #[test]
    fn selected_hue_marker_is_raised_and_outlined_wider() {
        let marker = |selected| {
            let middle_x = lerp(STRIP_RECT.x_range(), 0.5);
            painted_hue_gizmos(selected, true)
                .into_iter()
                .find(|g| (Rect::from_points(&g.points).center().x - middle_x).abs() < 1e-3)
                .expect("middle marker")
        };
        let (plain, selected) = (marker(None), marker(Some(1)));
        assert!(Rect::from_points(&selected.points).top() < Rect::from_points(&plain.points).top());
        assert!(selected.stroke.width > plain.stroke.width);
    }

    #[test]
    fn hue_after_drag_clamps_the_pointer_to_the_strip_and_keeps_the_turn() {
        let (left, width) = (100.0, 200.0);
        let cases = [
            (0.0, 50.0, 0.0, 0.0),
            (2.4, 50.0, 0.0, 2.0),
            (0.0, 350.0, 0.0, HUE_DRAG_MAX_FRACTION),
            (0.0, 200.0, 0.0, 0.5),
            (1.3, 150.0, 0.0, 1.25),
            (-0.25, 250.0, 0.0, -0.25),
            (0.0, 210.0, 10.0, 0.5),
        ];
        for (stored, pointer_x, grab_offset_x, expected) in cases {
            let hue = hue_after_drag(stored, pointer_x, grab_offset_x, left, width);
            assert!(
                (hue - expected).abs() < 1e-6,
                "{stored} at {pointer_x} - {grab_offset_x}: {hue}"
            );
        }
        for stored in [0.0, 2.4, -0.25] {
            let hue = hue_after_drag(stored, 350.0, 0.0, left, width);
            assert!(hue_strip_x_fraction(hue) > 0.99, "{stored}: {hue}");
        }
    }

    fn primary_drag_frames(start: Pos2, end: Pos2) -> Vec<Vec<egui::Event>> {
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        let mut frames = vec![
            Vec::new(),
            vec![egui::Event::PointerMoved(start)],
            vec![button(start, true)],
        ];
        for step in 1..=3 {
            let pos = start.lerp(end, step as f32 / 3.0);
            frames.push(vec![egui::Event::PointerMoved(pos)]);
        }
        frames.push(vec![button(end, false)]);
        frames
    }

    fn drag_first_hue_gizmo(first_hue: f32, press_x: f32, dx: f32) -> f32 {
        let mut points = two_strip_points();
        points[0].val_mut()[2] = first_hue;
        let r = STRIP_RECT.height() / 4.0;
        let start = pos2(press_x, STRIP_RECT.top() + 3.0 * r + 5.0);
        run_strip_overlay(
            &mut points,
            primary_drag_frames(start, start + vec2(dx, 0.0)),
            |ui, strip, points| hue_overlay(ui, strip, points, Some(1), false),
        );
        points[0].val().h()
    }

    #[test]
    fn dragging_hue_gizmo_left_past_the_strip_start_does_not_wrap() {
        let hue = drag_first_hue_gizmo(0.0, STRIP_RECT.left() + 4.0, -20.0);
        assert_eq!(hue, 0.0);
    }

    #[test]
    fn dragging_hue_gizmo_moves_its_hue_by_the_pointer_distance() {
        let width = STRIP_RECT.width();
        let hue = drag_first_hue_gizmo(0.25, STRIP_RECT.left() + 0.25 * width, 0.25 * width);
        assert!((hue - 0.5).abs() < 1e-4, "{hue}");
    }

    fn drag_t_gizmo(index: usize, grab_dx: f32, dx: f32) -> Vec<ControlPoint> {
        let mut points = three_t_strip_points();
        let start = t_strip_pos(*points[index].t()) + vec2(grab_dx, 0.0);
        run_t_overlay(
            &mut points,
            primary_drag_frames(start, start + vec2(dx, 0.0)),
        );
        points
    }

    #[test]
    fn dragging_a_middle_t_gizmo_moves_its_t_by_the_pointer_distance() {
        let points = drag_t_gizmo(1, 3.0, 0.25 * STRIP_RECT.width());
        assert!((*points[1].t() - 0.65).abs() < 1e-4, "{}", points[1].t());
        assert_eq!((*points[0].t(), *points[2].t()), (0.0, 1.0));
    }

    #[test]
    fn dragging_a_middle_t_gizmo_past_a_neighbour_stops_at_the_min_gap() {
        let right = drag_t_gizmo(1, 0.0, 2.0 * STRIP_RECT.width());
        assert_eq!(*right[1].t(), 1.0 - MIN_T_GAP);
        let left = drag_t_gizmo(1, 0.0, -2.0 * STRIP_RECT.width());
        assert_eq!(*left[1].t(), MIN_T_GAP);
    }

    #[test]
    fn end_t_gizmos_stay_pinned_when_dragged() {
        let first = drag_t_gizmo(0, 2.0, 0.5 * STRIP_RECT.width());
        let last = drag_t_gizmo(2, -2.0, -0.5 * STRIP_RECT.width());
        for points in [first, last] {
            let ts: Vec<f32> = points.iter().map(|cp| *cp.t()).collect();
            assert_eq!(ts, [0.0, 0.4, 1.0]);
        }
    }

    fn run_t_strip(
        points: &mut Vec<ControlPoint>,
        selected: Option<usize>,
        frames: Vec<Vec<egui::Event>>,
    ) -> (Option<usize>, Vec<Shape>) {
        let mut last_modifying_point_index = selected;
        let mut is_modifying_index = selected;
        let mut dragging_index = None;
        let mut dragging_tangent = None;
        let mut control_point_right_clicked = None;
        let mut copy_request = None;
        let egui_ctx = egui::Context::default();
        let mut shapes = Vec::new();
        for events in frames {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            };
            let output = egui_ctx.run(input, |egui_ctx| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    let mut ctx = MainColorPickerCtx {
                        control_points: &mut *points,
                        spline_mode: SplineMode::Linear,
                        picker_space: &mut PickerSpace::Hsv,
                        blend_space: BlendSpace::Picker,
                        color_copy_format: ColorStringCopy::HEXNOA,
                        last_modifying_point_index: &mut last_modifying_point_index,
                        dragging_index: &mut dragging_index,
                        dragging_tangent: &mut dragging_tangent,
                        control_point_right_clicked: &mut control_point_right_clicked,
                        copy_request: &mut copy_request,
                        options: &mut ZColorPickerOptions::default(),
                        auto_comb_unsolved: 0,
                    };
                    let strip = egui::UiBuilder::new().max_rect(STRIP_RECT);
                    ui.scope_builder(strip, |ui| {
                        handle_t_strip(ui, STRIP_RECT.size(), &mut ctx, &mut is_modifying_index)
                    });
                });
            });
            shapes = output
                .shapes
                .into_iter()
                .flat_map(|clipped| match clipped.shape {
                    Shape::Vec(shapes) => shapes,
                    shape => vec![shape],
                })
                .collect();
        }
        (is_modifying_index, shapes)
    }

    #[test]
    fn dragging_the_t_strip_background_changes_no_t_or_selection() {
        let mut points = three_t_strip_points();
        let frames = primary_drag_frames(t_strip_pos(0.15), t_strip_pos(0.3));
        let (selected, _) = run_t_strip(&mut points, Some(2), frames);
        assert_eq!(points, three_t_strip_points());
        assert_eq!(selected, Some(2));
    }

    fn closed_paths(shapes: &[Shape]) -> Vec<(usize, &egui::epaint::PathShape)> {
        shapes
            .iter()
            .enumerate()
            .filter_map(|(i, shape)| match shape {
                Shape::Path(path) if path.closed => Some((i, path)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn t_strip_paints_the_gradient_under_the_gizmos() {
        let mut points = three_t_strip_points();
        let (_, shapes) = run_t_strip(&mut points, None, vec![Vec::new()]);
        let (mesh_index, mesh) = shapes
            .iter()
            .enumerate()
            .find_map(|(i, shape)| match shape {
                Shape::Mesh(mesh) => Some((i, mesh)),
                _ => None,
            })
            .expect("gradient mesh");
        let color = |i: usize| points[i].val().color(PickerSpace::Hsv);
        assert_eq!(mesh.vertices.first().unwrap().color, color(0));
        assert_eq!(mesh.vertices.last().unwrap().color, color(2));
        let gizmos = closed_paths(&shapes);
        assert_eq!(gizmos.len(), 3);
        assert!(gizmos.iter().all(|(i, _)| mesh_index < *i));
        for (gizmo, cp) in gizmos.iter().zip(&points) {
            let x = Rect::from_points(&gizmo.1.points).center().x;
            let expected = lerp(STRIP_RECT.x_range(), *cp.t());
            assert!(
                (x - expected).abs() <= STRIP_RECT.height() / 4.0,
                "{x} vs {expected}"
            );
        }
    }

    #[test]
    fn selected_t_gizmo_is_outlined_wider() {
        let middle_width = |selected| {
            let mut points = three_t_strip_points();
            let (_, shapes) = run_t_strip(&mut points, selected, vec![Vec::new()]);
            closed_paths(&shapes)[1].1.stroke.width
        };
        assert!(middle_width(Some(1)) > middle_width(None));
    }

    #[test]
    fn t_strip_hint_does_not_need_a_selection() {
        assert_eq!(strip_hint(Strip::T, false), strip_hint(Strip::T, true));
        assert_ne!(strip_hint(Strip::T, true), strip_hint(Strip::Hue, true));
        assert_ne!(strip_hint(Strip::Hue, false), strip_hint(Strip::Hue, true));
        for hint in [strip_hint(Strip::T, true), strip_hint(Strip::Hue, false)] {
            assert!(!hint.is_empty());
        }
    }

    #[test]
    fn t_gizmo_hint_shows_t_and_which_points_are_fixed() {
        let first = t_gizmo_hint(0, 3, 0.0);
        let middle = t_gizmo_hint(1, 3, 0.4);
        let last = t_gizmo_hint(2, 3, 1.0);
        assert!(
            first.contains("fixed") && last.contains("fixed"),
            "{first} / {last}"
        );
        assert!(
            !middle.contains("fixed") && middle.contains("Drag"),
            "{middle}"
        );
        assert!(
            middle.contains("0.400") && middle.contains("2 of 3"),
            "{middle}"
        );
    }

    fn paints_empty_picker_hint(points: &mut Vec<ControlPoint>) -> bool {
        let ctx = egui::Context::default();
        let mut picker = ZColorPickerWrapper::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 900.0))),
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                picker.draw_ui(
                    ui,
                    points,
                    SplineMode::Linear,
                    &mut PickerSpace::Hsv,
                    BlendSpace::Picker,
                    &ColorStringCopy::HEXNOA,
                );
            });
        });
        output.shapes.iter().any(|clipped| match &clipped.shape {
            Shape::Text(text) => text.galley.text() == EMPTY_PICKER_HINT,
            _ => false,
        })
    }

    #[test]
    fn empty_picker_hint_is_painted_only_without_points() {
        assert!(paints_empty_picker_hint(&mut Vec::new()));
        assert!(!paints_empty_picker_hint(&mut points_sv(&[(0.5, 0.5)])));
    }

    #[test]
    fn readout_text_without_color_is_placeholder() {
        assert_eq!(
            readout_text(None, ColorStringCopy::HEXNOA),
            NO_SELECTION_READOUT
        );
    }

    #[test]
    fn readout_text_follows_format() {
        assert_eq!(
            readout_text(Some(orange()), ColorStringCopy::HEXNOA),
            "FF8000"
        );
        assert_eq!(
            readout_text(Some(orange()), ColorStringCopy::SRGB),
            "rgb(255, 128, 0)"
        );
        assert_eq!(
            readout_text(Some(orange()), ColorStringCopy::HSV),
            format_orange(ColorStringCopy::HSV)
        );
    }

    fn hover_then_click_frames(pos: Pos2) -> Vec<Vec<egui::Event>> {
        let mut frames = primary_click_frames(pos);
        frames.splice(2..2, [Vec::new(), Vec::new()]);
        frames
    }

    fn click_readout(color: Option<Color32>) -> Option<Pos2> {
        let ctx = egui::Context::default();
        let mut anchor = None;
        for events in hover_then_click_frames(pos2(14.0, 14.0)) {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0))),
                events,
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    anchor = anchor.or(selected_color_readout(ui, color, ColorStringCopy::HEXNOA));
                });
            });
        }
        anchor
    }

    #[test]
    fn clicking_the_readout_requests_a_copy_only_with_a_selection() {
        assert!(click_readout(Some(orange())).is_some());
        assert_eq!(click_readout(None), None);
    }

    fn paints_text(
        points: &mut Vec<ControlPoint>,
        selected: Option<usize>,
        expected: &str,
    ) -> bool {
        let ctx = egui::Context::default();
        let mut picker = ZColorPickerWrapper::default();
        picker.last_modifying_point_index = selected;
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 900.0))),
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                picker.draw_ui(
                    ui,
                    points,
                    SplineMode::Linear,
                    &mut PickerSpace::Hsv,
                    BlendSpace::Picker,
                    &ColorStringCopy::HEXNOA,
                );
            });
        });
        output.shapes.iter().any(|clipped| match &clipped.shape {
            Shape::Text(text) => text.galley.text() == expected,
            _ => false,
        })
    }

    #[test]
    fn readout_placeholder_is_painted_without_selection() {
        let mut points = points_sv(&[(0.5, 0.5)]);
        let selected_text = readout_text(
            Some(points[0].val().color(PickerSpace::Hsv)),
            ColorStringCopy::HEXNOA,
        );
        assert!(paints_text(&mut points, None, NO_SELECTION_READOUT));
        assert!(!paints_text(&mut points, None, &selected_text));
        assert!(paints_text(&mut points, Some(0), &selected_text));
        assert!(!paints_text(&mut points, Some(0), NO_SELECTION_READOUT));
    }

    #[test]
    fn format_color_as_hex() {
        assert_eq!(format_orange(ColorStringCopy::HEX), "CCFF8000");
    }

    #[test]
    fn format_color_as_hexnoa() {
        assert_eq!(format_orange(ColorStringCopy::HEXNOA), "FF8000");
    }

    #[test]
    fn format_color_as_srgbhex() {
        assert_eq!(format_orange(ColorStringCopy::SRGBHEX), "#FF8000CC");
    }

    #[test]
    fn format_color_as_srgb() {
        assert_eq!(format_orange(ColorStringCopy::SRGB), "rgb(255, 128, 0)");
    }

    #[test]
    fn format_color_as_srgba() {
        assert_eq!(
            format_orange(ColorStringCopy::SRGBA),
            "rgba(255, 128, 0, 0.80)"
        );
    }

    #[test]
    fn format_color_as_rgb() {
        assert_eq!(
            format_orange(ColorStringCopy::RGB),
            "rgb(1.000, 0.216, 0.000)"
        );
    }

    #[test]
    fn format_color_as_rgba() {
        assert_eq!(
            format_orange(ColorStringCopy::RGBA),
            "rgba(1.000, 0.216, 0.000, 0.800)"
        );
    }

    #[test]
    fn format_color_as_hsv() {
        assert_eq!(format_orange(ColorStringCopy::HSV), "hsv(30, 100%, 100%)");
    }

    #[test]
    fn format_color_as_hsva() {
        assert_eq!(
            format_orange(ColorStringCopy::HSVA),
            "hsva(30, 100%, 100%, 0.80)"
        );
    }

    #[test]
    fn format_color_as_int() {
        assert_eq!(format_orange(ColorStringCopy::INT), "3439296512");
    }

    #[test]
    fn format_color_as_float() {
        assert_eq!(
            format_orange(ColorStringCopy::FLOAT),
            "1.000, 0.502, 0.000, 0.800"
        );
    }

    #[test]
    fn format_color_as_hsv_gray_has_zero_hue() {
        assert_eq!(
            format_color_as(Color32::from_rgb(128, 128, 128), ColorStringCopy::HSV, None),
            "hsv(0, 0%, 50%)"
        );
    }

    fn format_rgb(rgb: [u8; 3], format: ColorStringCopy) -> String {
        format_color_as(Color32::from_rgb(rgb[0], rgb[1], rgb[2]), format, None)
    }

    #[test]
    fn format_color_as_hsl() {
        assert_eq!(format_orange(ColorStringCopy::HSL), "hsl(30.1 100% 50%)");
        assert_eq!(
            format_rgb([51, 102, 153], ColorStringCopy::HSL),
            "hsl(210 50% 40%)"
        );
        assert_eq!(format_rgb([0; 3], ColorStringCopy::HSL), "hsl(0 0% 0%)");
        assert_eq!(format_rgb([255; 3], ColorStringCopy::HSL), "hsl(0 0% 100%)");
    }

    #[test]
    fn format_color_as_oklab() {
        assert_eq!(
            format_rgb([255, 0, 0], ColorStringCopy::OKLAB),
            "oklab(0.62796 0.22486 0.12585)"
        );
        assert_eq!(format_rgb([0; 3], ColorStringCopy::OKLAB), "oklab(0 0 0)");
        assert_eq!(format_rgb([255; 3], ColorStringCopy::OKLAB), "oklab(1 0 0)");
    }

    #[test]
    fn format_color_as_oklch() {
        assert_eq!(
            format_rgb([255, 0, 0], ColorStringCopy::OKLCH),
            "oklch(0.62796 0.25768 29.23)"
        );
        assert_eq!(format_rgb([0; 3], ColorStringCopy::OKLCH), "oklch(0 0 0)");
        assert_eq!(format_rgb([255; 3], ColorStringCopy::OKLCH), "oklch(1 0 0)");
    }

    fn with_handles(mut points: Vec<ControlPoint>, handles: &[[[f32; 2]; 2]]) -> Vec<ControlPoint> {
        for (cp, pair) in points.iter_mut().zip(handles) {
            *cp.tangents_mut() = pair.map(|[s, v]| Some([s, v, 0.0].into()));
        }
        points
    }

    fn kinked_bezier_points() -> Vec<ControlPoint> {
        let mut points = with_handles(
            points_sv(&[(0.2, 0.3), (0.5, 0.5), (0.8, 0.8)]),
            &[
                [[-0.1, 0.0], [0.1, 0.1]],
                [[-0.1, 0.0], [0.0, 0.1]],
                [[-0.1, -0.05], [0.1, 0.0]],
            ],
        );
        crate::ui_egui::control_points::space_t_evenly(&mut points);
        points
    }

    fn auto_comb_picker() -> ZColorPickerWrapper {
        ZColorPickerWrapper {
            options: ZColorPickerOptions {
                is_auto_comb: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn handle_unit(cp: &ControlPoint, slot: usize) -> Vec2 {
        let offset = cp.tangents()[slot].unwrap();
        vec2(offset[0], offset[1]).normalized()
    }

    #[test]
    fn auto_comb_runs_before_drawing_only_in_bezier_mode() {
        let mut picker = auto_comb_picker();
        let mut points = kinked_bezier_points();
        picker.pre_draw_update(&mut points, SplineMode::Linear, BlendSpace::Picker);
        assert_eq!(points, kinked_bezier_points());

        picker.pre_draw_update(&mut points, SplineMode::Bezier, BlendSpace::Picker);
        let (incoming, outgoing) = (handle_unit(&points[1], 0), handle_unit(&points[1], 1));
        assert!(
            (incoming + outgoing).length() < 1e-4,
            "{incoming:?} {outgoing:?}"
        );
    }

    #[test]
    fn bezier_handles_are_left_alone_with_auto_comb_off() {
        let mut points = kinked_bezier_points();
        ZColorPickerWrapper::default().pre_draw_update(
            &mut points,
            SplineMode::Bezier,
            BlendSpace::Picker,
        );
        assert_eq!(points, kinked_bezier_points());
    }

    #[test]
    fn auto_comb_keeps_the_dragged_handle_direction() {
        let mut picker = auto_comb_picker();
        picker.dragging_tangent = Some((1, 0));
        let mut points = kinked_bezier_points();
        picker.pre_draw_update(&mut points, SplineMode::Bezier, BlendSpace::Picker);
        assert!((handle_unit(&points[1], 0) - vec2(-1.0, 0.0)).length() < 1e-4);
        assert!((handle_unit(&points[1], 1) - vec2(1.0, 0.0)).length() < 1e-4);
    }

    fn comb_teeth_painted(blend_space: BlendSpace) -> usize {
        let ctx = egui::Context::default();
        let mut picker = ZColorPickerWrapper::default();
        let mut points = kinked_bezier_points();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(600.0, 900.0))),
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                picker.draw_ui(
                    ui,
                    &mut points,
                    SplineMode::Bezier,
                    &mut PickerSpace::Hsv,
                    blend_space,
                    &ColorStringCopy::HEXNOA,
                );
            });
        });
        let tooth_color = Color32::from_rgba_unmultiplied(255, 255, 255, 120);
        output
            .shapes
            .into_iter()
            .flat_map(|clipped| match clipped.shape {
                Shape::Vec(shapes) => shapes,
                shape => vec![shape],
            })
            .filter(|shape| {
                matches!(shape, Shape::LineSegment { stroke, .. } if stroke.color == tooth_color)
            })
            .count()
    }

    #[test]
    fn comb_and_auto_comb_are_off_in_a_non_picker_blend() {
        assert!(comb_teeth_painted(BlendSpace::Picker) > 0);
        for blend in BlendSpace::ALL {
            if blend == BlendSpace::Picker {
                continue;
            }
            assert_eq!(comb_teeth_painted(blend), 0, "{blend:?}");

            let mut picker = auto_comb_picker();
            picker.auto_comb_unsolved = 3;
            let mut points = kinked_bezier_points();
            picker.pre_draw_update(&mut points, SplineMode::Bezier, blend);
            assert_eq!(points, kinked_bezier_points(), "{blend:?}");
            assert_eq!(picker.auto_comb_unsolved, 0, "{blend:?}");
        }
    }

    #[test]
    fn auto_comb_counts_joints_it_cannot_make_continuous() {
        let mut picker = auto_comb_picker();
        let mut staircase = with_handles(
            points_sv(&[(0.25, 0.33), (0.44, 0.38), (0.8, 0.6), (0.9, 0.8)]),
            &[[[-0.01, 0.0], [0.01, 0.0]]; 4],
        );
        picker.pre_draw_update(&mut staircase, SplineMode::Bezier, BlendSpace::Picker);
        assert_eq!(picker.auto_comb_unsolved, 2);
        picker.pre_draw_update(&mut staircase, SplineMode::Linear, BlendSpace::Picker);
        assert_eq!(picker.auto_comb_unsolved, 0);
    }

    #[test]
    fn dragging_a_handle_pins_it_for_auto_comb_until_release() {
        let ctx = egui::Context::default();
        let mut picker = auto_comb_picker();
        picker.last_modifying_point_index = Some(1);
        let mut points = with_handles(
            points_sv(&[(0.2, 0.5), (0.5, 0.5), (0.8, 0.5)]),
            &[[[-0.15, 0.0], [0.15, 0.0]]; 3],
        );
        let mut run = |points: &mut Vec<ControlPoint>, events: Vec<egui::Event>| {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 700.0))),
                events,
                ..Default::default()
            };
            let mut square = Rect::NOTHING;
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    square = picker
                        .draw_ui(
                            ui,
                            points,
                            SplineMode::Bezier,
                            &mut PickerSpace::Hsv,
                            BlendSpace::Picker,
                            &ColorStringCopy::HEXNOA,
                        )
                        .rect;
                });
            });
            (square, picker.dragging_tangent)
        };
        let (square, _) = run(&mut points, vec![]);
        let handle = square.lerp_inside(vec2(0.65, 0.5));
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        run(&mut points, vec![egui::Event::PointerMoved(handle)]);
        run(&mut points, vec![button(handle, true)]);
        let mut pinned = None;
        for dy in [3.0, 15.0, 30.0, 45.0] {
            let (_, dragging) = run(
                &mut points,
                vec![egui::Event::PointerMoved(handle - vec2(0.0, dy))],
            );
            pinned = dragging;
        }
        assert_eq!(pinned, Some((1, 1)));
        let up = handle - vec2(0.0, 45.0);
        let (_, released) = run(&mut points, vec![button(up, false)]);
        assert_eq!(released, None);

        let (incoming, outgoing) = (handle_unit(&points[1], 0), handle_unit(&points[1], 1));
        assert!(outgoing.y > 0.2, "{outgoing:?}");
        assert!(
            (incoming + outgoing).length() < 1e-4,
            "{incoming:?} {outgoing:?}"
        );
    }

    fn points_sv(values: &[(f32, f32)]) -> Vec<ControlPoint> {
        values
            .iter()
            .map(|&(s, v)| ControlPoint::new_simple([s, v, 0.0].into(), 0.0))
            .collect()
    }

    #[test]
    fn selection_after_removal_keeps_unremoved_selection() {
        assert_eq!(selection_after_removal(Some(4), 1, 4), Some(3));
        assert_eq!(selection_after_removal(Some(0), 3, 4), Some(0));
    }

    #[test]
    fn selection_after_removal_of_selected_moves_to_left_neighbour() {
        assert_eq!(selection_after_removal(Some(1), 1, 4), Some(0));
        assert_eq!(selection_after_removal(Some(0), 0, 4), Some(0));
        assert_eq!(selection_after_removal(Some(4), 4, 4), Some(3));
    }

    #[test]
    fn selection_after_removal_none_when_empty_or_unselected() {
        assert_eq!(selection_after_removal(Some(0), 0, 0), None);
        assert_eq!(selection_after_removal(None, 2, 4), None);
    }

    #[test]
    fn remove_control_point_keeps_selected_point_and_ends_drag() {
        let mut points = points_sv(&[(0.0, 0.0), (0.1, 0.1), (0.2, 0.2), (0.3, 0.3), (0.4, 0.4)]);
        let mut wrapper = ZColorPickerWrapper {
            last_modifying_point_index: Some(4),
            dragging_index: Some(4),
            ..Default::default()
        };
        wrapper.remove_control_point(&mut points, 1);
        assert_eq!(wrapper.last_modifying_point_index, Some(3));
        assert_eq!(wrapper.dragging_index, None);
    }

    fn points_t(ts: &[f32]) -> Vec<ControlPoint> {
        ts.iter()
            .map(|&t| ControlPoint::new_simple([0.5, 0.5, 0.0].into(), t))
            .collect()
    }

    fn assert_ts_near(points: &[ControlPoint], expected: &[f32]) {
        let ts: Vec<f32> = points.iter().map(|cp| *cp.t()).collect();
        assert_eq!(ts.len(), expected.len(), "{ts:?} vs {expected:?}");
        for (t, e) in ts.iter().zip(expected) {
            assert!((t - e).abs() < 1e-6, "{ts:?} vs {expected:?}");
        }
    }

    #[test]
    fn spawning_right_of_a_middle_point_takes_the_middle_t_of_the_gap() {
        let mut points = points_t(&[0.0, 0.4, 1.0]);
        let mut wrapper = ZColorPickerWrapper {
            last_modifying_point_index: Some(1),
            ..Default::default()
        };
        wrapper.spawn_control_point(&mut points, ControlPoint::default());
        assert_ts_near(&points, &[0.0, 0.4, 0.7, 1.0]);
        assert_eq!(wrapper.last_modifying_point_index, Some(2));
    }

    #[test]
    fn spawning_at_an_end_pins_the_new_point() {
        for (is_insert_right, new_index, expected) in
            [(true, 2, [0.0, 0.5, 1.0]), (false, 0, [0.0, 0.5, 1.0])]
        {
            let mut points = points_t(&[0.0, 1.0]);
            let mut wrapper = ZColorPickerWrapper::default();
            wrapper.options.is_insert_right = is_insert_right;
            let spawned = ControlPoint::new_simple([0.9, 0.9, 0.9].into(), 0.5);
            wrapper.spawn_control_point(&mut points, spawned);
            assert_ts_near(&points, &expected);
            assert_eq!(points[new_index].val()[0], 0.9);
        }
    }

    #[test]
    fn removing_an_end_point_re_pins_t() {
        let mut points = points_t(&[0.0, 0.5, 0.75, 1.0]);
        let mut wrapper = ZColorPickerWrapper::default();
        wrapper.remove_control_point(&mut points, 0);
        assert_ts_near(&points, &[0.0, 0.5, 1.0]);
        wrapper.remove_control_point(&mut points, 2);
        assert_ts_near(&points, &[0.0, 1.0]);
    }

    #[test]
    fn constraints_space_invalid_t_evenly() {
        let mut points = points_t(&[0.0, 0.0, 0.0]);
        ZColorPickerWrapper::default().apply_control_point_constraints(&mut points);
        assert_ts_near(&points, &[0.0, 0.5, 1.0]);
        let mut points = points_t(&[0.0, 1.0, 2.0]);
        ZColorPickerWrapper::default().pre_draw_update(
            &mut points,
            SplineMode::Linear,
            BlendSpace::Picker,
        );
        assert_ts_near(&points, &[0.0, 0.5, 1.0]);
    }

    #[test]
    fn constraints_keep_valid_t() {
        let mut points = points_t(&[0.0, 0.1, 1.0]);
        let mut wrapper = ZColorPickerWrapper::default();
        wrapper.pre_draw_update(&mut points, SplineMode::Linear, BlendSpace::Picker);
        wrapper.apply_control_point_constraints(&mut points);
        assert_ts_near(&points, &[0.0, 0.1, 1.0]);
    }

    #[test]
    fn auto_hue_interpolates_middle_hues_by_t() {
        let mut points: Vec<ControlPoint> = [(0.0, 0.0), (0.25, 0.9), (1.0, 0.4)]
            .into_iter()
            .map(|(t, h)| ControlPoint::new_simple([0.5, 0.5, h].into(), t))
            .collect();
        ZColorPickerWrapper::default().apply_control_point_constraints(&mut points);
        assert!((points[1].val().h() - 0.1).abs() < 1e-6, "{:?}", points[1]);
    }

    #[test]
    fn remove_control_point_down_to_one_keeps_selection() {
        let mut points = points_sv(&[(0.0, 0.0), (0.5, 0.5)]);
        let mut wrapper = ZColorPickerWrapper {
            last_modifying_point_index: Some(1),
            ..Default::default()
        };
        wrapper.remove_control_point(&mut points, 0);
        assert_eq!(wrapper.last_modifying_point_index, Some(0));
    }

    #[test]
    fn window_lock_wraps_hue_and_clamps_saturation_and_value() {
        let mut points: Vec<ControlPoint> = [
            [0.5, 0.5, 1.05],
            [0.5, 0.5, -0.1],
            [0.5, 0.5, 1.0],
            [1.2, -0.2, 0.3],
        ]
        .into_iter()
        .map(|val| ControlPoint::new_simple(val.into(), 0.0))
        .collect();
        let mut wrapper = ZColorPickerWrapper {
            options: ZColorPickerOptions {
                is_window_lock: true,
                is_hue_middle_interpolated: false,
                ..Default::default()
            },
            ..Default::default()
        };
        wrapper.apply_control_point_constraints(&mut points);
        let expected = [
            [0.5, 0.5, 0.05],
            [0.5, 0.5, 0.9],
            [0.5, 0.5, 0.0],
            [1.0, 0.0, 0.3],
        ];
        for (cp, expected) in points.iter().zip(expected) {
            for i in 0..3 {
                assert!(
                    (cp.val()[i] - expected[i]).abs() < 1e-5,
                    "{:?} vs {expected:?}",
                    cp.val()
                );
            }
        }
    }

    #[test]
    fn limit_group_delta_in_range_unchanged() {
        let points = points_sv(&[(0.25, 0.5), (0.9, 0.3)]);
        let delta = vec2(0.05, -0.1);
        assert_eq!(limit_group_delta(&points, delta), delta);
    }

    #[test]
    fn limit_group_delta_stops_exactly_on_border() {
        let mut points = points_sv(&[(0.25, 0.5), (0.9, 0.3)]);
        let delta = limit_group_delta(&points, vec2(0.2, -0.7));
        apply_group_delta_sv(&mut points, delta);
        assert_eq!(points[1].val()[0], 1.0);
        assert_eq!(points[1].val()[1], 0.0);
    }

    #[test]
    fn limit_group_delta_push_past_border_and_back_keeps_offsets() {
        let mut points = points_sv(&[(0.25, 0.5), (0.9, 0.3)]);
        let mut wrapper = ZColorPickerWrapper::default();
        for push in [vec2(0.2, 0.0), vec2(-0.2, 0.0)] {
            let delta = limit_group_delta(&points, push);
            apply_group_delta_sv(&mut points, delta);
            wrapper.clamp_control_points(&mut points);
        }
        let ds = points[1].val()[0] - points[0].val()[0];
        let dv = points[1].val()[1] - points[0].val()[1];
        assert!((ds - 0.65).abs() < 1e-6, "ds {ds}");
        assert!((dv - -0.2).abs() < 1e-6, "dv {dv}");
    }

    #[test]
    fn limit_group_delta_empty_unchanged() {
        let delta = vec2(0.3, -2.0);
        assert_eq!(limit_group_delta(&[], delta), delta);
    }

    #[test]
    fn limit_group_delta_wider_than_range_does_not_move() {
        let points = points_sv(&[(-0.1, 0.5), (1.1, 0.5)]);
        assert_eq!(limit_group_delta(&points, vec2(0.2, 0.0)).x, 0.0);
    }

    const DRAG_RECT: Rect = Rect {
        min: pos2(10.0, 20.0),
        max: pos2(210.0, 120.0),
    };

    #[test]
    fn drag_target_sv_centre_without_offset_is_half() {
        assert_eq!(
            drag_target_sv(DRAG_RECT.center(), Vec2::ZERO, DRAG_RECT),
            vec2(0.5, 0.5)
        );
    }

    #[test]
    fn drag_target_sv_top_left_is_zero_saturation_full_value() {
        assert_eq!(
            drag_target_sv(DRAG_RECT.min, Vec2::ZERO, DRAG_RECT),
            vec2(0.0, 1.0)
        );
    }

    #[test]
    fn drag_target_sv_subtracts_grab_offset() {
        let offset = vec2(20.0, -10.0);
        assert_eq!(
            drag_target_sv(DRAG_RECT.center() + offset, offset, DRAG_RECT),
            vec2(0.5, 0.5)
        );
    }

    #[test]
    fn drag_target_sv_outside_rect_is_unclamped() {
        let pointer = DRAG_RECT.right_bottom() + vec2(50.0, 50.0);
        assert_eq!(
            drag_target_sv(pointer, Vec2::ZERO, DRAG_RECT),
            vec2(1.25, -0.5)
        );
    }

    fn run_picker_frame(
        ctx: &egui::Context,
        picker: &mut ZColorPickerWrapper,
        points: &mut Vec<ControlPoint>,
        event: Option<egui::Event>,
    ) -> Rect {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 700.0))),
            events: event.into_iter().collect(),
            ..Default::default()
        };
        let mut square = Rect::NOTHING;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                square = picker
                    .draw_ui(
                        ui,
                        points,
                        SplineMode::Linear,
                        &mut PickerSpace::Hsv,
                        BlendSpace::Picker,
                        &ColorStringCopy::HEXNOA,
                    )
                    .rect;
            });
        });
        square
    }

    struct OutAndBackDrag {
        at_far: Vec<ControlPoint>,
        at_back: Vec<ControlPoint>,
        back_s: f32,
        tolerance: f32,
    }

    fn drag_first_point_right_out_and_back(
        options: ZColorPickerOptions,
        mut points: Vec<ControlPoint>,
    ) -> OutAndBackDrag {
        let ctx = egui::Context::default();
        let mut picker = ZColorPickerWrapper {
            options,
            ..Default::default()
        };
        let square = run_picker_frame(&ctx, &mut picker, &mut points, None);
        let start = square.lerp_inside(control_point_pos(&points[0]).to_vec2());
        let below_drag_threshold = start + vec2(3.0, 0.0);
        let far = pos2(square.right() + 60.0, start.y);
        let back = pos2(square.right() - 30.0, start.y);
        let press = egui::Event::PointerButton {
            pos: start,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::default(),
        };
        let mut run = |points: &mut Vec<ControlPoint>, event| {
            run_picker_frame(&ctx, &mut picker, points, Some(event));
        };
        run(&mut points, egui::Event::PointerMoved(start));
        run(&mut points, press);
        run(&mut points, egui::Event::PointerMoved(below_drag_threshold));
        for step in 1..=4 {
            let pos = below_drag_threshold.lerp(far, step as f32 / 4.0);
            run(&mut points, egui::Event::PointerMoved(pos));
        }
        let at_far = points.clone();
        for step in 1..=3 {
            let pos = far.lerp(back, step as f32 / 3.0);
            run(&mut points, egui::Event::PointerMoved(pos));
        }
        OutAndBackDrag {
            at_far,
            at_back: points,
            back_s: (back.x - square.left()) / square.width(),
            tolerance: 0.5 / square.width(),
        }
    }

    fn assert_s_near(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "s {actual}, expected {expected} +- {tolerance}"
        );
    }

    #[test]
    fn drag_past_border_and_back_keeps_point_under_pointer_with_clamp() {
        let drag = drag_first_point_right_out_and_back(
            ZColorPickerOptions::default(),
            points_sv(&[(0.5, 0.5)]),
        );
        assert_s_near(drag.at_back[0].val()[0], drag.back_s, drag.tolerance);
    }

    #[test]
    fn drag_past_border_and_back_keeps_point_under_pointer_without_clamp() {
        let options = ZColorPickerOptions {
            is_window_lock: false,
            ..Default::default()
        };
        let drag = drag_first_point_right_out_and_back(options, points_sv(&[(0.5, 0.5)]));
        assert_s_near(drag.at_back[0].val()[0], drag.back_s, drag.tolerance);
    }

    #[test]
    fn locked_clamped_group_drag_stops_at_border_and_follows_pointer_back() {
        let options = ZColorPickerOptions {
            is_curve_locked: true,
            ..Default::default()
        };
        let drag =
            drag_first_point_right_out_and_back(options, points_sv(&[(0.5, 0.5), (0.25, 0.25)]));
        assert_s_near(drag.at_far[0].val()[0], 1.0, drag.tolerance);
        assert_s_near(drag.at_far[1].val()[0], 0.75, drag.tolerance);
        assert_s_near(drag.at_back[0].val()[0], drag.back_s, drag.tolerance);
        assert_s_near(drag.at_back[1].val()[0], drag.back_s - 0.25, drag.tolerance);
    }

    fn spawn_at(points: &[ControlPoint], x: f32, y: f32) -> Option<ControlPoint> {
        point_to_spawn(
            points,
            pos2(x, y),
            vec2(200.0, 200.0),
            CONTROL_POINT_FILL_RADIUS,
        )
    }

    fn point(s: f32, v: f32, h: f32, t: f32) -> ControlPoint {
        ControlPoint::new_simple([s, v, h].into(), t)
    }

    #[test]
    fn point_to_spawn_empty_list_uses_click_with_zero_hue_and_t() {
        assert_eq!(spawn_at(&[], 0.25, 0.75), Some(point(0.25, 0.25, 0.0, 0.0)));
    }

    #[test]
    fn point_to_spawn_on_point_disc_spawns_nothing() {
        let points = [point(0.5, 0.5, 0.3, 0.7)];
        assert_eq!(spawn_at(&points, 0.565, 0.5), None);
        assert_eq!(spawn_at(&points, 0.5, 0.45), None);
    }

    #[test]
    fn point_to_spawn_just_outside_disc_inherits_hue() {
        let points = [point(0.5, 0.5, 0.3, 0.7)];
        assert_eq!(
            spawn_at(&points, 0.58, 0.5),
            Some(point(0.58, 0.5, 0.3, 0.0))
        );
        assert_eq!(spawn_at(&points, 0.6, 0.5), Some(point(0.6, 0.5, 0.3, 0.0)));
    }

    #[test]
    fn point_to_spawn_inherits_hue_from_nearest_point() {
        let points = [point(0.25, 0.5, 0.1, 0.2), point(0.75, 0.5, 0.9, 0.8)];
        assert_eq!(
            spawn_at(&points, 0.625, 0.5),
            Some(point(0.625, 0.5, 0.9, 0.0))
        );
    }

    #[test]
    fn point_to_spawn_clamps_click_outside_unit_square() {
        assert_eq!(spawn_at(&[], 1.5, -0.25), Some(point(1.0, 1.0, 0.0, 0.0)));
    }

    #[test]
    fn point_to_spawn_top_left_is_zero_saturation_full_value() {
        assert_eq!(spawn_at(&[], 0.0, 0.0), Some(point(0.0, 1.0, 0.0, 0.0)));
    }
}
