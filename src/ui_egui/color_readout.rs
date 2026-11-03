use eframe::egui::{
    style::Spacing, text::LayoutJob, Align, CursorIcon, FontSelection, Label, PointerButton, Pos2,
    RichText, Sense, Ui,
};
use eframe::epaint::Color32;

use crate::{
    color_space::{linear_to_oklab, oklab_to_oklch, srgb_to_hsl, srgb_to_linear},
    common::ColorStringCopy,
    ui_egui::color_picker::{format_color_as, NO_SELECTION_HINT},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadoutRow {
    Hex,
    Srgb,
    Hsl,
    Oklab,
    Oklch,
}

impl ReadoutRow {
    const ALL: [Self; 5] = [Self::Hex, Self::Srgb, Self::Hsl, Self::Oklab, Self::Oklch];

    fn label(self) -> &'static str {
        match self {
            Self::Hex => "Hex",
            Self::Srgb => "sRGB",
            Self::Hsl => "HSL",
            Self::Oklab => "Oklab",
            Self::Oklch => "OkLCh",
        }
    }

    fn copy_format(self) -> ColorStringCopy {
        match self {
            Self::Hex => ColorStringCopy::HEXNOA,
            Self::Srgb => ColorStringCopy::SRGB,
            Self::Hsl => ColorStringCopy::HSL,
            Self::Oklab => ColorStringCopy::OKLAB,
            Self::Oklch => ColorStringCopy::OKLCH,
        }
    }
}

fn fixed(value: f32, decimals: usize, width: usize) -> String {
    let scale = 10f32.powi(decimals as i32);
    // + 0.0 turns the -0.0 of a tiny negative into 0.0.
    let rounded = (value * scale).round() / scale + 0.0;
    format!("{rounded:>width$.decimals$}")
}

fn row_values(color: Color32, row: ReadoutRow) -> String {
    let [r, g, b, _] = color.to_srgba_unmultiplied();
    let srgb = [r, g, b].map(|c| c as f32 / 255.0);
    let oklab = || linear_to_oklab(srgb_to_linear(srgb));
    let degrees = |turns: f32| fixed(turns * 360.0, 1, 5);
    let percent = |fraction: f32| fixed(fraction * 100.0, 1, 5);
    match row {
        ReadoutRow::Hex => format!("{r:02X}{g:02X}{b:02X}"),
        ReadoutRow::Srgb => format!("{r:>3} {g:>3} {b:>3}"),
        ReadoutRow::Hsl => {
            let [h, s, l] = srgb_to_hsl(srgb);
            format!("{}° {}% {}%", degrees(h), percent(s), percent(l))
        }
        ReadoutRow::Oklab => {
            let [l, a, b] = oklab();
            format!("{} {} {}", fixed(l, 3, 5), fixed(a, 3, 6), fixed(b, 3, 6))
        }
        ReadoutRow::Oklch => {
            let [l, c, h] = oklab_to_oklch(oklab());
            format!("{} {} {}°", fixed(l, 3, 5), fixed(c, 3, 5), degrees(h))
        }
    }
}

fn row_label(row: ReadoutRow) -> String {
    format!("{:<5} ", row.label())
}

fn row_text(color: Option<Color32>, row: ReadoutRow) -> String {
    let values = color.map_or_else(|| "-".to_owned(), |color| row_values(color, row));
    format!("{}{values}", row_label(row))
}

fn row_job(ui: &Ui, color: Color32, row: ReadoutRow) -> LayoutJob {
    let mut job = LayoutJob::default();
    let style = ui.style();
    RichText::new(row_label(row))
        .monospace()
        .color(ui.visuals().text_color())
        .append_to(&mut job, style, FontSelection::Default, Align::Center);
    RichText::new(row_values(color, row)).monospace().append_to(
        &mut job,
        style,
        FontSelection::Default,
        Align::Center,
    );
    job
}

pub fn color_readout_height(spacing: &Spacing) -> f32 {
    ReadoutRow::ALL.len() as f32 * spacing.interact_size.y
}

