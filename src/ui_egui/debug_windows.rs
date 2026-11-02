use crate::datatypes::control_point::ControlPoint;
use crate::ui_egui::ui_common::ContentWindow;
use eframe::egui::Ui;

#[derive(Default, Debug)]
pub struct DebugWindowControlPoints {
    open: bool,
    pub control_points: Vec<ControlPoint>,
}

impl DebugWindowControlPoints {
    pub fn new() -> Self {
        Self {
            open: false,
            control_points: Vec::new(),
        }
    }

    pub fn update(&mut self, in_control_points: &[ControlPoint]) {
        self.control_points = in_control_points.to_vec();
    }
}

impl ContentWindow for DebugWindowControlPoints {
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
