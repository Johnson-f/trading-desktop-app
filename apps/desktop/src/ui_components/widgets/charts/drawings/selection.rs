//! Pure state machine driving selection, handle / body / toolbar-offset drag,
//! and keyboard shortcuts. The `step` function takes a `SelectionInput`
//! snapshot and mutates the `DrawingsManager` in place — it's unit-testable
//! without egui.

use egui::{Modifiers, Pos2, Rect, Vec2};

use super::super::camera::Camera;
use super::manager::{DrawingsManager, SelectionDrag};
use super::trait_def::{HIT_TOLERANCE_PX, WorldPoint, screen_to_world};

/// Plain-data input snapshot.
pub struct SelectionInput {
    pub pointer_pos: Option<Pos2>,
    /// True only on the frame the pointer went down.
    pub pointer_down: bool,
    /// True only on the frame the pointer was released.
    pub pointer_released: bool,
    pub modifiers: Modifiers,
    pub key_pressed: Option<egui::Key>,
    pub chart_rect: Rect,
    pub full_rect: Rect,
    /// Toolbar anchor rect in screen space; None when nothing is selected.
    pub toolbar_rect: Option<Rect>,
    /// Clone offset in world units when Cmd/Ctrl+D is pressed. Caller supplies
    /// so step doesn't need access to camera scale math.
    pub clone_offset: (f32, f32),
}

pub fn step(manager: &mut DrawingsManager, camera: &Camera, input: &SelectionInput) {
    // 1. Keyboard shortcuts (operate on selection, if any).
    if let Some(key) = input.key_pressed {
        handle_keyboard(manager, input, key);
    }

    // 2. Advance / end active drag.
    advance_drag(manager, camera, input);
    if input.pointer_released {
        manager.drag = SelectionDrag::None;
    }

    // 3. Start drags / change selection on pointer-down.
    if input.pointer_down {
        handle_pointer_down(manager, camera, input);
    }
}

fn handle_keyboard(manager: &mut DrawingsManager, input: &SelectionInput, key: egui::Key) {
    if manager.selected.is_none() {
        return;
    }
    let shift = input.modifiers.shift;
    let cmd = input.modifiers.command; // egui maps ⌘ on macOS and Ctrl on others.
    let magnitude = if shift { 10.0 } else { 1.0 };

    match key {
        egui::Key::Escape => manager.deselect(),
        egui::Key::Delete | egui::Key::Backspace => manager.remove_selected(),
        egui::Key::D if cmd => {
            let (dx, dy) = input.clone_offset;
            manager.clone_selected(dx, dy);
        }
        egui::Key::L if cmd => manager.toggle_lock_selected(),
        egui::Key::ArrowLeft => manager.nudge_selected(-magnitude, 0.0),
        egui::Key::ArrowRight => manager.nudge_selected(magnitude, 0.0),
        egui::Key::ArrowUp => manager.nudge_selected(0.0, magnitude),
        egui::Key::ArrowDown => manager.nudge_selected(0.0, -magnitude),
        _ => {}
    }
}

fn advance_drag(manager: &mut DrawingsManager, camera: &Camera, input: &SelectionInput) {
    let Some(pos) = input.pointer_pos else {
        return;
    };
    match manager.drag {
        SelectionDrag::None => {}
        SelectionDrag::Handle {
            drawing_id,
            handle_idx,
        } => {
            let world = screen_to_world(input.chart_rect, camera, pos);
            manager.move_handle(drawing_id, handle_idx, world);
        }
        SelectionDrag::Body {
            drawing_id,
            anchor_world,
            anchor_pointer_world,
        } => {
            // Drawing anchor should equal `anchor_world + (pointer_now - anchor_pointer_world)`.
            let pointer_world = screen_to_world(input.chart_rect, camera, pos);
            let target_index =
                anchor_world.index + (pointer_world.index - anchor_pointer_world.index);
            let target_price =
                anchor_world.price + (pointer_world.price - anchor_pointer_world.price);
            // Current anchor is `committed[id].points[0]` (falls back to 0 if missing).
            let current = manager
                .committed
                .iter()
                .find(|d| d.id == drawing_id)
                .and_then(|d| d.points.first())
                .copied()
                .unwrap_or(WorldPoint {
                    index: 0.0,
                    price: 0.0,
                });
            let shift_dx = target_index - current.index;
            let shift_dy = target_price - current.price;
            manager.translate_drawing(drawing_id, shift_dx, shift_dy);
        }
        SelectionDrag::ToolbarOffset {
            start_offset,
            start_pointer,
        } => {
            manager.toolbar_offset = start_offset + (pos - start_pointer);
        }
    }
}

