use eframe::egui::{
    self,
    style::{Selection, WidgetVisuals},
    vec2, Color32, CornerRadius, Frame, Margin, Rect, Response, RichText, Stroke, Style, Ui,
    UiBuilder, Vec2, Visuals,
};

const ACCENT: Color32 = Color32::from_rgb(86, 156, 245);
const ACCENT_FILL: Color32 = Color32::from_rgb(36, 74, 122);
const ACCENT_TEXT: Color32 = Color32::from_rgb(214, 230, 255);

const CANVAS_FILL: Color32 = Color32::from_gray(15);
pub const PANE_FILL: Color32 = Color32::from_gray(27);
pub const TOOLBAR_FILL: Color32 = Color32::from_gray(36);
pub const DRAWER_FILL: Color32 = Color32::from_gray(21);

const LABEL_TEXT: Color32 = Color32::from_gray(168);
const VALUE_TEXT: Color32 = Color32::from_gray(226);
const SECTION_TEXT: Color32 = Color32::from_gray(140);
const SECTION_SIZE: f32 = 10.5;
const SECTION_LETTER_SPACING: f32 = 0.8;
const ROW_HEADER_WIDTH: f32 = 56.0;
const GROUP_GAP: f32 = 20.0;

const WIDGET_RADIUS: u8 = 3;
const PANE_RADIUS: u8 = 6;
pub const PANE_GAP: f32 = 8.0;
const PANE_PADDING: f32 = 8.0;

fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        ..Style::default()
    };
    let spacing = &mut style.spacing;
    spacing.item_spacing = vec2(8.0, 4.0);
    spacing.button_padding = vec2(8.0, 2.0);
    spacing.interact_size.y = 20.0;
    spacing.icon_spacing = 6.0;
    spacing.window_margin = Margin::same(10);
    spacing.menu_margin = Margin::same(6);
    style
}

fn widget(fill: Color32, bg_stroke: Stroke, fg_stroke: Stroke, expansion: f32) -> WidgetVisuals {
    WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: fill,
        bg_stroke,
        fg_stroke,
        corner_radius: CornerRadius::same(WIDGET_RADIUS),
        expansion,
    }
}

fn visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    let widgets = &mut visuals.widgets;
    widgets.noninteractive = widget(
        PANE_FILL,
        Stroke::new(1.0, Color32::from_gray(58)),
        Stroke::new(1.0, LABEL_TEXT),
        0.0,
    );
    widgets.inactive = widget(
        Color32::from_gray(52),
        Stroke::NONE,
        Stroke::new(1.0, VALUE_TEXT),
        0.0,
    );
    widgets.hovered = widget(
        Color32::from_gray(64),
        Stroke::new(1.0, ACCENT),
        Stroke::new(1.5, Color32::from_gray(245)),
        1.0,
    );
    widgets.active = widget(
        ACCENT_FILL,
        Stroke::new(1.0, ACCENT),
        Stroke::new(2.0, Color32::WHITE),
        1.0,
    );
    widgets.open = widget(
        Color32::from_gray(46),
        Stroke::new(1.0, ACCENT),
        Stroke::new(1.0, VALUE_TEXT),
        0.0,
    );
    visuals.selection = Selection {
        bg_fill: ACCENT_FILL,
        stroke: Stroke::new(1.0, ACCENT_TEXT),
    };
    visuals.hyperlink_color = ACCENT;
    visuals.slider_trailing_fill = true;
    visuals.panel_fill = CANVAS_FILL;
    visuals.window_fill = Color32::from_gray(32);
    visuals.window_stroke = Stroke::new(1.0, Color32::from_gray(66));
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(6);
    visuals.extreme_bg_color = Color32::from_gray(12);
    visuals.faint_bg_color = Color32::from_gray(32);
    visuals.warn_fg_color = Color32::from_rgb(240, 170, 60);
    visuals.error_fg_color = Color32::from_rgb(240, 90, 90);
    visuals
}

pub fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| *style = self::style());
}

pub fn toolbar_frame() -> Frame {
    Frame::new()
        .fill(TOOLBAR_FILL)
        .inner_margin(Margin::symmetric(10, 6))
}

pub fn drawer_frame() -> Frame {
    Frame::new()
        .fill(DRAWER_FILL)
        .inner_margin(Margin::symmetric(10, 4))
}

pub fn pane_card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let card = ui.max_rect();
    ui.painter().rect_filled(card, PANE_RADIUS, PANE_FILL);
    let content_size = (card.size() - Vec2::splat(2.0 * PANE_PADDING)).max(Vec2::ZERO);
    let content = Rect::from_center_size(card.center(), content_size);
    ui.scope_builder(UiBuilder::new().max_rect(content), add_contents)
        .inner
}

pub fn pane_card_size(content_size: Vec2) -> Vec2 {
    content_size + Vec2::splat(2.0 * PANE_PADDING)
}

pub fn section_header(ui: &mut Ui, title: &str) -> Response {
    ui.label(
        RichText::new(title.to_uppercase())
            .size(SECTION_SIZE)
            .color(SECTION_TEXT)
            .extra_letter_spacing(SECTION_LETTER_SPACING),
    )
}

