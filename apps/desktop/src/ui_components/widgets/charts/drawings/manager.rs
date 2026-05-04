use egui::{Pos2, Vec2};
use zaned_chart_core::CandleData;

use super::registry;
use super::trait_def::{CommittedDrawing, DrawingDraft, DrawingTool, WorldPoint};

pub struct DrawingsManager {
    pub committed: Vec<CommittedDrawing>,
    pub draft: Option<DrawingDraft>,
    pub selected: Option<u64>,
    pub toolbar_offset: Vec2,
    pub drag: SelectionDrag,
    next_id: u64,
    /// Set true by every committed-Vec mutation. Drained by ChartWidget each
    /// frame to schedule a debounced async save to SQLite.
    pub dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SelectionDrag {
    None,
    Handle {
        drawing_id: u64,
        handle_idx: usize,
    },
    Body {
        drawing_id: u64,
        anchor_world: WorldPoint,
        anchor_pointer_world: WorldPoint,
    },
    ToolbarOffset {
        start_offset: Vec2,
        start_pointer: Pos2,
    },
}

impl Default for DrawingsManager {
    fn default() -> Self {
        Self {
            committed: Vec::new(),
            draft: None,
            selected: None,
            toolbar_offset: Vec2::ZERO,
            drag: SelectionDrag::None,
            next_id: 1,
            dirty: false,
        }
    }
}

impl DrawingsManager {
    pub fn commit(
        &mut self,
        def_id: &'static str,
        points: Vec<WorldPoint>,
        point_dates: Vec<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let kind_style = registry::get(def_id)
            .map(|def| (def.factory)().default_kind_style())
            .unwrap_or(super::kind_style::KindStyle::None);
        self.committed.push(CommittedDrawing {
            id,
            def_id,
            points,
            point_dates,
            style: super::style::DrawingStyle::default(),
            kind_style,
            locked: false,
        });
        self.draft = None;
        self.dirty = true;
        id
    }

    /// Remap every committed drawing's point indices to match `data` (a
    /// potentially aggregated CandleData). Each point carries a stored date;
    /// we look up the aggregated candle that contains that date and update
    /// `index` accordingly.
    pub fn remap_to_data(&mut self, data: &CandleData) {
        for drawing in &mut self.committed {
            for (i, point) in drawing.points.iter_mut().enumerate() {
                if let Some(date) = drawing.point_dates.get(i) {
                    if !date.is_empty() {
                        if let Some(idx) = data.index_for_date(date) {
                            point.index = idx as f32;
                        }
                    }
                }
            }
        }
    }

    pub fn cancel_draft(&mut self) {
        self.draft = None;
    }

    /// Instantiate the tool with the given id (falls back to None if unknown).
    pub fn tool_for(&self, def_id: &str) -> Option<Box<dyn DrawingTool>> {
        registry::get(def_id).map(|def| (def.factory)())
    }

    pub fn select(&mut self, id: u64) {
        self.selected = Some(id);
        self.toolbar_offset = Vec2::ZERO;
    }

    pub fn deselect(&mut self) {
        self.selected = None;
        self.toolbar_offset = Vec2::ZERO;
        self.drag = SelectionDrag::None;
    }

    pub fn selected_drawing(&self) -> Option<&CommittedDrawing> {
        let id = self.selected?;
        self.committed.iter().find(|d| d.id == id)
    }

    pub fn selected_drawing_mut(&mut self) -> Option<&mut CommittedDrawing> {
        let id = self.selected?;
        self.committed.iter_mut().find(|d| d.id == id)
    }

    /// Returns the id of the new clone, or None if nothing was selected.
    /// Offsets every point by (dx, dy) in world space and selects the clone.
    pub fn clone_selected(&mut self, dx: f32, dy: f32) -> Option<u64> {
        let src = self.selected_drawing()?.clone();
        let id = self.next_id;
        self.next_id += 1;
        let points = src
            .points
            .iter()
            .map(|p| WorldPoint {
                index: p.index + dx,
                price: p.price + dy,
            })
            .collect();
        self.committed.push(CommittedDrawing {
            id,
            def_id: src.def_id,
            points,
            point_dates: src.point_dates.clone(),
            style: src.style,
            kind_style: src.kind_style,
            locked: false,
        });
        self.selected = Some(id);
        self.dirty = true;
        Some(id)
    }

    pub fn remove_selected(&mut self) {
        let Some(id) = self.selected else {
            return;
        };
        self.committed.retain(|d| d.id != id);
        self.selected = None;
        self.toolbar_offset = Vec2::ZERO;
        self.drag = SelectionDrag::None;
        self.dirty = true;
    }

    pub fn toggle_lock_selected(&mut self) {
        if let Some(d) = self.selected_drawing_mut() {
            d.locked = !d.locked;
            self.dirty = true;
        }
    }

    /// Translate every point of the selected drawing by (dx, dy) in world units.
    /// Suppressed when the drawing is locked.
    pub fn nudge_selected(&mut self, dx: f32, dy: f32) {
        let Some(d) = self.selected_drawing_mut() else {
            return;
        };
        if d.locked {
            return;
        }
        for p in d.points.iter_mut() {
            p.index += dx;
            p.price += dy;
        }
        self.dirty = true;
    }

