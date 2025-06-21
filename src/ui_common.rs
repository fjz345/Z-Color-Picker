use crate::color_picker::format_color_as;
use crate::common::ColorStringCopy;
use crate::control_point::ControlPoint;
use crate::egui::PointerButton;
use crate::egui::TextStyle;
use crate::image_processing::flip_v;
use crate::image_processing::u8_to_u8u8u8;
use crate::image_processing::Rgb;
use eframe::egui::InnerResponse;
use eframe::egui::Pos2;
use eframe::egui::Window;
use eframe::glow;
use eframe::glow::HasContext;
use eframe::{
    egui::{color_picker::Alpha, Painter, Response, Sense, Ui, WidgetInfo, WidgetType},
    emath::{lerp, remap_clamp},
    epaint::{pos2, Color32, Mesh, Rect, Rgba, Shape, Stroke, Vec2},
};
use std::borrow::BorrowMut;
use std::sync::Arc;

pub fn contrast_color(color: impl Into<Rgba>) -> Color32 {
    if color.into().intensity() < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

pub fn background_checkers(painter: &Painter, rect: Rect) {
    let rect = rect.shrink(0.5); // Small hack to avoid the checkers from peeking through the sides
    if !rect.is_positive() {
        return;
    }

    let dark_color = Color32::from_gray(32);
    let bright_color = Color32::from_gray(128);

    let checker_size = Vec2::splat(rect.height() / 2.0);
    let n = (rect.width() / checker_size.x).round() as u32;

    let mut mesh = Mesh::default();
    mesh.add_colored_rect(rect, dark_color);

    let mut top = true;
    for i in 0..n {
        let x = lerp(rect.left()..=rect.right(), i as f32 / (n as f32));
        let small_rect = if top {
            Rect::from_min_size(pos2(x, rect.top()), checker_size)
        } else {
            Rect::from_min_size(pos2(x, rect.center().y), checker_size)
        };
        mesh.add_colored_rect(small_rect, bright_color);
        top = !top;
    }
    painter.add(Shape::mesh(mesh));
}

pub fn color_button(ui: &mut Ui, size: Vec2, color: Color32, _open: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    response.widget_info(|| WidgetInfo::new(WidgetType::ColorButton));

    if ui.is_rect_visible(rect) {
        let visuals = &ui.visuals().widgets.open;
        let rect = rect.expand(visuals.expansion);

        show_color_at(ui.painter(), color, rect);
    }

    response
}

/// Show a color with background checkers to demonstrate transparency (if any).
pub fn show_color_at(painter: &Painter, color: Color32, rect: Rect) {
    if color.is_opaque() {
        painter.rect_filled(rect, 0.0, color);
    } else {
        // Transparent: how both the transparent and opaque versions of the color
        background_checkers(painter, rect);

        if color == Color32::TRANSPARENT {
            // There is no opaque version, so just show the background checkers
        } else {
            let left = Rect::from_min_max(rect.left_top(), rect.center_bottom());
            let right = Rect::from_min_max(rect.center_top(), rect.right_bottom());
            painter.rect_filled(left, 0.0, color);
            painter.rect_filled(right, 0.0, color.to_opaque());
        }
    }
}

pub fn color_slider_1d(
    ui: &mut Ui,
    desired_size: Vec2,
    val: Option<&mut f32>,
    color_at: impl Fn(f32) -> Color32,
) -> Response {
    #![allow(clippy::identity_op)]

    let (rect, mut response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());
    let visuals = ui.style().interact(&response);

    if let Some(mpos) = response.interact_pointer_pos() {
        if let Some(val_mut) = val {
            *val_mut = remap_clamp(
                mpos.x,
                response.rect.left()..=response.rect.right(),
                0.0..=1.0,
            );
        }

        response.mark_changed();
    }

    if ui.is_rect_visible(rect) {
        // fill color:
        let mut mesh = Mesh::default();
        for i in 0..=N {
            let t = i as f32 / (N as f32);
            let color = color_at(t);
            let x = lerp(rect.left()..=rect.right(), t);
            mesh.colored_vertex(pos2(x, rect.top()), color);
            mesh.colored_vertex(pos2(x, rect.bottom()), color);
            if i < N {
                mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
            }
        }
        ui.painter().add(Shape::mesh(mesh));

        ui.painter().rect_stroke(
            rect,
            0.0,
            visuals.bg_stroke,
            eframe::egui::StrokeKind::Middle,
        ); // outline
    }

    response
}

pub fn ui_hue_control_points_overlay(
    ui: &mut Ui,
    parent_response: &Response,
    control_points: &mut [ControlPoint],
    modifying_control_point_index: Option<usize>,
    is_hue_middle_interpolated: bool,
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
        if is_hue_middle_interpolated {
            if i != 0 && i != control_points.len() - 1 {
                continue;
            }
        }

        let val = control_points[i].val().h();
        let picked_color = control_points[i].val().color();
        // Show where the slider is at:
        let x = lerp(
            container_response.rect.left()..=container_response.rect.right(),
            val,
        );

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

        let response = ui.interact(
            Rect::from_points(&gizmo_rect),
            container_response.id.with(i),
            Sense::click_and_drag(),
        );

        if response.dragged_by(PointerButton::Primary) {
            selected_key_frame = Some(i);
            control_points[i].val_mut()[2] +=
                response.drag_delta().x / container_response.rect.width();
        }

        ui.painter().add(Shape::convex_polygon(
            gizmo_rect,
            picked_color,
            Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        ));
    }

    (container_response, selected_key_frame)
}

