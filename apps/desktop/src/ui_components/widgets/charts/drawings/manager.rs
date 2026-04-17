use super::registry;
use super::trait_def::{CommittedDrawing, DrawingDraft, DrawingTool, InputResult, WorldPoint};

pub struct DrawingsManager {
    pub committed: Vec<CommittedDrawing>,
    pub draft: Option<DrawingDraft>,
    next_id: u64,
}

impl Default for DrawingsManager {
    fn default() -> Self {
        Self {
            committed: Vec::new(),
            draft: None,
            next_id: 1,
        }
    }
}

impl DrawingsManager {
    pub fn commit(&mut self, def_id: &'static str, points: Vec<WorldPoint>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.committed.push(CommittedDrawing {
            id,
            def_id,
            points,
        });
        self.draft = None;
        id
    }

    pub fn cancel_draft(&mut self) {
        self.draft = None;
    }

    /// Instantiate the tool with the given id (falls back to None if unknown).
    pub fn tool_for(&self, def_id: &str) -> Option<Box<dyn DrawingTool>> {
        registry::get(def_id).map(|def| (def.factory)())
    }
}

/// Drive the currently-active tool against the manager's draft + committed
/// lists. Caller supplies the tool; this helper wires up the `InputResult`.
pub fn dispatch_input(
    tool: &dyn DrawingTool,
    manager: &mut DrawingsManager,
    ui: &egui::Ui,
    chart_rect: egui::Rect,
    camera: &super::super::camera::Camera,
) {
    let result = tool.handle_input(ui, chart_rect, camera, &mut manager.draft);
    if let InputResult::Commit(points) = result {
        manager.commit(tool.id(), points);
    }
}
