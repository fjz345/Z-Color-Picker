use eframe::egui::*;

pub fn color_function_gradient(
    ui: &mut Ui,
    size: Vec2,
    color_at: impl Fn(f32) -> Color32,
) -> Response {
    let (rect, response) = ui.allocate_at_least(size, Sense::click_and_drag());
    if ui.is_rect_visible(rect) {
        let segments = gradient_segment_count(rect.width());
        ui.painter()
            .add(Shape::mesh(gradient_mesh(rect, segments, color_at)));
    }
    response
}

const MIN_GRADIENT_SEGMENTS: u32 = 36;
const MAX_GRADIENT_SEGMENTS: u32 = 512;
const GRADIENT_SEGMENT_WIDTH_PT: f32 = 4.0;

fn gradient_segment_count(width_pt: f32) -> u32 {
    if !width_pt.is_finite() {
        return MIN_GRADIENT_SEGMENTS;
    }
    let segments = (width_pt / GRADIENT_SEGMENT_WIDTH_PT).ceil() as u32;
    segments.clamp(MIN_GRADIENT_SEGMENTS, MAX_GRADIENT_SEGMENTS)
}

fn gradient_mesh(rect: Rect, segments: u32, color_at: impl Fn(f32) -> Color32) -> Mesh {
    assert!(segments > 0);
    let mut mesh = Mesh::default();
    for column in 0..=segments {
        let x = column as f32 / segments as f32;
        let color = color_at(x);
        mesh.colored_vertex(rect.lerp_inside(vec2(x, 0.0)), color);
        mesh.colored_vertex(rect.lerp_inside(vec2(x, 1.0)), color);
    }
    for top_left in (0..2 * segments).step_by(2) {
        let (bottom_left, top_right) = (top_left + 1, top_left + 2);
        mesh.add_triangle(top_left, bottom_left, top_right);
        mesh.add_triangle(top_right, bottom_left, top_right + 1);
    }
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_color_at(t: f32) -> Color32 {
        Color32::from_rgb((t * 255.0) as u8, ((1.0 - t) * 255.0) as u8, 7)
    }

    #[test]
    fn segment_count_is_36_for_narrow_or_invalid_widths() {
        for width in [0.0, 100.0, 144.0, -5.0, f32::NAN, f32::INFINITY] {
            assert_eq!(gradient_segment_count(width), 36, "width {width}");
        }
    }

    #[test]
    fn segment_count_targets_4pt_columns() {
        assert_eq!(gradient_segment_count(1000.0), 250);
        assert_eq!(gradient_segment_count(1001.0), 251);
    }

    #[test]
    fn segment_count_caps_at_512() {
        assert_eq!(gradient_segment_count(10000.0), 512);
    }

    #[test]
    fn segment_count_is_non_decreasing_in_width() {
        let mut previous = gradient_segment_count(0.0);
        for width in 0..=5000 {
            let count = gradient_segment_count(width as f32);
            assert!(count >= previous, "width {width}: {count} < {previous}");
            previous = count;
        }
    }

    #[test]
    fn mesh_has_two_vertices_per_column_and_two_triangles_per_segment() {
        let rect = Rect::from_min_size(pos2(10.0, 5.0), vec2(1000.0, 20.0));
        let mesh = gradient_mesh(rect, 250, test_color_at);
        assert_eq!(mesh.vertices.len(), 502);
        assert_eq!(mesh.indices.len(), 1500);
    }

    #[test]
    fn mesh_columns_span_the_rect_at_most_4pt_apart() {
        let rect = Rect::from_min_size(pos2(10.0, 5.0), vec2(1000.0, 20.0));
        let mesh = gradient_mesh(rect, 250, test_color_at);
        let xs: Vec<f32> = mesh.vertices.chunks(2).map(|c| c[0].pos.x).collect();

        assert_eq!(xs[0], rect.left());
        assert_eq!(*xs.last().unwrap(), rect.right());
        for pair in xs.windows(2) {
            assert!(pair[1] - pair[0] <= 4.0 + 1e-3, "{pair:?}");
        }
    }

    #[test]
    fn mesh_column_colors_sample_color_at() {
        let rect = Rect::from_min_size(pos2(10.0, 5.0), vec2(1000.0, 20.0));
        let segments = 250;
        let mesh = gradient_mesh(rect, segments, test_color_at);

        for (i, column) in mesh.vertices.chunks(2).enumerate() {
            let expected = test_color_at(i as f32 / segments as f32);
            assert_eq!(column[0].color, expected, "column {i}");
            assert_eq!(column[1].color, expected, "column {i}");
            assert_eq!(column[0].pos.x, column[1].pos.x, "column {i}");
        }
    }
}