    /// Translate a specific drawing's handle to a new world position. Delegates
    /// to the tool's `apply_handle_move` so tools with synthesized handles
    /// (Rectangle's 4 corners from 2 stored points) can map the drag back to
    /// their own point layout.
    pub fn move_handle(&mut self, drawing_id: u64, handle_idx: usize, target: WorldPoint) {
        let Some(idx) = self.committed.iter().position(|d| d.id == drawing_id) else {
            return;
        };
        if self.committed[idx].locked {
            return;
        }
        let def_id = self.committed[idx].def_id;
        let Some(tool) = self.tool_for(def_id) else {
            return;
        };
        tool.apply_handle_move(&mut self.committed[idx].points, handle_idx, target);
        self.dirty = true;
    }

    /// Translate every point of a drawing by a world-space delta.
    pub fn translate_drawing(&mut self, drawing_id: u64, dx: f32, dy: f32) {
        let Some(d) = self.committed.iter_mut().find(|d| d.id == drawing_id) else {
            return;
        };
        if d.locked {
            return;
        }
        for p in d.points.iter_mut() {
            p.index += dx;
            p.price += dy;
        }
        self.dirty = true;
    }

    /// Wholesale replacement (used by the "load from DB" path on symbol switch).
    /// Does NOT mark dirty — we don't want to immediately re-save what we just
    /// loaded.
    pub fn replace_committed(&mut self, drawings: Vec<CommittedDrawing>) {
        let max_id = drawings.iter().map(|d| d.id).max().unwrap_or(0);
        self.committed = drawings;
        self.selected = None;
        self.draft = None;
        self.drag = SelectionDrag::None;
        self.toolbar_offset = Vec2::ZERO;
        self.next_id = max_id + 1;
        self.dirty = false;
    }

    /// Clear all committed drawings (used by the trash icon). Marks dirty so
    /// the chart's debounced save fires and replicates the clear to the DB.
    pub fn clear_all(&mut self) {
        if self.committed.is_empty() {
            return;
        }
        self.committed.clear();
        self.selected = None;
        self.draft = None;
        self.drag = SelectionDrag::None;
        self.toolbar_offset = Vec2::ZERO;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui_components::widgets::charts::drawings::trait_def::WorldPoint;

    fn make_manager_with_one() -> (DrawingsManager, u64) {
        let mut m = DrawingsManager::default();
        let id = m.commit(
            "trend_line",
            vec![
                WorldPoint {
                    index: 1.0,
                    price: 10.0,
                },
                WorldPoint {
                    index: 5.0,
                    price: 20.0,
                },
            ],
            vec![],
        );
        (m, id)
    }

    #[test]
    fn clone_selected_offsets_and_reselects() {
        let (mut m, id) = make_manager_with_one();
        m.selected = Some(id);
        let new_id = m.clone_selected(0.5, 1.0).unwrap();
        assert_ne!(new_id, id);
        assert_eq!(m.committed.len(), 2);
        assert_eq!(m.selected, Some(new_id));
        let cloned = m.committed.iter().find(|d| d.id == new_id).unwrap();
        assert!((cloned.points[0].index - 1.5).abs() < 1e-3);
        assert!((cloned.points[0].price - 11.0).abs() < 1e-3);
    }

    #[test]
    fn clone_selected_noop_when_nothing_selected() {
        let (mut m, _) = make_manager_with_one();
        m.selected = None;
        assert_eq!(m.clone_selected(0.5, 1.0), None);
        assert_eq!(m.committed.len(), 1);
    }

    #[test]
    fn remove_selected_clears_selection() {
        let (mut m, id) = make_manager_with_one();
        m.selected = Some(id);
        m.remove_selected();
        assert!(m.committed.is_empty());
        assert_eq!(m.selected, None);
    }

    #[test]
    fn toggle_lock_flips() {
        let (mut m, id) = make_manager_with_one();
        m.selected = Some(id);
        assert!(!m.committed[0].locked);
        m.toggle_lock_selected();
        assert!(m.committed[0].locked);
        m.toggle_lock_selected();
        assert!(!m.committed[0].locked);
    }

    #[test]
    fn nudge_selected_shifts_all_points() {
        let (mut m, id) = make_manager_with_one();
        m.selected = Some(id);
        m.nudge_selected(0.25, -0.5);
        let d = &m.committed[0];
        assert!((d.points[0].index - 1.25).abs() < 1e-3);
        assert!((d.points[0].price - 9.5).abs() < 1e-3);
        assert!((d.points[1].index - 5.25).abs() < 1e-3);
        assert!((d.points[1].price - 19.5).abs() < 1e-3);
    }

    #[test]
    fn nudge_selected_suppressed_when_locked() {
        let (mut m, id) = make_manager_with_one();
        m.selected = Some(id);
        m.toggle_lock_selected();
        m.nudge_selected(0.25, -0.5);
        let d = &m.committed[0];
        // Unchanged.
        assert!((d.points[0].index - 1.0).abs() < 1e-3);
    }
}
