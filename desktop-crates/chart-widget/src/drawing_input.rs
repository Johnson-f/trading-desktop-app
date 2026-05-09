use super::ChartWidget;
use super::drawings::{
    self, DrawingsManager, HIT_TOLERANCE_PX, SelectionInput, ToolbarEvent, paint_handles,
    selection_step, show_toolbar,
};

impl ChartWidget {
    /// Toggle the eye-icon visibility flag (UI-only, not persisted).
    pub fn toggle_drawings_visibility(&mut self) {
        self.drawings_hidden = !self.drawings_hidden;
    }

    pub fn drawings_hidden(&self) -> bool {
        self.drawings_hidden
    }

    pub(super) fn handle_drawing_input(&mut self, ui: &egui::Ui, chart_rect: egui::Rect) {
        let Some(def_id) = self.toolbar.active_drawing else {
            return;
        };
        let Some(tool) = self.drawings.tool_for(def_id) else {
            return;
        };
        let camera = self.camera.lock();
        let result = tool.handle_input(ui, chart_rect, &camera, &mut self.drawings.draft);
        drop(camera);
        if let drawings::InputResult::Commit(points) = result {
            let point_dates: Vec<String> = points
                .iter()
                .map(|p| {
                    self.raw_data
                        .dates
                        .get(p.index as usize)
                        .cloned()
                        .unwrap_or_default()
                })
                .collect();
            self.drawings.commit(tool.id(), points, point_dates);
            self.toolbar.active_drawing = None;
        }
    }

    pub(super) fn paint_drawings(
        &self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        let camera = self.camera.lock();
        // Wide clip so tools that span sub-panes (vertical lines) can reach
        // past chart_rect; other tools narrow the clip back themselves.
        let painter = ui.painter_at(full_rect);
        for drawing in &self.drawings.committed {
            let Some(tool) = self.drawings.tool_for(drawing.def_id) else {
                continue;
            };
            tool.render(
                &painter,
                chart_rect,
                full_rect,
                &camera,
                &drawing.points,
                &drawing.style,
                &drawing.kind_style,
            );
        }
        if let Some(draft) = self.drawings.draft.as_ref() {
            if let Some(tool) = self.drawings.tool_for(draft.def_id) {
                tool.render_preview(&painter, chart_rect, full_rect, &camera, &draft.points);
            }
        }
    }

    /// Show an immediate (no-delay) label naming the drawing under the cursor.
    /// Painted directly on the chart painter as a small pill anchored to the
    /// drawing's bounds (above, or below if it wouldn't fit) — never on top
    /// of the drawing itself, and position is stable so it doesn't flicker.
    /// Suppressed while placing / dragging.
    pub(super) fn paint_drawing_hover_tooltip(
        &self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        if self.drawings.draft.is_some() {
            return;
        }
        if !matches!(self.drawings.drag, drawings::SelectionDrag::None) {
            return;
        }
        let Some(pointer) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        if !chart_rect.contains(pointer) {
            return;
        }

        let camera = self.camera.lock();
        for drawing in self.drawings.committed.iter().rev() {
            let Some(tool) = self.drawings.tool_for(drawing.def_id) else {
                continue;
            };
            if !tool.hit_test(
                chart_rect,
                full_rect,
                &camera,
                &drawing.points,
                pointer,
                HIT_TOLERANCE_PX,
            ) {
                continue;
            }

            let label = tool.display_name().to_string();
            let bounds = tool.bounds(chart_rect, full_rect, &camera, &drawing.points);

            let painter = ui.painter_at(chart_rect);
            let font = egui::FontId::proportional(11.0);
            let text_color = egui::Color32::from_rgb(225, 225, 230);
            let bg = egui::Color32::from_rgba_premultiplied(30, 30, 36, 230);
            let border = egui::Color32::from_rgb(60, 60, 66);

            let galley = painter.layout_no_wrap(label, font, text_color);
            let pad = egui::vec2(8.0, 4.0);
            let size = galley.size() + pad * 2.0;

            // Anchor horizontally to the bounds' midpoint; vertically above
            // the bounds with a small gap, flipping below if that'd clip the
            // chart top. Always offset so the pill never overlaps the shape.
            let gap = 6.0;
            let mut top = bounds.top() - size.y - gap;
            if top < chart_rect.top() + 2.0 {
                top = bounds.bottom() + gap;
            }
            let mut left = bounds.center().x - size.x * 0.5;
            left = left
                .max(chart_rect.left() + 2.0)
                .min(chart_rect.right() - size.x - 2.0);

            let rect = egui::Rect::from_min_size(egui::pos2(left, top), size);
            painter.rect_filled(rect, egui::CornerRadius::same(3), bg);
            painter.rect_stroke(
                rect,
                egui::CornerRadius::same(3),
                egui::Stroke::new(0.5, border),
                egui::StrokeKind::Inside,
            );
            painter.galley(rect.min + pad, galley, text_color);
            break;
        }
    }

