# Top Header Styling Design

**Date:** 2026-03-31
**Component:** `apps/desktop/src/components/top_header.rs`
**Style:** Modern Fintech (Linear/Arc/Raycast aesthetic)

---

## Overview

Restyle the top header from a flat icon row into a grouped, polished navigation bar with an indigo accent, refined hover/active states, and premium spacing. The header is the primary navigation surface — users switch views frequently.

## Layout (left to right)

```
[76px macOS traffic light zone] Brand | div | [Charts group + label] | div | [Browse group + label] | ←spacer→ | Search bar | Bell | Avatar
```

- **Brand**: "Zaned" — white, 13px, semibold, -0.2px letter spacing
- **Dividers**: Vertical lines, `rgba(255,255,255,0.06)`, 18px tall (brand) / 14px tall (groups)
- **Icon groups**: Clustered icons with a muted category label to the right
- **Spacer**: Flexible space pushes right section to the edge
- **Right section** (RTL order): Avatar, Bell, Search bar

## Icon Groups

Two groups, each with a small label:

### Charts group
| Icon | Phosphor name | Purpose |
|------|--------------|---------|
| Bar chart | `CHART_BAR` | Dashboard/overview |
| Trend up | `TREND_UP` | Trends view |
| Line chart | `CHART_LINE_UP` | Detailed charting |

### Browse group
| Icon | Phosphor name | Purpose |
|------|--------------|---------|
| List | `LIST_BULLETS` | Watchlists |
| House | `HOUSE` | Home/portfolio |
| Newspaper | `NEWSPAPER` | News feed |

Labels: muted white (25% opacity), 10px, font-weight 500. Positioned to the right of the icon cluster.

## Colors

| Token | Value | Usage |
|-------|-------|-------|
| `header_bg` | `#18181C` / `rgb(24, 24, 28)` | Header background fill |
| `border` | `rgba(255, 255, 255, 0.06)` / ~`rgb(30, 30, 33)` | All borders and dividers |
| `accent` | `#6366F1` / `rgb(99, 102, 241)` | Active indicator, focus ring |
| `accent_bg` | `rgba(99, 102, 241, 0.12)` / ~`rgb(33, 33, 54)` | Active icon background tint |
| `accent_focus_border` | `rgba(99, 102, 241, 0.4)` / ~`rgb(54, 55, 120)` | Search focus border |
| `text_primary` | `#F0F0F2` / `rgb(240, 240, 242)` | Brand text |
| `text_muted` | `rgba(255, 255, 255, 0.25)` / ~`rgb(63, 63, 63)` | Group labels, inactive icons |
| `icon_inactive` | 35% opacity on icon color | Inactive nav icons |
| `icon_hover` | 60% opacity on icon color | Hovered nav icons |
| `hover_bg` | `rgba(255, 255, 255, 0.06)` / ~`rgb(30, 30, 33)` | Hover background on icons |
| `search_bg` | `rgba(255, 255, 255, 0.04)` / ~`rgb(28, 28, 31)` | Search bar fill |
| `notification_dot` | `#EF4444` / `rgb(239, 68, 68)` | Bell notification indicator |
| `avatar_gradient_start` | `#6366F1` | Avatar gradient (top-left) |
| `avatar_gradient_end` | `#8B5CF6` | Avatar gradient (bottom-right) |

**Note on rgba in egui:** egui uses `Color32` which is RGBA. For the semi-transparent overlays, pre-compute the blended color against the header background (`#18181C`). The table includes approximate pre-blended RGB values as comments.

## Dimensions

