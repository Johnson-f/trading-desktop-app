//! Floating toolbar rendered above the currently-selected drawing. Reads
//! from a `&mut CommittedDrawing` so popup edits (color / width / dash)
//! apply instantly. Returns a `ToolbarEvent` for actions that need to flow
//! to the parent (clone, delete, open settings).

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, Vec2};

use super::super::camera::Camera;
use super::super::util::egui_color;
use super::manager::SelectionDrag;
use super::style::{COLOR_PALETTE, DashStyle, DrawingStyle, WIDTH_PRESETS};
use super::trait_def::{CommittedDrawing, DrawingTool};

pub enum ToolbarEvent {
    None,
    Clone,
    Delete,
    OpenSettings,
}

const TOOLBAR_BG: Color32 = Color32::from_rgb(36, 36, 40);
const TOOLBAR_BORDER: Color32 = Color32::from_rgb(50, 50, 55);
const ICON_COLOR: Color32 = Color32::from_rgb(160, 160, 170);
const ICON_HOVER: Color32 = Color32::from_rgb(240, 240, 242);
const LOCK_ACTIVE: Color32 = Color32::from_rgb(255, 193, 7);

const BUTTON_SIZE: Vec2 = Vec2::new(28.0, 28.0);
const GRIP_WIDTH: f32 = 20.0;
const TOOLBAR_HEIGHT: f32 = 32.0;
const TOOLBAR_FULL_WIDTH: f32 = 330.0;
const TOOLBAR_LOCKED_WIDTH: f32 = 80.0;
const ANCHOR_GAP: f32 = 8.0;

pub fn show(
    ui: &mut egui::Ui,
    tool: &dyn DrawingTool,
    chart_rect: Rect,
    full_rect: Rect,
    camera: &Camera,
    drawing: &mut CommittedDrawing,
    toolbar_offset: &mut Vec2,
    drag: &mut SelectionDrag,
) -> (Rect, ToolbarEvent) {
    let bounds = tool.bounds(chart_rect, full_rect, camera, &drawing.points);
    let width = if drawing.locked {
        TOOLBAR_LOCKED_WIDTH
    } else {
        TOOLBAR_FULL_WIDTH
    };
    let rect = anchor_rect(bounds, full_rect, width, *toolbar_offset);

    let painter = ui.painter_at(full_rect);
    painter.rect_filled(rect, CornerRadius::same(6), TOOLBAR_BG);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(0.5, TOOLBAR_BORDER),
        egui::StrokeKind::Inside,
    );

    let mut event = ToolbarEvent::None;
    let mut cursor_x = rect.left();

    // Grip
    let grip_rect = Rect::from_min_size(
        Pos2::new(cursor_x, rect.top()),
        Vec2::new(GRIP_WIDTH, rect.height()),
    );
    draw_grip(&painter, grip_rect);
    let grip_resp = ui.interact(
        grip_rect,
        ui.id().with("drawing_toolbar_grip"),
        Sense::click_and_drag(),
    );
    if grip_resp.drag_started() {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            *drag = SelectionDrag::ToolbarOffset {
                start_offset: *toolbar_offset,
                start_pointer: pos,
            };
        }
    }
    if grip_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
    }
    cursor_x += GRIP_WIDTH;

    if drawing.locked {
        // Locked: only lock + trash.
        if icon_btn(
            ui,
            &painter,
            cursor_x,
            rect.top(),
            egui_phosphor::regular::LOCK_SIMPLE,
            LOCK_ACTIVE,
            Some(LOCK_ACTIVE),
        ) {
            drawing.locked = false;
        }
        cursor_x += BUTTON_SIZE.x;
        if icon_btn(
            ui,
            &painter,
            cursor_x,
            rect.top(),
            egui_phosphor::regular::TRASH,
            ICON_COLOR,
            None,
        ) {
            event = ToolbarEvent::Delete;
        }
        return (rect, event);
    }

    // Color popup
    let color_rect =
        Rect::from_min_size(Pos2::new(cursor_x, rect.top() + 4.0), Vec2::new(20.0, 20.0));
    let color_resp = ui.interact(
        color_rect,
        ui.id().with("drawing_toolbar_color"),
        Sense::click(),
    );
    painter.rect_filled(
        color_rect,
        CornerRadius::same(3),
        egui_color(drawing.style.color),
    );
    painter.rect_stroke(
        color_rect,
        CornerRadius::same(3),
        Stroke::new(0.5, TOOLBAR_BORDER),
        egui::StrokeKind::Inside,
    );
    egui::Popup::from_toggle_button_response(&color_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| color_popup_body(ui, &mut drawing.style));
    cursor_x += 28.0;

    // Line style popup
    let dash_rect = Rect::from_min_size(Pos2::new(cursor_x, rect.top()), BUTTON_SIZE);
    let dash_resp = ui.interact(
        dash_rect,
        ui.id().with("drawing_toolbar_dash"),
        Sense::click(),
    );
    painter.text(
        dash_rect.center(),
        egui::Align2::CENTER_CENTER,
        match drawing.style.dash {
            DashStyle::Solid => egui_phosphor::regular::MINUS,
            DashStyle::Dashed => egui_phosphor::regular::DOTS_THREE_OUTLINE,
            DashStyle::Dotted => egui_phosphor::regular::DOTS_THREE,
        },
        egui::FontId::proportional(15.0),
        if dash_resp.hovered() {
            ICON_HOVER
        } else {
            ICON_COLOR
        },
    );
    egui::Popup::from_toggle_button_response(&dash_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| dash_popup_body(ui, &mut drawing.style));
    cursor_x += BUTTON_SIZE.x;

    // Width popup
    let width_rect = Rect::from_min_size(Pos2::new(cursor_x, rect.top()), BUTTON_SIZE);
    let width_resp = ui.interact(
        width_rect,
        ui.id().with("drawing_toolbar_width"),
        Sense::click(),
    );
    painter.text(
        width_rect.center(),
        egui::Align2::CENTER_CENTER,
        format!("{:.1}", drawing.style.width),
        egui::FontId::monospace(10.0),
        if width_resp.hovered() {
            ICON_HOVER
        } else {
            ICON_COLOR
        },
    );
    egui::Popup::from_toggle_button_response(&width_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| width_popup_body(ui, &mut drawing.style));
    cursor_x += BUTTON_SIZE.x;

    // Clone / Gear / Lock / Trash
    if icon_btn(
        ui,
        &painter,
        cursor_x,
        rect.top(),
        egui_phosphor::regular::COPY,
        ICON_COLOR,
        None,
    ) {
        event = ToolbarEvent::Clone;
    }
    cursor_x += BUTTON_SIZE.x;
    if icon_btn(
        ui,
        &painter,
        cursor_x,
        rect.top(),
        egui_phosphor::regular::GEAR,
        ICON_COLOR,
        None,
    ) {
        event = ToolbarEvent::OpenSettings;
    }
    cursor_x += BUTTON_SIZE.x;
    if icon_btn(
        ui,
        &painter,
        cursor_x,
        rect.top(),
        egui_phosphor::regular::LOCK_SIMPLE_OPEN,
        ICON_COLOR,
        None,
    ) {
        drawing.locked = true;
    }
    cursor_x += BUTTON_SIZE.x;
    if icon_btn(
        ui,
        &painter,
        cursor_x,
        rect.top(),
        egui_phosphor::regular::TRASH,
        ICON_COLOR,
        None,
    ) {
        event = ToolbarEvent::Delete;
    }

    (rect, event)
}