/// Number of vertices per dimension in the color sliders.
/// We need at least 6 for hues, and more for smooth 2D areas.
/// Should always be a multiple of 6 to hit the peak hues in HSV/HSL (every 60°).
const N: u32 = 6 * 6;
/// # Arguments
/// * `x_value` - X axis, either saturation or value (0.0-1.0).
/// * `y_value` - Y axis, either saturation or value (0.0-1.0).
/// * `color_at` - A function that dictates how the mix of saturation and value will be displayed in the 2d slider.
/// E.g.: `|x_value, y_value| HsvaGamma { h: 1.0, s: x_value, v: y_value, a: 1.0 }.into()` displays the colors as follows: top-left: white \[s: 0.0, v: 1.0], top-right: fully saturated color \[s: 1.0, v: 1.0], bottom-right: black \[s: 0.0, v: 1.0].
///
pub fn color_slider_2d(
    ui: &mut Ui,
    desiered_size: Vec2,
    x_value: &mut f32,
    y_value: &mut f32,
    color_at: impl Fn(f32, f32) -> Color32,
) -> Response {
    let (rect, response) = ui.allocate_at_least(desiered_size, Sense::click());

    if let Some(mpos) = response.interact_pointer_pos() {
        *x_value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
        *y_value = remap_clamp(mpos.y, rect.bottom()..=rect.top(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        let mut mesh = Mesh::default();

        for xi in 0..=N {
            for yi in 0..=N {
                let xt = xi as f32 / (N as f32);
                let yt: f32 = yi as f32 / (N as f32);
                let color = color_at(xt, yt);
                let x = lerp(rect.left()..=rect.right(), xt);
                let y = lerp(rect.bottom()..=rect.top(), yt);
                mesh.colored_vertex(pos2(x, y), color);

                if xi < N && yi < N {
                    let x_offset = 1;
                    let y_offset = N + 1;
                    let tl = yi * y_offset + xi;
                    mesh.add_triangle(tl, tl + x_offset, tl + y_offset);
                    mesh.add_triangle(tl + x_offset, tl + y_offset, tl + y_offset + x_offset);
                }
            }
        }
        ui.painter().add(Shape::mesh(mesh)); // fill

        ui.painter().rect_stroke(
            rect,
            0.0,
            visuals.bg_stroke,
            eframe::egui::StrokeKind::Middle,
        ); // outline

        // // Show where the slider is at:
        // let x = lerp(rect.left()..=rect.right(), *x_value);
        // let y = lerp(rect.bottom()..=rect.top(), *y_value);
        // let picked_color = color_at(*x_value, *y_value);
        // ui.painter().add(epaint::CircleShape {
        //     center: pos2(x, y),
        //     radius: rect.width() / 12.0,
        //     fill: picked_color,
        //     stroke: Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        // });
    }

    response
}

pub fn color_button_copy(
    ui: &mut Ui,
    color: impl Into<Color32>,
    _alpha: Alpha,
    color_copy_format: ColorStringCopy,
) {
    let button_response = ui.button("📋").on_hover_text("Copy (middle mouse click)");
    if button_response.clicked() {
        ui.output_mut(|w| {
            w.copied_text = format_color_as(color.into(), color_copy_format, None);
        });
    }
}

pub fn response_copy_color_on_click(
    ui: &mut Ui,
    response: &Response,
    color: impl Into<Color32>,
    color_copy_format: ColorStringCopy,
    button_click_type: PointerButton,
) {
    if response.clicked_by(button_click_type) {
        ui.output_mut(|w| {
            w.copied_text = format_color_as(color.into(), color_copy_format, None);
        });
    }
}

pub fn color_text_ui(
    ui: &mut Ui,
    color: impl Into<Color32>,
    alpha: Alpha,
    color_copy_format: ColorStringCopy,
) -> InnerResponse<()> {
    let color = color.into();
    let [r, g, b, a] = color.to_array();

    ui.horizontal(|ui| {
        color_button_copy(ui, color, alpha, color_copy_format);

        let old_style = Arc::as_ref(ui.style()).clone();

        ui.style_mut()
            .text_styles
            .get_mut(&TextStyle::Body)
            .unwrap()
            .size = 8.0;

        if alpha == Alpha::Opaque {
            ui.label(format!("rgb({}, {}, {})", r, g, b))
                .on_hover_text("Red Green Blue");
        } else {
            ui.label(format!("rgba({}, {}, {}, {})", r, g, b, a))
                .on_hover_text("Red Green Blue with premultiplied Alpha");
        }

        *ui.style_mut() = old_style;
    })
}

pub trait ContentWindow {
    fn is_open(&self) -> bool;
    fn close(&mut self);
    fn open(&mut self);

    fn title(&self) -> &str {
        "Content Window"
    }

    fn draw_content(&mut self, _ui: &mut Ui) {}

    fn draw_ui(&mut self, ui: &mut Ui) -> Option<InnerResponse<Option<()>>> {
        let prev_visuals = ui.visuals_mut().clone();

        let mut open = self.is_open();
        let response = Window::new(self.title())
            .resizable(true)
            .title_bar(true)
            .open(&mut open)
            .auto_sized()
            .show(ui.ctx(), |ui| {
                self.draw_content(ui);
            });

        if open {
            self.open();
        } else {
            self.close();
        }

        ui.ctx().set_visuals(prev_visuals);

        response
    }
}