| Element | Value |
|---------|-------|
| Header height | 44px |
| Left margin | 76px (macOS traffic light clearance) |
| Right margin | 16px |
| Top/bottom padding | Centered vertically in 44px |
| Icon tap target padding | 6px vertical, 8px horizontal |
| Icon corner radius | 6px |
| Active indicator pill | 12px wide, 2px tall, 1px corner radius, centered under icon |
| Search bar width | 240px |
| Search bar padding | 5px vertical, 12px horizontal |
| Search bar corner radius | 8px |
| Search text input width | ~180px |
| Avatar diameter | 28px (fully round) |
| Notification dot | 6px diameter, 1.5px border matching header bg |
| Brand-to-divider gap | 16px |
| Divider-to-group gap | 14px |
| Icon gap within group | 1px (tight clustering) |
| Group icons to label gap | 6px |
| Group label to divider gap | 14px |
| Search to bell gap | 14px |
| Bell to avatar gap | 6px |
| Item spacing (right section) | 8px default |

## Interaction States

### Nav Icons
- **Default**: Icon at 35% opacity, transparent background
- **Hover**: Background `hover_bg` (white 6%), icon opacity 60%, cursor pointer
- **Active**: Background `accent_bg` (indigo 12%), icon full opacity, indigo pill underline (12x2px centered at bottom)

### Search Bar
- **Default**: `search_bg` fill, `border` stroke, placeholder "Search symbols..." in muted text, magnifying glass icon, ⌘K shortcut badge
- **Focused**: Border changes to `accent_focus_border` (indigo 40%), subtle glow ring (3px spread, indigo 8% opacity), placeholder replaced by cursor
- **⌘K badge**: Muted text on slightly darker background pill, hidden when focused

### Bell Icon
- **Default**: Same as nav icon inactive (35% opacity)
- **Hover**: Same as nav icon hover
- **Notification dot**: 6px red circle at top-right, with 1.5px header-bg border for separation

### Avatar
- **Default**: 28px circle with indigo→violet gradient, white initial "J" centered (11px, semibold)
- **Hover**: Subtle brightness increase, cursor pointer

## Implementation Notes

### egui-specific considerations
- Use `egui::Frame` for the header container with `fill`, `inner_margin`, and `stroke`
- Use `ui.scope()` for local style overrides on hover states
- For the active indicator pill: paint a small `Rect` using `ui.painter().rect_filled()` positioned below the active icon
- For the avatar gradient: egui doesn't natively support gradients on circles. Use a pre-rendered gradient texture, or approximate with the midpoint color `#7762F3` as a solid fill. Gradient is a nice-to-have, solid indigo is acceptable.
- For notification dot border: paint a filled circle in header bg color first (8px), then paint the red circle on top (6px) to simulate a border
- Icon opacity: apply via `Color32::from_rgba_premultiplied` or by adjusting the icon color's alpha channel
- Group labels: use `RichText` with small size and reduced alpha
- Search focus glow: egui doesn't have box-shadow. Approximate by painting a slightly larger rounded rect behind the search frame in `accent` at very low alpha before painting the search frame itself

### What changes from current code
- Header height: 36px → 44px
- Nav icons: flat row of 8 → two groups of 3 with labels
- Icons removed from header: `BELL` moves to right section, `DOTS_THREE_VERTICAL` removed entirely
- Icon styling: uniform color → opacity-based active/inactive/hover states with background
- Active state: none currently → indigo tinted background + underline pill
- Border color: hard-coded `rgb(50,50,55)` → softer `rgba(255,255,255,0.06)` equivalent
- Search bar: basic TextEdit → styled with focus ring behavior
- Account section: text label → gradient avatar circle with initial
- Brand text: pure white → slightly warm white `#F0F0F2`

### State to track
- `active_group: usize` — which icon group is selected (0 = Charts, 1 = Browse)
- `active_icon: usize` — which icon within the group is active
- `search_focused: bool` — whether search has focus (for ring styling)
- Existing `search_query: String` — stays as-is

## Out of Scope
- Dropdown menus on avatar click
- Search autocomplete/suggestions
- Icon tooltips (can be added later)
- Keyboard navigation between icons
- Animation/transitions (egui doesn't support CSS-like transitions natively)
