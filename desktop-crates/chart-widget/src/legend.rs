use super::ChartWidget;
use super::candle::CandleData;
use super::indicators::{self, ParamValues};
use super::util;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum LegendAction {
    None,
    OpenSettings,
    Remove,
}

pub(super) struct LegendItem {
    pub(super) instance_id: u64,
    pub(super) name: String,
    pub(super) entries: Vec<indicators::LegendEntry>,
    pub(super) params: ParamValues,
}

pub(super) struct FamilyMember {
    pub(super) instance_id: u64,
    pub(super) entries: Vec<indicators::LegendEntry>,
    pub(super) params: ParamValues,
}

pub(super) struct FamilyGroup {
    pub(super) def_id: &'static str,
    pub(super) family_name: String,
    pub(super) members: Vec<FamilyMember>,
}

impl ChartWidget {
    pub(super) fn apply_legend_actions(&mut self, actions: Vec<(u64, LegendAction, ParamValues)>) {
        for (id, action, params) in actions {
            match action {
                LegendAction::OpenSettings => self.settings_modal.open_for(id, params),
                LegendAction::Remove => self.manager.remove(id),
                LegendAction::None => {}
            }
        }
    }
}

/// Paint the OHLC + change + volume row at `anchor`. Uses the candle under
/// `cursor_idx` when hovering, otherwise the last candle.
pub(super) fn paint_ohlc_row(
    ui: &egui::Ui,
    clip_rect: egui::Rect,
    data: &CandleData,
    cursor_idx: Option<usize>,
    anchor: egui::Pos2,
) {
    use egui::{Color32, FontId, Pos2};

    if data.is_empty() {
        return;
    }
    let idx = cursor_idx.unwrap_or(data.len() - 1).min(data.len() - 1);
    let c = &data.instances[idx];
    let prev_close = if idx > 0 {
        data.instances[idx - 1].close
    } else {
        c.open
    };
    let change = c.close - prev_close;
    let change_pct = if prev_close.abs() > f32::EPSILON {
        (change / prev_close) * 100.0
    } else {
        0.0
    };

    let painter = ui.painter_at(clip_rect);
    let font = FontId::monospace(12.0);
    let label_color = Color32::from_rgb(140, 140, 150);
    let value_color = Color32::from_rgb(225, 225, 230);
    let up_color = Color32::from_rgb(38, 201, 160);
    let down_color = Color32::from_rgb(255, 107, 107);
    let change_color = if change >= 0.0 { up_color } else { down_color };
    let sign = if change >= 0.0 { "+" } else { "" };

    let parts: Vec<(String, Color32)> = vec![
        ("O".into(), label_color),
        (format!("{:.2}", c.open), value_color),
        ("H".into(), label_color),
        (format!("{:.2}", c.high), value_color),
        ("L".into(), label_color),
        (format!("{:.2}", c.low), value_color),
        ("C".into(), label_color),
        (format!("{:.2}", c.close), value_color),
        (
            format!("{}{:.2} ({}{:.2}%)", sign, change, sign, change_pct),
            change_color,
        ),
        ("Vol".into(), label_color),
        (format_with_commas(c.volume as i64), value_color),
    ];

    // Pass 1: layout galleys + measure total width.
    let galleys: Vec<(std::sync::Arc<egui::Galley>, Color32)> = parts
        .into_iter()
        .map(|(text, color)| (painter.layout_no_wrap(text, font.clone(), color), color))
        .collect();
    let inter_gap = 6.0;

    // Pass 2: paint galleys.
    let mut x = anchor.x;
    let mut first = true;
    for (g, color) in galleys {
        if !first {
            x += inter_gap;
        }
        first = false;
        let size = g.size();
        painter.galley(Pos2::new(x, anchor.y), g, color);
        x += size.x;
    }
}

