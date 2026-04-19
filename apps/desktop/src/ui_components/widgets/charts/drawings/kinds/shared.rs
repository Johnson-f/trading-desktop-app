//! Reusable input handlers shared across drawing tools. Each concrete tool
//! picks the handler that matches its interaction shape and only customises
//! rendering.

use egui::Rect;

use super::super::super::camera::Camera;
use super::super::trait_def::{DrawingDraft, InputResult, screen_to_world};

/// Standard input shape for single-click tools (horizontal line, vertical
/// line, horizontal ray). While the tool is armed the draft mirrors the
/// cursor so `render_preview` shows where the drawing would land; clicking
/// commits a one-point drawing.
pub fn one_click_input(
    ui: &egui::Ui,
    chart_rect: Rect,
    camera: &Camera,
    draft: &mut Option<DrawingDraft>,
    def_id: &'static str,
) -> InputResult {
    let pointer_pos = ui.input(|i| i.pointer.latest_pos());
    let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
    let over_chart = pointer_pos.map(|p| chart_rect.contains(p)).unwrap_or(false);

    if over_chart {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        if let Some(pos) = pointer_pos {
            let p = screen_to_world(chart_rect, camera, pos);
            *draft = Some(DrawingDraft {
                def_id,
                points: vec![p],
            });
        }
        ui.ctx().request_repaint();
    } else {
        *draft = None;
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && draft.is_some() {
        *draft = None;
        return InputResult::Continue;
    }

    if primary_clicked && over_chart {
        if let Some(pos) = pointer_pos {
            let p = screen_to_world(chart_rect, camera, pos);
            return InputResult::Commit(vec![p]);
        }
    }

    InputResult::Continue
}

/// Standard input shape for two-click tools (trend line, extended line,
/// ray). First click anchors the start (draft gets two points at the click
/// position); pointer movement updates the second point for the preview;
/// second click commits, rejecting a zero-length line.
pub fn two_click_input(
    ui: &egui::Ui,
    chart_rect: Rect,
    camera: &Camera,
    draft: &mut Option<DrawingDraft>,
    def_id: &'static str,
) -> InputResult {
    let pointer_pos = ui.input(|i| i.pointer.latest_pos());
    let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
    let over_chart = pointer_pos.map(|p| chart_rect.contains(p)).unwrap_or(false);

    if over_chart {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && draft.is_some() {
        *draft = None;
        return InputResult::Continue;
    }

    if let Some(d) = draft.as_mut() {
        if d.points.len() == 2 {
            if let Some(pos) = pointer_pos {
                d.points[1] = screen_to_world(chart_rect, camera, pos);
            }
            ui.ctx().request_repaint();
        }
    }

    if primary_clicked && over_chart {
        if let Some(pos) = pointer_pos {
            let p = screen_to_world(chart_rect, camera, pos);
            match draft.take() {
                None => {
                    *draft = Some(DrawingDraft {
                        def_id,
                        points: vec![p, p],
                    });
                }
                Some(mut d) if d.points.len() == 2 => {
                    d.points[1] = p;
                    let dx = (d.points[0].index - d.points[1].index).abs();
                    let dy = (d.points[0].price - d.points[1].price).abs();
                    if dx >= 0.01 || dy >= 0.01 {
                        return InputResult::Commit(d.points);
                    }
                }
                _ => {}
            }
        }
    }

    InputResult::Continue
}
