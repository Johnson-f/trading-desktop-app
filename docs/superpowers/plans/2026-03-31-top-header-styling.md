# Top Header Styling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the top header into a polished, grouped navigation bar with indigo accent, hover/active states, and premium fintech aesthetic.

**Architecture:** Single-file rewrite of `top_header.rs`. The `TopHeader` struct gains state fields (`active_group`, `active_icon`). Helper methods paint individual sections (brand, icon groups, search, bell, avatar). All colors are constants at the top of the file for easy tuning.

**Tech Stack:** Rust, egui 0.34, eframe 0.34, egui-phosphor 0.12

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `apps/desktop/src/components/top_header.rs` | Rewrite | Full header: struct, state, color constants, layout, painting |
| `apps/desktop/src/main.rs` | No change | Already creates `TopHeader::default()` and calls `.show(ui)` |

No new files needed. The entire change is self-contained in `top_header.rs`.

---

### Task 1: Define Color Constants and Update Struct

**Files:**
- Modify: `apps/desktop/src/components/top_header.rs:1-6`

- [ ] **Step 1: Replace the imports and struct definition**

Replace the entire file content with the new module skeleton — imports, color constants, and updated struct with state fields. The `show()` method will be a placeholder that just renders the header frame.

```rust
use eframe::egui::{self, Align, Color32, CornerRadius, Layout, Pos2, Rect, RichText, Stroke, Vec2};

// ── Color Palette ──────────────────────────────────────────────
const HEADER_BG: Color32 = Color32::from_rgb(24, 24, 28);
const BORDER: Color32 = Color32::from_rgb(30, 30, 33);
const ACCENT: Color32 = Color32::from_rgb(99, 102, 241);
const ACCENT_BG: Color32 = Color32::from_rgb(33, 33, 54);
const ACCENT_FOCUS_BORDER: Color32 = Color32::from_rgb(54, 55, 120);
const ACCENT_GLOW: Color32 = Color32::from_rgba_premultiplied(99, 102, 241, 20); // ~8% alpha
const TEXT_PRIMARY: Color32 = Color32::from_rgb(240, 240, 242);
const TEXT_MUTED: Color32 = Color32::from_rgb(63, 63, 63);
const ICON_INACTIVE: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 89); // 35%
const ICON_HOVER: Color32 = Color32::from_rgba_premultiplied(240, 240, 242, 153); // 60%
const ICON_ACTIVE: Color32 = Color32::from_rgb(240, 240, 242); // full
const HOVER_BG: Color32 = Color32::from_rgb(30, 30, 33);
const SEARCH_BG: Color32 = Color32::from_rgb(28, 28, 31);
const NOTIFICATION_DOT: Color32 = Color32::from_rgb(239, 68, 68);
const AVATAR_COLOR: Color32 = Color32::from_rgb(119, 98, 243); // midpoint of indigo→violet

// ── Dimensions ─────────────────────────────────────────────────
const HEADER_HEIGHT: f32 = 44.0;
const ICON_ROUNDING: f32 = 6.0;
const SEARCH_WIDTH: f32 = 240.0;
const SEARCH_ROUNDING: f32 = 8.0;
const AVATAR_SIZE: f32 = 28.0;
const PILL_WIDTH: f32 = 12.0;
const PILL_HEIGHT: f32 = 2.0;
const DOT_RADIUS: f32 = 3.0;

// ── Icon Groups ────────────────────────────────────────────────
const CHARTS_ICONS: [&str; 3] = [
    egui_phosphor::regular::CHART_BAR,
    egui_phosphor::regular::TREND_UP,
    egui_phosphor::regular::CHART_LINE_UP,
];

const BROWSE_ICONS: [&str; 3] = [
    egui_phosphor::regular::LIST_BULLETS,
    egui_phosphor::regular::HOUSE,
    egui_phosphor::regular::NEWSPAPER,
];

pub struct TopHeader {
    pub search_query: String,
    pub active_group: usize,  // 0 = Charts, 1 = Browse
    pub active_icon: usize,   // index within the active group
}

impl Default for TopHeader {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            active_group: 0,
            active_icon: 0,
        }
    }
}

impl TopHeader {
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(HEADER_BG)
            .inner_margin(egui::Margin { left: 76, right: 16, top: 0, bottom: 0 })
            .stroke(Stroke::new(1.0, BORDER))
            .show(ui, |ui| {
                ui.set_height(HEADER_HEIGHT);
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("Zaned").color(TEXT_PRIMARY).strong().size(13.0));
                });
            });
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd /Users/user/Zaned && cargo check -p zaned-desktop`
Expected: Compiles with no errors (header shows just the brand text).

- [ ] **Step 3: Commit**