pub fn row_header(ui: &mut Ui, title: &str) -> Response {
    let response = section_header(ui, title);
    let item_gap = ui.spacing().item_spacing.x;
    ui.add_space((ROW_HEADER_WIDTH - response.rect.width() - item_gap).max(0.0));
    response
}

pub fn unbroken_group(ui: &mut Ui, id_salt: &str, add_contents: impl FnOnce(&mut Ui)) {
    // A wrapping row only moves an item to the next line when it knows the item's width up front.
    let id = ui.id().with(id_salt);
    let last_width = ui.data(|data| data.get_temp(id)).unwrap_or(0.0);
    let size = vec2(last_width, ui.spacing().interact_size.y);
    let layout = egui::Layout::left_to_right(egui::Align::Center);
    let response = ui
        .allocate_ui_with_layout(size, layout, add_contents)
        .response;
    let width = response.rect.width();
    if (width - last_width).abs() > 0.5 {
        ui.data_mut(|data| data.insert_temp(id, width));
        ui.ctx().request_discard("a row group changed width");
    }
}

const OUT_OF_EFFECT_OPACITY: f32 = 0.45;

pub fn setting_checkbox(
    ui: &mut Ui,
    checked: &mut bool,
    text: &str,
    is_in_effect: bool,
) -> Response {
    // Not a child Ui: a wrapping row only wraps an item whose width it knows up front.
    let opacity = ui.opacity();
    if !is_in_effect {
        ui.multiply_opacity(OUT_OF_EFFECT_OPACITY);
    }
    let response = ui.checkbox(checked, text);
    ui.set_opacity(opacity);
    response
}

pub fn group_separator(ui: &mut Ui) {
    ui.add(egui::Separator::default().spacing(GROUP_GAP));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_egui::{control_points::control_point_outline, curves::ribbon_halo_stroke};
    use eframe::egui::{ecolor::Hsva, Rgba};

    fn hue(color: Color32) -> f32 {
        Hsva::from(color).h
    }

    #[test]
    fn selection_hover_and_active_share_one_accent() {
        let visuals = style().visuals;
        let widgets = &visuals.widgets;
        assert_eq!(widgets.hovered.bg_stroke.color, ACCENT);
        assert_eq!(widgets.active.bg_stroke.color, ACCENT);
        assert_eq!(widgets.active.bg_fill, visuals.selection.bg_fill);
        assert!(visuals.slider_trailing_fill);
        for tone in [visuals.selection.bg_fill, visuals.selection.stroke.color] {
            assert!((hue(tone) - hue(ACCENT)).abs() < 0.02, "{tone:?}");
        }
    }

    #[test]
    fn canvas_panes_toolbar_and_drawer_fills_differ() {
        let fills = [
            style().visuals.panel_fill,
            PANE_FILL,
            TOOLBAR_FILL,
            DRAWER_FILL,
        ];
        for (i, a) in fills.iter().enumerate() {
            for b in &fills[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    fn over(edge: Color32, background: Color32) -> Rgba {
        let edge = Rgba::from(edge);
        edge + Rgba::from(background) * (1.0 - edge.a())
    }

    fn stands_out(fill: Color32, edge: Color32, background: Color32) -> bool {
        const MIN_INTENSITY_GAP: f32 = 0.3;
        let background_intensity = Rgba::from(background).intensity();
        [Rgba::from(fill), over(edge, background)]
            .iter()
            .any(|shown| (shown.intensity() - background_intensity).abs() >= MIN_INTENSITY_GAP)
    }

    #[test]
    fn markers_and_ribbon_stand_out_on_the_pane_and_canvas_fills() {
        let fills = (0..=255u8).map(Color32::from_gray).chain([
            Color32::RED,
            Color32::GREEN,
            Color32::BLUE,
            Color32::YELLOW,
        ]);
        for fill in fills {
            for background in [PANE_FILL, style().visuals.panel_fill] {
                for emphasized in [false, true] {
                    let outline = control_point_outline(fill, emphasized).color;
                    assert!(stands_out(fill, outline, background), "{fill:?}");
                }
                let halo = ribbon_halo_stroke(fill, 12.8).color;
                assert!(stands_out(fill, halo, background), "{fill:?}");
            }
        }
    }

    #[test]
    fn pane_card_content_stays_inside_a_card_smaller_than_its_padding() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let card = egui::Rect::from_min_size(ui.max_rect().min, vec2(10.0, 0.0));
                ui.scope_builder(UiBuilder::new().max_rect(card), |ui| {
                    let content = pane_card(ui, |ui| ui.max_rect());
                    assert!(card.contains_rect(content), "{content:?} in {card:?}");
                    pane_card(ui, |ui| ui.allocate_ui(ui.max_rect().size(), |_| {}));
                });
            });
        });
    }

    #[test]
    fn section_header_is_small_muted_uppercase_text() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                section_header(ui, "Copy as");
                ui.label("Spline");
            });
        });
        let texts: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        let [header, label] = &texts[..] else {
            panic!("{} texts", texts.len());
        };
        assert_eq!(header.galley.text(), "COPY AS");
        assert!(header.galley.size().y < label.galley.size().y);
        let color = |text: &egui::epaint::TextShape| match text.galley.job.sections[0].format.color
        {
            Color32::PLACEHOLDER => text.fallback_color,
            color => color,
        };
        assert_ne!(color(header), color(label));
    }
}
