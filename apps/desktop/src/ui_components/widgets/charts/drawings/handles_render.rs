use eframe::egui::{Color32, Painter, Pos2, Stroke};

use super::trait_def::HANDLE_RADIUS_PX;

const HANDLE_FILL: Color32 = Color32::from_rgb(0, 0, 0);
const HANDLE_STROKE: Color32 = Color32::from_rgb(88, 166, 255);
const HANDLE_STROKE_WIDTH: f32 = 1.5;

/// Paint the blue ring-with-dark-center selection handles.
pub fn paint_handles(painter: &Painter, handles: &[Pos2]) {
    for pos in handles {
        painter.circle_filled(*pos, HANDLE_RADIUS_PX, HANDLE_FILL);
        painter.circle_stroke(
            *pos,
            HANDLE_RADIUS_PX,
            Stroke::new(HANDLE_STROKE_WIDTH, HANDLE_STROKE),
        );
    }
}