fn readout_row(ui: &mut Ui, color: Option<Color32>, row: ReadoutRow) -> Option<Pos2> {
    ui.horizontal(|ui| {
        let Some(color) = color else {
            let text = RichText::new(row_text(None, row)).monospace().weak();
            ui.add(Label::new(text).truncate())
                .on_hover_text(NO_SELECTION_HINT);
            return None;
        };
        let copied = format_color_as(color, row.copy_format(), None);
        let response = ui
            .add(
                Label::new(row_job(ui, color, row))
                    .truncate()
                    .selectable(false)
                    .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(format!("Click: copy {copied}"));
        response
            .clicked_by(PointerButton::Primary)
            .then(|| response.rect.left_bottom())
    })
    .inner
}

pub fn color_readout(ui: &mut Ui, color: Option<Color32>) -> Option<(Pos2, ColorStringCopy)> {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut request = None;
        for row in ReadoutRow::ALL {
            if let Some(anchor) = readout_row(ui, color, row) {
                request = Some((anchor, row.copy_format()));
            }
        }
        request
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        color_space::{BlendSpace, PickerSpace},
        common::SplineMode,
        datatypes::control_point::{ControlPoint, ControlPointValue},
        ui_egui::{
            clipboard::{selected_color_payload, ClipboardPayload},
            color_picker::ZColorPickerWrapper,
        },
    };
    use eframe::egui::{self, Rect, Shape};
    use ReadoutRow::{Oklab, Oklch};

    fn assert_rows(color: Option<Color32>, expected: [&str; 5]) {
        for (row, expected) in ReadoutRow::ALL.into_iter().zip(expected) {
            assert_eq!(row_text(color, row), expected, "{color:?} {row:?}");
        }
    }

    #[test]
    fn rows_of_known_colors() {
        assert_rows(
            Some(Color32::WHITE),
            [
                "Hex   FFFFFF",
                "sRGB  255 255 255",
                "HSL     0.0°   0.0% 100.0%",
                "Oklab 1.000  0.000  0.000",
                "OkLCh 1.000 0.000   0.0°",
            ],
        );
        assert_rows(
            Some(Color32::BLACK),
            [
                "Hex   000000",
                "sRGB    0   0   0",
                "HSL     0.0°   0.0%   0.0%",
                "Oklab 0.000  0.000  0.000",
                "OkLCh 0.000 0.000   0.0°",
            ],
        );
        assert_rows(
            Some(Color32::from_rgb(255, 0, 0)),
            [
                "Hex   FF0000",
                "sRGB  255   0   0",
                "HSL     0.0° 100.0%  50.0%",
                "Oklab 0.628  0.225  0.126",
                "OkLCh 0.628 0.258  29.2°",
            ],
        );
        assert_rows(
            Some(Color32::from_gray(128)),
            [
                "Hex   808080",
                "sRGB  128 128 128",
                "HSL     0.0°   0.0%  50.2%",
                "Oklab 0.600  0.000  0.000",
                "OkLCh 0.600 0.000   0.0°",
            ],
        );
    }

    #[test]
    fn negative_components_keep_the_row_width() {
        let blue = Color32::from_rgb(0, 0, 255);
        assert_eq!(row_text(Some(blue), Oklab), "Oklab 0.452 -0.032 -0.312");
        assert_eq!(row_text(Some(blue), Oklch), "OkLCh 0.452 0.313 264.1°");
        for row in ReadoutRow::ALL {
            assert_eq!(
                row_text(Some(blue), row).chars().count(),
                row_text(Some(Color32::WHITE), row).chars().count(),
                "{row:?}"
            );
        }
    }

    #[test]
    fn rows_without_a_color_are_dashes() {
        assert_rows(
            None,
            ["Hex   -", "sRGB  -", "HSL   -", "Oklab -", "OkLCh -"],
        );
    }

    const SCREEN: Rect = Rect {
        min: Pos2::ZERO,
        max: Pos2::new(600.0, 900.0),
    };

    #[test]
    fn readout_height_is_the_same_with_and_without_a_color() {
        for color in [None, Some(Color32::WHITE)] {
            let ctx = egui::Context::default();
            let mut height = 0.0;
            let input = egui::RawInput {
                screen_rect: Some(SCREEN),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    height = ui
                        .scope(|ui| color_readout(ui, color))
                        .response
                        .rect
                        .height();
                    assert_eq!(height, color_readout_height(ui.spacing()), "{color:?}");
                });
            });
            assert!(height > 0.0);
        }
    }

    fn red_point() -> Vec<ControlPoint> {
        vec![ControlPoint::new_simple(
            ControlPointValue::new(1.0, 1.0, 0.0),
            0.0,
        )]
    }

    fn picker_frame(
        ctx: &egui::Context,
        picker: &mut ZColorPickerWrapper,
        points: &mut Vec<ControlPoint>,
        mut picker_space: PickerSpace,
        events: Vec<egui::Event>,
    ) -> Vec<(String, Rect)> {
        let input = egui::RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..Default::default()
        };
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                picker.draw_ui(
                    ui,
                    points,
                    SplineMode::Linear,
                    &mut picker_space,
                    BlendSpace::Picker,
                    &ColorStringCopy::HSV,
                );
            });
        });
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Text(text) => Some((
                    text.galley.text().to_owned(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                _ => None,
            })
            .collect()
    }

    fn picker_selecting(selected: Option<usize>) -> ZColorPickerWrapper {
        ZColorPickerWrapper {
            last_modifying_point_index: selected,
            ..Default::default()
        }
    }

    fn painted_texts(selected: Option<usize>) -> Vec<String> {
        let mut picker = picker_selecting(selected);
        let ctx = egui::Context::default();
        picker_frame(
            &ctx,
            &mut picker,
            &mut red_point(),
            PickerSpace::Hsv,
            Vec::new(),
        )
        .into_iter()
        .map(|(text, _)| text)
        .collect()
    }

    #[test]
    fn picker_shows_the_rows_of_the_selected_point() {
        let red = Some(red_point()[0].val().color(PickerSpace::Hsv));
        let selected = painted_texts(Some(0));
        let unselected = painted_texts(None);
        for row in ReadoutRow::ALL {
            assert!(
                selected.contains(&row_text(red, row)),
                "{row:?}: {selected:?}"
            );
            assert!(
                unselected.contains(&row_text(None, row)),
                "{row:?}: {unselected:?}"
            );
        }
    }

    #[test]
    fn rows_show_the_selected_point_through_the_picker_space() {
        let mut hsl_red = vec![ControlPoint::new_simple(
            ControlPointValue::new(1.0, 0.5, 0.0),
            0.0,
        )];
        let texts: Vec<String> = picker_frame(
            &egui::Context::default(),
            &mut picker_selecting(Some(0)),
            &mut hsl_red,
            PickerSpace::Hsl,
            Vec::new(),
        )
        .into_iter()
        .map(|(text, _)| text)
        .collect();
        for row in ReadoutRow::ALL {
            let expected = row_text(Some(Color32::from_rgb(255, 0, 0)), row);
            assert!(texts.contains(&expected), "{row:?}: {texts:?}");
        }
    }

    fn click_text(text: &str) -> Option<(Pos2, ColorStringCopy)> {
        let ctx = egui::Context::default();
        let mut picker = picker_selecting(Some(0));
        let mut points = red_point();
        let (_, rect) = picker_frame(&ctx, &mut picker, &mut points, PickerSpace::Hsv, Vec::new())
            .into_iter()
            .find(|(painted, _)| painted == text)
            .unwrap_or_else(|| panic!("{text} not painted"));
        let pos = rect.center();
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        for events in [
            vec![egui::Event::PointerMoved(pos)],
            vec![button(true)],
            vec![button(false)],
        ] {
            let _ = picker_frame(&ctx, &mut picker, &mut points, PickerSpace::Hsv, events);
        }
        assert_eq!(points, red_point());
        picker.copy_request
    }

    #[test]
    fn clicking_a_row_copies_it_in_its_format() {
        let red = Some(red_point()[0].val().color(PickerSpace::Hsv));
        let expected = [
            "FF0000",
            "rgb(255, 0, 0)",
            "hsl(0 100% 50%)",
            "oklab(0.62796 0.22486 0.12585)",
            "oklch(0.62796 0.25768 29.23)",
        ];
        for (row, expected) in ReadoutRow::ALL.into_iter().zip(expected) {
            let (_, format) = click_text(&row_text(red, row))
                .unwrap_or_else(|| panic!("{row:?}: no copy request"));
            assert_eq!(format, row.copy_format(), "{row:?}");
            assert_eq!(
                selected_color_payload(&red_point(), Some(0), PickerSpace::Hsv, format),
                Some(ClipboardPayload::Text(expected.to_owned())),
                "{row:?}"
            );
        }
    }

    #[test]
    fn row_labels_are_dimmer_than_their_values() {
        let ctx = egui::Context::default();
        crate::ui_egui::theme::apply(&ctx);
        let output = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| color_readout(ui, Some(Color32::WHITE)));
        });
        let rows: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                Shape::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(rows.len(), ReadoutRow::ALL.len());
        let intensity = |text: &egui::epaint::TextShape, section: usize| {
            let color = match text.galley.job.sections[section].format.color {
                Color32::PLACEHOLDER => text.fallback_color,
                color => color,
            };
            egui::Rgba::from(color).intensity()
        };
        for row in &rows {
            let last = row.galley.job.sections.len() - 1;
            assert!(
                intensity(row, 0) < intensity(row, last),
                "{}",
                row.galley.text()
            );
        }
    }

    #[test]
    fn clicking_the_main_readout_still_copies_in_the_copy_as_format() {
        let red = Color32::from_rgb(255, 0, 0);
        let readout = format_color_as(red, ColorStringCopy::HSV, None);
        let (_, format) = click_text(&readout).expect("no copy request");
        assert_eq!(format, ColorStringCopy::HSV);
    }
}