```bash
git add apps/desktop/src/components/top_header.rs
git commit -m "refactor(header): add color constants, dimensions, and updated struct with nav state"
```

---

### Task 2: Implement Brand + Dividers + Icon Groups (Left Section)

**Files:**
- Modify: `apps/desktop/src/components/top_header.rs` — the `show()` method

- [ ] **Step 1: Add a helper method `paint_divider`**

Add this method to the `impl TopHeader` block, before `show()`:

```rust
fn paint_divider(ui: &mut egui::Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, height), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, BORDER);
}
```

- [ ] **Step 2: Add a helper method `paint_icon_group`**

This method renders a group of icons with active/hover states and the group label. Add it to `impl TopHeader`:

```rust
fn paint_icon_group(
    &mut self,
    ui: &mut egui::Ui,
    icons: &[&str],
    label: &str,
    group_index: usize,
) {
    let is_active_group = self.active_group == group_index;

    for (i, icon) in icons.iter().enumerate() {
        let is_active = is_active_group && self.active_icon == i;

        let icon_color = if is_active {
            ICON_ACTIVE
        } else {
            ICON_INACTIVE
        };

        let bg_fill = if is_active { ACCENT_BG } else { Color32::TRANSPARENT };

        let btn = ui.add(
            egui::Button::new(RichText::new(*icon).size(16.0).color(icon_color))
                .fill(bg_fill)
                .corner_radius(CornerRadius::same(ICON_ROUNDING as u8))
                .min_size(Vec2::new(30.0, 30.0)),
        );

        if btn.hovered() && !is_active {
            // Repaint with hover styling by drawing over
            let rect = btn.rect;
            ui.painter().rect_filled(rect, ICON_ROUNDING, HOVER_BG);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                *icon,
                egui::FontId::proportional(16.0),
                ICON_HOVER,
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        if is_active {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            // Paint active indicator pill below the icon
            let pill_rect = Rect::from_center_size(
                Pos2::new(btn.rect.center().x, btn.rect.bottom() - 1.0),
                Vec2::new(PILL_WIDTH, PILL_HEIGHT),
            );
            ui.painter().rect_filled(pill_rect, 1.0, ACCENT);
        }

        if btn.clicked() {
            self.active_group = group_index;
            self.active_icon = i;
        }
    }

    ui.add_space(6.0);
    ui.label(RichText::new(label).color(TEXT_MUTED).size(10.0));
}
```

- [ ] **Step 3: Update `show()` to render brand + groups**

Replace the `show()` method body (inside the `horizontal_centered` closure):

```rust
pub fn show(&mut self, ui: &mut egui::Ui) {
    egui::Frame::new()
        .fill(HEADER_BG)
        .inner_margin(egui::Margin { left: 76, right: 16, top: 0, bottom: 0 })
        .stroke(Stroke::new(1.0, BORDER))
        .show(ui, |ui| {
            ui.set_height(HEADER_HEIGHT);

            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;

                // ── Brand ──
                ui.label(RichText::new("Zaned").color(TEXT_PRIMARY).strong().size(13.0));
                ui.add_space(16.0);
                Self::paint_divider(ui, 18.0);
                ui.add_space(14.0);

                // ── Charts Group ──
                self.paint_icon_group(ui, &CHARTS_ICONS, "Charts", 0);
                ui.add_space(14.0);
                Self::paint_divider(ui, 14.0);
                ui.add_space(14.0);

                // ── Browse Group ──
                self.paint_icon_group(ui, &BROWSE_ICONS, "Browse", 1);

                // Right section placeholder
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(RichText::new("…").color(TEXT_MUTED));
                });
            });
        });
}
```

- [ ] **Step 4: Verify it compiles and renders**

Run: `cd /Users/user/Zaned && cargo check -p zaned-desktop`
Expected: Compiles. Header shows Brand | divider | Charts icons + label | divider | Browse icons + label.

- [ ] **Step 5: Commit**

```bash
git add apps/desktop/src/components/top_header.rs
git commit -m "feat(header): add grouped icon navigation with active/hover states and dividers"
```

---

### Task 3: Implement Search Bar (Right Section)

**Files:**
- Modify: `apps/desktop/src/components/top_header.rs` — add `paint_search` method, update `show()`

- [ ] **Step 1: Add `paint_search` helper method**

