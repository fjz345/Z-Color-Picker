use crate::egui::Pos2;
use crate::ui_common::DebugWindow;
use crate::{control_point::ControlPoint, math::color_lerp_ex};
use ecolor::HsvaGamma;
use eframe::egui::color_picker::show_color;
use eframe::egui::{Rect, Slider, Ui, Vec2};

pub struct DebugWindowControlPoints {
    open: bool,
    pub position: Pos2,
    pub control_points: Vec<ControlPoint>,
}

impl DebugWindowControlPoints {
    pub fn new(window_position: Pos2) -> Self {
        Self {
            open: false,
            position: window_position,
            control_points: Vec::new(),
        }
    }

    pub fn update(&mut self, in_control_points: &[ControlPoint]) {
        self.control_points = in_control_points.to_vec();
    }
}

impl DebugWindow for DebugWindowControlPoints {
    fn title(&self) -> &str {
        "Debug Control Points"
    }

    fn is_open(&self) -> bool {
        return self.open;
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn open(&mut self) {
        self.open = true;
    }

    fn draw_content(&mut self, ui: &mut Ui) {
        for i in 0..self.control_points.len() {
            let point = &self.control_points[i];
            ui.label(format!("[{i}]"));
            ui.label(format!(
                "- val: {:.4},{:.4},{:.4}",
                point.val()[0],
                point.val()[1],
                point.val()[2]
            ));
            ui.label(format!("- t: {:.4}", point.t(),));
            for (tangent_index, tangent) in point.tangents().iter().enumerate() {
                if let Some(tang) = tangent {
                    ui.label(format!(
                        "- tangent{tangent_index}: {:.4},{:.4},{:.4}",
                        tang[0], tang[1], tang[2]
                    ));
                }
            }
            ui.label(format!(""));
        }
    }
}

pub struct DebugWindowTestWindow {
    open: bool,
    pub position: Pos2,
    debug_t: f32,
    debug_c: f32,
    debug_alpha: f32,
    pub source_color: HsvaGamma,
    pub target_color: HsvaGamma,
}

impl DebugWindowTestWindow {
    pub fn new(window_position: Pos2) -> Self {
        Self {
            open: false,
            position: window_position,
            debug_t: 0.0,
            debug_c: 0.0,
            debug_alpha: 0.0,
            source_color: HsvaGamma::default(),
            target_color: HsvaGamma::default(),
        }
    }

    pub fn update(&mut self, source_color: HsvaGamma, target_color: HsvaGamma) {
        self.source_color = source_color;
        self.target_color = target_color;
    }
}

impl DebugWindow for DebugWindowTestWindow {
    fn title(&self) -> &str {
        "Debug Test Window"
    }

    fn is_open(&self) -> bool {
        return self.open;
    }

    fn close(&mut self) {
        self.open = false;
    }

    fn open(&mut self) {
        self.open = true;
    }

    fn draw_content(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.add(Slider::new(&mut self.debug_t, 0.0..=1.0).text("debug_t"));
            ui.add(Slider::new(&mut self.debug_c, 0.0..=1.0).text("debug_C"));
            ui.add(Slider::new(&mut self.debug_alpha, 0.0..=1.0).text("debug_alpha"));
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let src_color = self.source_color;
            let trg_color = self.target_color;
            let res_color = color_lerp_ex(
                src_color.into(),
                trg_color.into(),
                self.debug_t,
                self.debug_c,
                self.debug_alpha,
            );

            let show_size = 100.0;
            show_color(ui, src_color, Vec2::new(show_size, show_size));
            show_color(ui, trg_color, Vec2::new(show_size, show_size));
            show_color(ui, res_color, Vec2::new(show_size, show_size));
        });
    }
}