    pub(super) fn handle_drawing_selection(
        &mut self,
        ui: &egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        if !chart_rect.intersects(full_rect) {
            return;
        }

        let (pointer_pos, pointer_down, pointer_released, modifiers, key_pressed) = ui.input(|i| {
            (
                i.pointer.latest_pos(),
                i.pointer.primary_pressed(),
                i.pointer.primary_released(),
                i.modifiers,
                first_consumed_key(&i.events),
            )
        });

        let key_pressed = if ui.ctx().egui_wants_keyboard_input() {
            None
        } else {
            key_pressed
        };

        let (dx, dy) = {
            let camera = self.camera.lock();
            let x_range = camera.viewport.x as f64 / camera.x_scale;
            let y_range = camera.viewport.y as f64 / camera.y_scale;
            ((x_range * 0.05) as f32, (y_range * 0.05) as f32)
        };

        let toolbar_rect = self.cached_toolbar_rect(chart_rect, full_rect);

        let input = SelectionInput {
            pointer_pos,
            pointer_down,
            pointer_released,
            modifiers,
            key_pressed,
            chart_rect,
            full_rect,
            toolbar_rect,
            clone_offset: (dx, dy),
        };

        let camera = self.camera.lock();
        selection_step(&mut self.drawings, &camera, &input);
    }

    pub(super) fn paint_drawing_selection(
        &mut self,
        ui: &mut egui::Ui,
        chart_rect: egui::Rect,
        full_rect: egui::Rect,
    ) {
        // 1. Paint the blue endpoint handles for the selected drawing.
        let handles_to_paint: Option<Vec<egui::Pos2>> = {
            let camera = self.camera.lock();
            self.drawings
                .selected_drawing()
                .filter(|d| !d.locked)
                .and_then(|d| {
                    self.drawings
                        .tool_for(d.def_id)
                        .map(|t| t.handles(chart_rect, full_rect, &camera, &d.points))
                })
        };
        if let Some(handles) = handles_to_paint {
            let painter = ui.painter_at(full_rect);
            paint_handles(&painter, &handles);
        }

        // 2. Paint the floating toolbar.
        let Some(drawing_id) = self.drawings.selected else {
            return;
        };
        let Some(idx) = self
            .drawings
            .committed
            .iter()
            .position(|d| d.id == drawing_id)
        else {
            return;
        };
        let def_id = self.drawings.committed[idx].def_id;
        let Some(tool) = self.drawings.tool_for(def_id) else {
            return;
        };

        // Split-borrow `self.drawings` so show_toolbar can hold &mut drawing,
        // &mut drag, and &mut toolbar_offset simultaneously. The destructure
        // gives three disjoint &mut references, which is safe.
        // Snapshot style + kind_style before the popups so we can detect
        // in-place mutations from the color/dash/width popups (which don't
        // signal back through the event channel) and mark the manager dirty.
        let (event, style_changed) = {
            let camera = self.camera.lock();
            let DrawingsManager {
                committed,
                drag,
                toolbar_offset,
                ..
            } = &mut self.drawings;
            let drawing = &mut committed[idx];
            let style_before = drawing.style;
            let kind_style_before = drawing.kind_style;
            let (_rect, event) = show_toolbar(
                ui,
                tool.as_ref(),
                chart_rect,
                full_rect,
                &camera,
                drawing,
                toolbar_offset,
                drag,
            );
            let changed = drawing.style != style_before || drawing.kind_style != kind_style_before;
            (event, changed)
        };
        if style_changed {
            self.drawings.dirty = true;
        }

        // 3. Map toolbar events to manager mutations / modal opens.
        match event {
            ToolbarEvent::None => {}
            ToolbarEvent::Clone => {
                let (dx, dy) = {
                    let camera = self.camera.lock();
                    let x_range = camera.viewport.x as f64 / camera.x_scale;
                    let y_range = camera.viewport.y as f64 / camera.y_scale;
                    ((x_range * 0.05) as f32, (y_range * 0.05) as f32)
                };
                self.drawings.clone_selected(dx, dy);
            }
            ToolbarEvent::Delete => {
                self.drawings.remove_selected();
            }
            ToolbarEvent::OpenSettings => {
                if let Some(d) = self.drawings.selected_drawing() {
                    self.drawing_settings_modal.open_for(d);
                }
            }
            ToolbarEvent::SetAsDefault => {
                if let Some(d) = self.drawings.selected_drawing() {
                    if let Err(e) = drawings::set_user_default_style(d.style) {
                        eprintln!("Failed to save default style: {}", e);
                        self.notification = Some((
                            format!("✗ Failed to save: {}", e),
                            std::time::Instant::now(),
                        ));
                    } else {
                        self.notification = Some((
                            "✓ Saved as default style".to_string(),
                            std::time::Instant::now(),
                        ));
                    }
                }
            }
        }
    }
}

fn first_consumed_key(events: &[egui::Event]) -> Option<egui::Key> {
    for e in events {
        if let egui::Event::Key {
            key, pressed: true, ..
        } = e
        {
            return Some(*key);
        }
    }
    None
}