```rust
fn paint_search(&mut self, ui: &mut egui::Ui) {
    let search_response = ui.scope(|ui| {
        // Determine if search is focused by checking the text edit after rendering
        let fill = SEARCH_BG;
        let stroke_color = BORDER;

        let search_frame = egui::Frame::new()
            .fill(fill)
            .corner_radius(CornerRadius::same(SEARCH_ROUNDING as u8))
            .stroke(Stroke::new(1.0, stroke_color))
            .inner_margin(egui::Margin::symmetric(12, 5));

        let frame_response = search_frame.show(ui, |ui| {
            ui.set_width(SEARCH_WIDTH);
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(
                    RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                        .size(14.0)
                        .color(ICON_INACTIVE),
                );
                let te = egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text(RichText::new("Search symbols...").color(TEXT_MUTED))
                    .desired_width(170.0)
                    .text_color(TEXT_PRIMARY);
                let te_response = ui.add(te);

                // Show ⌘K badge only when not focused
                if !te_response.has_focus() {
                    ui.label(
                        RichText::new("⌘K").color(TEXT_MUTED).size(10.0)
                    );
                }

                te_response
            })
            .inner
        });

        frame_response
    });

    // Paint focus ring if the text edit has focus
    let inner_response = search_response.response;
    let frame_rect = inner_response.rect;

    // Check if any child has focus (the TextEdit)
    let has_focus = ui.memory(|mem| {
        mem.has_focus(inner_response.id)
    });

    // We check focus by looking at whether the search query input is focused
    // Since we can't easily get the inner response out, we'll use a simpler approach:
    // paint the glow ring based on whether the text edit area was clicked recently
    // For now, we paint it if the text edit has focus
    if self.search_query.len() > 0 || ui.input(|i| i.key_pressed(egui::Key::Escape)).not() {
        // Paint focus glow behind the search bar if focused
        // We'll detect focus via the inner text edit response in the next step
    }
}
```

Hmm — getting the focus state out of the nested closure is awkward. Let me restructure. Replace the above with this cleaner approach:

```rust
fn paint_search(&mut self, ui: &mut egui::Ui) {
    // We need to know focus state before painting the frame, but the TextEdit
    // is inside the frame. Solution: use a stable Id and check focus from memory.
    let search_id = ui.id().with("search_input");
    let is_focused = ui.memory(|mem| mem.has_focus(search_id));

    let stroke_color = if is_focused { ACCENT_FOCUS_BORDER } else { BORDER };

    // Paint glow ring behind search if focused
    if is_focused {
        let glow_rect = ui.cursor();
        // We'll paint the glow after we know the rect — see below
    }

    let search_frame = egui::Frame::new()
        .fill(SEARCH_BG)
        .corner_radius(CornerRadius::same(SEARCH_ROUNDING as u8))
        .stroke(Stroke::new(1.0, stroke_color))
        .inner_margin(egui::Margin::symmetric(12, 5));

    let frame_resp = search_frame.show(ui, |ui| {
        ui.set_width(SEARCH_WIDTH);
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.label(
                RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .size(14.0)
                    .color(if is_focused { ICON_HOVER } else { ICON_INACTIVE }),
            );
            let te = egui::TextEdit::singleline(&mut self.search_query)
                .id(search_id)
                .hint_text(RichText::new("Search symbols...").color(TEXT_MUTED))
                .desired_width(170.0)
                .text_color(TEXT_PRIMARY);
            ui.add(te);

            if !is_focused {
                let badge_frame = egui::Frame::new()
                    .fill(Color32::from_rgb(26, 26, 30))
                    .corner_radius(CornerRadius::same(4))
                    .stroke(Stroke::new(1.0, BORDER))
                    .inner_margin(egui::Margin::symmetric(6, 2));
                badge_frame.show(ui, |ui| {
                    ui.label(RichText::new("⌘K").color(TEXT_MUTED).size(10.0));
                });
            }
        });
    });

    // Paint glow ring behind search frame if focused
    if is_focused {
        let glow_rect = frame_resp.response.rect.expand(3.0);
        ui.painter().rect_filled(glow_rect, SEARCH_ROUNDING + 3.0, ACCENT_GLOW);
    }
}
```

- [ ] **Step 2: Wire `paint_search` into the right section of `show()`**

Replace the right section placeholder in `show()`:

```rust
// ── Right Section (RTL) ──
ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
    ui.spacing_mut().item_spacing.x = 8.0;

    // Avatar placeholder
    ui.add_space(0.0);

    // Bell placeholder
    ui.add_space(0.0);

    // Search
    self.paint_search(ui);
});
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/user/Zaned && cargo check -p zaned-desktop`
Expected: Compiles. Search bar appears on the right with ⌘K badge and focus ring.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/top_header.rs
git commit -m "feat(header): add styled search bar with focus ring and shortcut badge"
```

---

### Task 4: Implement Bell Icon + Notification Dot

**Files:**
- Modify: `apps/desktop/src/components/top_header.rs` — add `paint_bell` method, update right section

- [ ] **Step 1: Add `paint_bell` helper method**

```rust
fn paint_bell(ui: &mut egui::Ui, has_notification: bool) {
    let btn = ui.add(
        egui::Button::new(
            RichText::new(egui_phosphor::regular::BELL).size(16.0).color(ICON_INACTIVE),
        )
        .fill(Color32::TRANSPARENT)
        .corner_radius(CornerRadius::same(ICON_ROUNDING as u8))
        .min_size(Vec2::new(30.0, 30.0)),
    );

    if btn.hovered() {
        let rect = btn.rect;
        ui.painter().rect_filled(rect, ICON_ROUNDING, HOVER_BG);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            egui_phosphor::regular::BELL,
            egui::FontId::proportional(16.0),
            ICON_HOVER,
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    if has_notification {
        let dot_center = Pos2::new(btn.rect.right() - 7.0, btn.rect.top() + 7.0);
        // Border circle (header bg color, slightly larger)
        ui.painter().circle_filled(dot_center, DOT_RADIUS + 1.5, HEADER_BG);
        // Red notification dot
        ui.painter().circle_filled(dot_center, DOT_RADIUS, NOTIFICATION_DOT);
    }
}
```

- [ ] **Step 2: Update the right section in `show()` to include the bell**

Replace the right section:

```rust
// ── Right Section (RTL) ──
ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
    ui.spacing_mut().item_spacing.x = 8.0;

    // Avatar placeholder
    ui.add_space(0.0);

    // Bell
    Self::paint_bell(ui, true); // hardcoded notification for now

    ui.add_space(6.0);

    // Search
    self.paint_search(ui);
});
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/user/Zaned && cargo check -p zaned-desktop`
Expected: Compiles. Bell icon with red dot appears to the right of the search bar.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/top_header.rs
git commit -m "feat(header): add bell icon with notification dot indicator"
```

---

### Task 5: Implement Avatar Circle

**Files:**
- Modify: `apps/desktop/src/components/top_header.rs` — add `paint_avatar` method, update right section

- [ ] **Step 1: Add `paint_avatar` helper method**

```rust
fn paint_avatar(ui: &mut egui::Ui, initial: char) {
    let size = Vec2::splat(AVATAR_SIZE);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    let color = if response.hovered() {
        // Slightly brighter on hover
        Color32::from_rgb(130, 115, 245)
    } else {
        AVATAR_COLOR
    };

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    // Circle background
    ui.painter().circle_filled(rect.center(), AVATAR_SIZE / 2.0, color);

    // Initial letter
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initial.to_string(),
        egui::FontId::proportional(11.0),
        Color32::WHITE,
    );
}
```

- [ ] **Step 2: Update the right section in `show()` with the avatar**

Final right section:

```rust
// ── Right Section (RTL) ──
ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
    ui.spacing_mut().item_spacing.x = 8.0;

    // Avatar
    Self::paint_avatar(ui, 'J');

    ui.add_space(6.0);

    // Bell
    Self::paint_bell(ui, true);

    ui.add_space(6.0);

    // Search
    self.paint_search(ui);
});
```

- [ ] **Step 3: Verify it compiles**

Run: `cd /Users/user/Zaned && cargo check -p zaned-desktop`
Expected: Compiles. Full header renders: Brand | Charts | Browse | Search | Bell | Avatar.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/top_header.rs
git commit -m "feat(header): add avatar circle with initial and hover state"
```

---

### Task 6: Visual Polish Pass

**Files:**
- Modify: `apps/desktop/src/components/top_header.rs` — final tweaks

- [ ] **Step 1: Run the app and visually inspect**

Run: `cd /Users/user/Zaned && cargo run -p zaned-desktop`

Check:
- Header height feels right (44px)
- Brand text is clear, not too large
- Icon groups are visually distinct with the dividers
- Active icon has indigo background tint + underline pill
- Clicking a different icon switches the active state
- Hovering shows background highlight and pointer cursor
- Search bar has ⌘K badge that disappears on focus
- Search focus shows the accent-colored border
- Bell has red notification dot
- Avatar shows "J" on indigo circle
- Right section doesn't overlap left section

- [ ] **Step 2: Fix any spacing/alignment issues found during inspection**

Adjust constants or margins as needed based on visual inspection. Common fixes:
- Adjust `item_spacing.x` if icons are too tight or too loose
- Adjust `inner_margin` if content feels cramped
- Adjust icon `min_size` if tap targets are too small

- [ ] **Step 3: Verify final compile**

Run: `cd /Users/user/Zaned && cargo check -p zaned-desktop`
Expected: Compiles cleanly with no warnings.

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/components/top_header.rs
git commit -m "style(header): polish spacing, alignment, and visual details"
```