fn handle_pointer_down(manager: &mut DrawingsManager, camera: &Camera, input: &SelectionInput) {
    let Some(pos) = input.pointer_pos else {
        return;
    };

    // 1. Over a handle of the currently selected drawing?
    if let Some(sel_id) = manager.selected {
        let sel = manager.committed.iter().find(|d| d.id == sel_id).cloned();
        if let Some(drawing) = sel {
            if !drawing.locked {
                if let Some(tool) = manager.tool_for(drawing.def_id) {
                    let handles =
                        tool.handles(input.chart_rect, input.full_rect, camera, &drawing.points);
                    for (i, h) in handles.iter().enumerate() {
                        if (pos - *h).length() <= HIT_TOLERANCE_PX + 2.0 {
                            manager.drag = SelectionDrag::Handle {
                                drawing_id: sel_id,
                                handle_idx: i,
                            };
                            return;
                        }
                    }
                }
            }
        }
    }

    // 2. Over the floating toolbar? The grip (leftmost 20px) starts a
    //    ToolbarOffset drag; clicks anywhere else in the toolbar are handled
    //    by the toolbar widget itself (color / dash / width popups and the
    //    clone / gear / lock / trash buttons). Either way, don't touch
    //    selection state — otherwise the toolbar would vanish the instant
    //    the user tried to use it.
    if let Some(toolbar_rect) = input.toolbar_rect {
        if toolbar_rect.contains(pos) {
            let grip_rect =
                Rect::from_min_size(toolbar_rect.min, Vec2::new(20.0, toolbar_rect.height()));
            if grip_rect.contains(pos) {
                manager.drag = SelectionDrag::ToolbarOffset {
                    start_offset: manager.toolbar_offset,
                    start_pointer: pos,
                };
            }
            return;
        }
    }

    // 3. Over a drawing body? Iterate in reverse so more-recent drawings take
    // priority.
    let mut hit: Option<u64> = None;
    let committed = manager.committed.clone();
    for drawing in committed.iter().rev() {
        let Some(tool) = manager.tool_for(drawing.def_id) else {
            continue;
        };
        if tool.hit_test(
            input.chart_rect,
            input.full_rect,
            camera,
            &drawing.points,
            pos,
            HIT_TOLERANCE_PX,
        ) {
            hit = Some(drawing.id);
            break;
        }
    }

    match hit {
        Some(id) => {
            let locked = committed
                .iter()
                .find(|d| d.id == id)
                .map(|d| d.locked)
                .unwrap_or(false);
            manager.select(id);
            // Always start body drag on click — keeps the chart pan path from
            // hijacking the gesture and flipping `auto_scale_y` off. A pure
            // click with no movement just releases with the drawing unchanged.
            if !locked {
                if let Some(anchor) = committed
                    .iter()
                    .find(|d| d.id == id)
                    .and_then(|d| d.points.first())
                {
                    let pointer_world = screen_to_world(input.chart_rect, camera, pos);
                    manager.drag = SelectionDrag::Body {
                        drawing_id: id,
                        anchor_world: *anchor,
                        anchor_pointer_world: pointer_world,
                    };
                }
            }
        }
        None => {
            manager.deselect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_components::widgets::charts::drawings::manager::DrawingsManager;
    use crate::ui_components::widgets::charts::drawings::trait_def::WorldPoint;

    fn test_camera() -> Camera {
        let mut c = Camera::default();
        c.x_offset = 0.0;
        c.x_scale = 10.0;
        c.y_offset = 0.0;
        c.y_scale = 10.0;
        c.viewport = egui::Vec2::new(100.0, 100.0);
        c
    }

    fn rect() -> Rect {
        Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0))
    }

    fn manager_with_line() -> (DrawingsManager, u64) {
        let mut m = DrawingsManager::default();
        let id = m.commit(
            "trend_line",
            vec![
                WorldPoint {
                    index: 1.0,
                    price: 5.0,
                }, // screen (10, 50)
                WorldPoint {
                    index: 5.0,
                    price: 5.0,
                }, // screen (50, 50)
            ],
            vec![],
        );
        (m, id)
    }

    fn noop_input() -> SelectionInput {
        SelectionInput {
            pointer_pos: None,
            pointer_down: false,
            pointer_released: false,
            modifiers: Modifiers::default(),
            key_pressed: None,
            chart_rect: rect(),
            full_rect: rect(),
            toolbar_rect: None,
            clone_offset: (0.5, 0.0),
        }
    }

    #[test]
    fn click_empty_space_deselects() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.pointer_pos = Some(Pos2::new(80.0, 90.0));
        input.pointer_down = true;
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.selected, None);
    }

    #[test]
    fn click_on_line_selects_it() {
        let (mut m, id) = manager_with_line();
        let mut input = noop_input();
        input.pointer_pos = Some(Pos2::new(30.0, 50.0));
        input.pointer_down = true;
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.selected, Some(id));
    }

    #[test]
    fn second_click_on_selected_body_starts_body_drag() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.pointer_pos = Some(Pos2::new(30.0, 50.0));
        input.pointer_down = true;
        step(&mut m, &test_camera(), &input);
        assert!(matches!(m.drag, SelectionDrag::Body { drawing_id, .. } if drawing_id == id));
    }

    #[test]
    fn click_on_handle_starts_handle_drag() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.pointer_pos = Some(Pos2::new(10.0, 50.0)); // first handle
        input.pointer_down = true;
        step(&mut m, &test_camera(), &input);
        assert!(matches!(m.drag,
            SelectionDrag::Handle { drawing_id, handle_idx: 0 } if drawing_id == id));
    }

    #[test]
    fn release_ends_drag() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        m.drag = SelectionDrag::Body {
            drawing_id: id,
            anchor_world: WorldPoint {
                index: 1.0,
                price: 5.0,
            },
            anchor_pointer_world: WorldPoint {
                index: 1.0,
                price: 5.0,
            },
        };
        let mut input = noop_input();
        input.pointer_pos = Some(Pos2::new(50.0, 50.0));
        input.pointer_released = true;
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.drag, SelectionDrag::None);
    }

    #[test]
    fn escape_deselects() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.key_pressed = Some(egui::Key::Escape);
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.selected, None);
    }

    #[test]
    fn delete_removes_and_clears() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.key_pressed = Some(egui::Key::Delete);
        step(&mut m, &test_camera(), &input);
        assert!(m.committed.is_empty());
        assert_eq!(m.selected, None);
    }

    #[test]
    fn cmd_d_clones() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.modifiers.command = true;
        input.key_pressed = Some(egui::Key::D);
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.committed.len(), 2);
        assert_ne!(m.selected, Some(id));
    }

    #[test]
    fn arrow_nudges_selected() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        input.key_pressed = Some(egui::Key::ArrowRight);
        step(&mut m, &test_camera(), &input);
        let d = m.committed.iter().find(|d| d.id == id).unwrap();
        assert!((d.points[0].index - 2.0).abs() < 1e-3);
    }

    #[test]
    fn locked_drawing_suppresses_arrow_nudge() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        m.toggle_lock_selected();
        let mut input = noop_input();
        input.key_pressed = Some(egui::Key::ArrowRight);
        step(&mut m, &test_camera(), &input);
        let d = m.committed.iter().find(|d| d.id == id).unwrap();
        assert!((d.points[0].index - 1.0).abs() < 1e-3);
    }

    #[test]
    fn click_on_toolbar_body_keeps_selection() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        // Pointer is on a toolbar button (not the grip, not on the drawing).
        // Grip is the leftmost 20px of the toolbar (40..60), so 70 is clearly past it.
        input.pointer_pos = Some(Pos2::new(70.0, 10.0));
        input.pointer_down = true;
        input.toolbar_rect = Some(Rect::from_min_size(
            Pos2::new(40.0, 0.0),
            Vec2::new(60.0, 20.0),
        ));
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.selected, Some(id), "clicking toolbar must not deselect");
        assert_eq!(
            m.drag,
            SelectionDrag::None,
            "non-grip toolbar click must not start a drag"
        );
    }

    #[test]
    fn click_on_toolbar_grip_starts_offset_drag() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        let mut input = noop_input();
        // Pointer is inside the grip (leftmost 20px of the toolbar).
        input.pointer_pos = Some(Pos2::new(45.0, 10.0));
        input.pointer_down = true;
        input.toolbar_rect = Some(Rect::from_min_size(
            Pos2::new(40.0, 0.0),
            Vec2::new(60.0, 20.0),
        ));
        step(&mut m, &test_camera(), &input);
        assert!(matches!(m.drag, SelectionDrag::ToolbarOffset { .. }));
    }

    #[test]
    fn locked_drawing_selectable_but_no_body_drag() {
        let (mut m, id) = manager_with_line();
        m.selected = Some(id);
        m.toggle_lock_selected();
        let mut input = noop_input();
        input.pointer_pos = Some(Pos2::new(30.0, 50.0));
        input.pointer_down = true;
        step(&mut m, &test_camera(), &input);
        assert_eq!(m.selected, Some(id));
        assert_eq!(m.drag, SelectionDrag::None);
    }
}