/// Paint the ticker identity row (symbol, company name, timeframe,
/// "Adjusted" label) at `anchor` as a row of rounded-rect chips.
/// Each chip has a faint 1px border, 4px corner radius, 6px horizontal
/// padding, and 3px vertical padding. Chips are separated by 4px gaps.
///
/// Returns the chip row's full height (~22px) so the caller can offset
/// the OHLC row directly underneath.
pub(super) fn paint_ticker_info_row(
    ui: &egui::Ui,
    clip_rect: egui::Rect,
    symbol: &str,
    company_name: Option<&str>,
    timeframe_label: &str,
    anchor: egui::Pos2,
) -> f32 {
    use egui::{FontId, Pos2, Rect, Stroke, Vec2};
    use zaned_theme::{BORDER, TEXT_DIM, TEXT_MUTED, TEXT_PRIMARY};

    let painter = ui.painter_at(clip_rect);

    // Uniform chip height regardless of font size, so all chips align.
    let chip_height: f32 = 22.0;
    let h_pad: f32 = 6.0; // horizontal padding inside chip (each side)
    let v_pad: f32 = 3.0; // vertical padding (each side) — for centering text
    let chip_gap: f32 = 4.0; // gap between chips
    let corner_radius: f32 = 4.0;

    // Segment: (galley, color)
    let sym_font = FontId::proportional(14.0);
    let small_font = FontId::proportional(12.0);

    let mut segments: Vec<(std::sync::Arc<egui::Galley>, egui::Color32)> = Vec::new();

    let sym_galley = painter.layout_no_wrap(symbol.to_string(), sym_font, TEXT_PRIMARY);
    segments.push((sym_galley, TEXT_PRIMARY));

    if let Some(name) = company_name {
        let g = painter.layout_no_wrap(name.to_string(), small_font.clone(), TEXT_MUTED);
        segments.push((g, TEXT_MUTED));
    }

    let tf_g = painter.layout_no_wrap(timeframe_label.to_string(), small_font.clone(), TEXT_MUTED);
    segments.push((tf_g, TEXT_MUTED));

    let adj_g = painter.layout_no_wrap("Adjusted".to_string(), small_font, TEXT_DIM);
    segments.push((adj_g, TEXT_DIM));

    // Paint each segment as a chip.
    let mut x = anchor.x;
    let _ = v_pad; // used conceptually; text is vertically centered via y arithmetic
    for (g, color) in segments {
        let text_w = g.size().x;
        let text_h = g.size().y;

        let chip_rect = Rect::from_min_size(
            Pos2::new(x, anchor.y),
            Vec2::new(text_w + h_pad * 2.0, chip_height),
        );

        // Faint border, no fill.
        painter.rect_stroke(
            chip_rect,
            corner_radius,
            Stroke::new(1.0, BORDER),
            egui::StrokeKind::Inside,
        );

        // Text vertically centered inside the chip.
        let text_y = anchor.y + (chip_height - text_h) * 0.5;
        painter.galley(Pos2::new(x + h_pad, text_y), g, color);

        x += chip_rect.width() + chip_gap;
    }

    chip_height // Replaces sym_height; caller uses this as the OHLC row offset.
}

/// Sort key used to order family members (e.g. multiple EMAs) by period
/// ascending. Instances without a numeric `period` param sort to the end —
/// keeping their relative insertion order under a stable sort.
pub(super) fn period_sort_key(params: &ParamValues) -> i32 {
    match params.0.get("period") {
        Some(zaned_chart_core::ParamValue::Int(v)) => *v,
        _ => i32::MAX,
    }
}

fn format_with_commas(n: i64) -> String {
    let neg = n < 0;
    let digits: Vec<char> = n.unsigned_abs().to_string().chars().collect();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    for (i, d) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*d);
    }
    if neg {
        out.insert(0, '-');
    }
    out
}