/// Compute the toolbar's anchor rect. Shared between the selection state
/// machine (grip hit-testing) and the renderer (actual positioning) so
/// the two never disagree.
pub fn anchor_rect_for(bounds: Rect, full_rect: Rect, locked: bool, offset: Vec2) -> Rect {
    let width = if locked {
        TOOLBAR_LOCKED_WIDTH
    } else {
        TOOLBAR_FULL_WIDTH
    };
    anchor_rect(bounds, full_rect, width, offset)
}

fn anchor_rect(bounds: Rect, full_rect: Rect, width: f32, offset: Vec2) -> Rect {
    let center_x = bounds.center().x;
    let mut top = bounds.top() - TOOLBAR_HEIGHT - ANCHOR_GAP;
    if top < full_rect.top() {
        top = bounds.bottom() + ANCHOR_GAP;
    }
    let rect = Rect::from_min_size(
        Pos2::new(center_x - width / 2.0, top),
        Vec2::new(width, TOOLBAR_HEIGHT),
    )
    .translate(offset);

    // Clamp inside full_rect.
    let dx =
        (full_rect.right() - rect.right()).min(0.0) + (full_rect.left() - rect.left()).max(0.0);
    let dy =
        (full_rect.bottom() - rect.bottom()).min(0.0) + (full_rect.top() - rect.top()).max(0.0);
    rect.translate(Vec2::new(dx, dy))
}

fn draw_grip(painter: &eframe::egui::Painter, rect: Rect) {
    let cx = rect.center().x;
    let cy = rect.center().y;
    let dot_color = ICON_COLOR;
    for (dx, dy) in [
        (-2.0, -4.0),
        (2.0, -4.0),
        (-2.0, 0.0),
        (2.0, 0.0),
        (-2.0, 4.0),
        (2.0, 4.0),
    ] {
        painter.circle_filled(Pos2::new(cx + dx, cy + dy), 1.0, dot_color);
    }
}

fn icon_btn(
    ui: &mut egui::Ui,
    painter: &eframe::egui::Painter,
    x: f32,
    y: f32,
    icon: &str,
    idle: Color32,
    active: Option<Color32>,
) -> bool {
    let r = Rect::from_min_size(Pos2::new(x, y), BUTTON_SIZE);
    let resp = ui.interact(
        r,
        ui.id().with(("drawing_toolbar_btn", x as i32)),
        Sense::click(),
    );
    let color = if resp.hovered() {
        ICON_HOVER
    } else {
        active.unwrap_or(idle)
    };
    painter.text(
        r.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(15.0),
        color,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.clicked()
}

fn color_popup_body(ui: &mut egui::Ui, style: &mut DrawingStyle) {
    ui.horizontal_wrapped(|ui| {
        for c in COLOR_PALETTE {
            let size = Vec2::new(18.0, 18.0);
            let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
            ui.painter()
                .rect_filled(rect, CornerRadius::same(3), egui_color(*c));
            if resp.clicked() {
                style.color = *c;
            }
        }
    });
}

fn dash_popup_body(ui: &mut egui::Ui, style: &mut DrawingStyle) {
    for (d, label) in [
        (DashStyle::Solid, "Solid"),
        (DashStyle::Dashed, "Dashed"),
        (DashStyle::Dotted, "Dotted"),
    ] {
        if ui.selectable_label(style.dash == d, label).clicked() {
            style.dash = d;
        }
    }
}

fn width_popup_body(ui: &mut egui::Ui, style: &mut DrawingStyle) {
    for w in WIDTH_PRESETS {
        let label = format!("{:.1} px", w);
        if ui
            .selectable_label((style.width - w).abs() < 1e-3, label)
            .clicked()
        {
            style.width = *w;
        }
    }
    ui.separator();
    ui.add(egui::Slider::new(&mut style.width, 0.5..=5.0).text("px"));
}
