use std::{
    borrow::{Borrow, BorrowMut},
    sync::Arc,
};

use eframe::{
    egui::{
        color_picker::{show_color, Alpha},
        Label, Painter, Response, Sense, TextStyle, Ui, Widget,
    },
    emath::{lerp, remap_clamp},
    epaint::{self, pos2, vec2, Color32, HsvaGamma, Mesh, Pos2, Rect, Rgba, Shape, Stroke, Vec2},
};

use crate::bezier::PaintBezier;

pub struct MainColorPickerData {
    pub hsva: HsvaGamma,
    pub alpha: Alpha,
    pub paint_bezier: PaintBezier,
}

pub fn main_color_picker(ui: &mut Ui, data: &mut MainColorPickerData) {
    let current_color_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    show_color(ui, data.hsva, current_color_size).on_hover_text("Selected color");

    color_text_ui(ui, data.hsva, data.alpha);

    if data.alpha == Alpha::BlendOrAdditive {
        // We signal additive blending by storing a negative alpha (a bit ironic).
        let a = &mut data.hsva.a;
        let mut additive = *a < 0.0;
        ui.horizontal(|ui| {
            ui.label("Blending:");
            ui.radio_value(&mut additive, false, "Normal");
            ui.radio_value(&mut additive, true, "Additive");

            if additive {
                *a = -a.abs();
            }

            if !additive {
                *a = a.abs();
            }
        });
    }
    let additive = data.hsva.a < 0.0;

    let opaque = HsvaGamma {
        a: 1.0,
        ..data.hsva
    };

    if data.alpha == Alpha::Opaque {
        data.hsva.a = 1.0;
    } else {
        let a = &mut data.hsva.a;

        if data.alpha == Alpha::OnlyBlend {
            if *a < 0.0 {
                *a = 0.5; // was additive, but isn't allowed to be
            }
            color_slider_1d(ui, a, |a| HsvaGamma { a, ..opaque }.into()).on_hover_text("Alpha");
        } else if !additive {
            color_slider_1d(ui, a, |a| HsvaGamma { a, ..opaque }.into()).on_hover_text("Alpha");
        }
    }

    let HsvaGamma { h, s, v, a: _ } = &mut data.hsva;

    color_slider_1d(ui, h, |h| {
        HsvaGamma {
            h,
            s: 1.0,
            v: 1.0,
            a: 1.0,
        }
        .into()
    })
    .on_hover_text("Hue");

    if false {
        color_slider_1d(ui, s, |s| HsvaGamma { s, ..opaque }.into()).on_hover_text("Saturation");
    }

    if false {
        color_slider_1d(ui, v, |v| HsvaGamma { v, ..opaque }.into()).on_hover_text("Value");
    }

    let slider_2d_reponse = color_slider_2d(ui, s, v, |s, v| HsvaGamma { s, v, ..opaque }.into());

    data.paint_bezier
        .ui_content_with_painter(ui, &slider_2d_reponse, &ui.painter());
}

fn color_slider_1d(ui: &mut Ui, value: &mut f32, color_at: impl Fn(f32) -> Color32) -> Response {
    #![allow(clippy::identity_op)]

    let desired_size = vec2(ui.spacing().slider_width, ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

    if let Some(mpos) = response.interact_pointer_pos() {
        *value = remap_clamp(mpos.x, rect.left()..=rect.right(), 0.0..=1.0);
    }

    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);

        // background_checkers(ui.painter(), rect); // for alpha:

        {
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
        }

        ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

        {
            // Show where the slider is at:
            let x = lerp(rect.left()..=rect.right(), *value);
            let r = rect.height() / 4.0;
            let picked_color = color_at(*value);
            ui.painter().add(Shape::convex_polygon(
                vec![
                    pos2(x, rect.center().y),   // tip
                    pos2(x + r, rect.bottom()), // right bottom
                    pos2(x - r, rect.bottom()), // left bottom
                ],
                picked_color,
                Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
            ));
        }
    }

    response
}

/// # Arguments
/// * `x_value` - X axis, either saturation or value (0.0-1.0).
/// * `y_value` - Y axis, either saturation or value (0.0-1.0).
/// * `color_at` - A function that dictates how the mix of saturation and value will be displayed in the 2d slider.
/// E.g.: `|x_value, y_value| HsvaGamma { h: 1.0, s: x_value, v: y_value, a: 1.0 }.into()` displays the colors as follows: top-left: white \[s: 0.0, v: 1.0], top-right: fully saturated color \[s: 1.0, v: 1.0], bottom-right: black \[s: 0.0, v: 1.0].
///
fn color_slider_2d(
    ui: &mut Ui,
    x_value: &mut f32,
    y_value: &mut f32,
    color_at: impl Fn(f32, f32) -> Color32,
) -> Response {
    let desired_size = Vec2::splat(ui.spacing().slider_width);
    let (rect, response) = ui.allocate_at_least(desired_size, Sense::click_and_drag());

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
                let yt = yi as f32 / (N as f32);
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

        ui.painter().rect_stroke(rect, 0.0, visuals.bg_stroke); // outline

        // Show where the slider is at:
        let x = lerp(rect.left()..=rect.right(), *x_value);
        let y = lerp(rect.bottom()..=rect.top(), *y_value);
        let picked_color = color_at(*x_value, *y_value);
        ui.painter().add(epaint::CircleShape {
            center: pos2(x, y),
            radius: rect.width() / 12.0,
            fill: picked_color,
            stroke: Stroke::new(visuals.fg_stroke.width, contrast_color(picked_color)),
        });
    }

    response
}

fn contrast_color(color: impl Into<Rgba>) -> Color32 {
    if color.into().intensity() < 0.5 {
        Color32::WHITE
    } else {
        Color32::BLACK
    }
}

/// Number of vertices per dimension in the color sliders.
/// We need at least 6 for hues, and more for smooth 2D areas.
/// Should always be a multiple of 6 to hit the peak hues in HSV/HSL (every 60°).
const N: u32 = 6 * 6;

fn background_checkers(painter: &Painter, rect: Rect) {
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

fn color_text_ui(ui: &mut Ui, color: impl Into<Color32>, alpha: Alpha) {
    let color = color.into();
    ui.horizontal(|ui| {
        let [r, g, b, a] = color.to_array();

        if ui.button("📋").on_hover_text("Click to copy").clicked() {
            if alpha == Alpha::Opaque {
                ui.output_mut(|o| o.copied_text = format!("{}, {}, {}", r, g, b));
            } else {
                ui.output_mut(|o| o.copied_text = format!("{}, {}, {}, {}", r, g, b, a));
            }
        }

        // if alpha == Alpha::Opaque {
        //     ui.put(
        //         Rect {
        //             min: Pos2 { x: 0.0, y: 0.0 },
        //             max: Pos2 {
        //                 x: ui.available_size().x,
        //                 y: ui.available_size().y,
        //             },
        //         },
        //         Label::new(format!("rgb({}, {}, {})", r, g, b)),
        //     )
        //     .on_hover_text("Red Green Blue");
        // } else {
        //     ui.put(
        //         ui.available_rect_before_wrap(),
        //         Label::new(format!("rgba({}, {}, {}, {})", r, g, b, a)),
        //     )
        //     .on_hover_text("Red Green Blue with premultiplied Alpha");
        // }

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
    });
}