pub(super) fn draw_legend_row(
    ui: &mut egui::Ui,
    clip_rect: egui::Rect,
    anchor: egui::Pos2,
    display_name: &str,
    entries: &[indicators::LegendEntry],
    id_salt: &str,
) -> LegendAction {
    use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Vec2};

    let painter = ui.painter_at(clip_rect);
    let text_font = FontId::monospace(12.0);
    let icon_font = FontId::proportional(13.0);
    let name_color = Color32::from_rgb(240, 240, 242);
    let icon_color = Color32::from_rgb(160, 160, 170);
    let icon_hover = Color32::from_rgb(240, 240, 242);
    let pill_bg = Color32::from_rgba_premultiplied(30, 30, 34, 220);

    // Measure text upfront so we can allocate the hover rect before painting.
    let name_galley =
        painter.layout_no_wrap(display_name.to_string(), text_font.clone(), name_color);
    let name_size = name_galley.size();

    let mut entry_galleys: Vec<(std::sync::Arc<egui::Galley>, Color32)> =
        Vec::with_capacity(entries.len());
    let mut entries_width: f32 = 0.0;
    for entry in entries {
        let color = util::egui_color(entry.color);
        let g = painter.layout_no_wrap(entry.label.clone(), text_font.clone(), color);
        entries_width += 8.0 + g.size().x;
        entry_galleys.push((g, color));
    }

    let icon_slot: f32 = 20.0;
    let row_height: f32 = 18.0_f32.max(name_size.y);
    let full_width = name_size.x + entries_width;
    let hover_width = name_size.x + 6.0 + icon_slot + 2.0 + icon_slot + 4.0;
    let hit_width = full_width.max(hover_width);

    let hit_rect = Rect::from_min_size(
        anchor - Vec2::new(4.0, 2.0),
        Vec2::new(hit_width + 8.0, row_height + 4.0),
    );
    // Pure geometric pointer test. Don't use ui.interact(...).hovered() here —
    // registering the row as an interactable fights with the icon click rects
    // below (they'd steal focus and toggle .hovered() off every other frame,
    // causing the legend to flicker and swallow clicks).
    let hovered = ui
        .input(|i| i.pointer.latest_pos())
        .map(|p| hit_rect.contains(p))
        .unwrap_or(false);

    if !hovered {
        painter.galley(anchor, name_galley, name_color);
        let mut x = anchor.x + name_size.x + 8.0;
        for (g, fallback) in entry_galleys {
            let size = g.size();
            painter.galley(Pos2::new(x, anchor.y), g, fallback);
            x += size.x + 8.0;
        }
        return LegendAction::None;
    }

    let pill_rect = Rect::from_min_size(
        anchor - Vec2::new(6.0, 3.0),
        Vec2::new(hover_width + 4.0, row_height + 6.0),
    );
    painter.rect_filled(pill_rect, 4.0, pill_bg);
    painter.galley(anchor, name_galley, name_color);

    let mut action = LegendAction::None;

    let gear_x = anchor.x + name_size.x + 6.0;
    let gear_rect = Rect::from_min_size(
        Pos2::new(gear_x, anchor.y - 2.0),
        Vec2::new(icon_slot, icon_slot),
    );
    let gear_resp = ui.interact(
        gear_rect,
        ui.id().with("legend_gear").with(id_salt),
        Sense::click(),
    );
    let gear_color = if gear_resp.hovered() {
        icon_hover
    } else {
        icon_color
    };
    if gear_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        gear_rect.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::GEAR,
        icon_font.clone(),
        gear_color,
    );
    if gear_resp.clicked() {
        action = LegendAction::OpenSettings;
    }

    let x_x = gear_x + icon_slot + 2.0;
    let x_rect = Rect::from_min_size(
        Pos2::new(x_x, anchor.y - 2.0),
        Vec2::new(icon_slot, icon_slot),
    );
    let x_resp = ui.interact(
        x_rect,
        ui.id().with("legend_x").with(id_salt),
        Sense::click(),
    );
    let x_color = if x_resp.hovered() {
        icon_hover
    } else {
        icon_color
    };
    if x_resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        x_rect.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::X,
        icon_font.clone(),
        x_color,
    );
    if x_resp.clicked() {
        action = LegendAction::Remove;
    }

    action
}
