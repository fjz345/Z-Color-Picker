use eframe::egui::*;

pub fn color_function_gradient(
    ui: &mut Ui,
    size: Vec2,
    color_at: impl Fn(f32) -> Color32,
) -> Response {
    let (rect, response) = ui.allocate_at_least(size, Sense::click_and_drag());

    if ui.is_rect_visible(rect) {
        let _visuals = ui.style().interact(&response);

        // background_checkers(ui.painter(), rect); // for alpha:

        {
            let num_: u32 = 6 * 6;
            // fill color:
            let mut mesh = Mesh::default();
            for i in 0..=num_ {
                let t = i as f32 / (num_ as f32);
                let color = color_at(t);
                let x = lerp(rect.left()..=rect.right(), t);
                mesh.colored_vertex(pos2(x, rect.top()), color);
                mesh.colored_vertex(pos2(x, rect.bottom()), color);
                if i < num_ {
                    mesh.add_triangle(2 * i + 0, 2 * i + 1, 2 * i + 2);
                    mesh.add_triangle(2 * i + 1, 2 * i + 2, 2 * i + 3);
                }
            }
            ui.painter().add(Shape::mesh(mesh));
        }
    }

    response
}
