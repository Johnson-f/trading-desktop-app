use eframe::egui::{Color32, Painter, Pos2, Rect, Stroke};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use zaned_chart_core::Rgba;

pub const DEFAULT_COLOR: Rgba = Rgba([255, 255, 255, 255]);

/// Global default style that can be configured by the user.
static USER_DEFAULT_STYLE: RwLock<Option<DrawingStyle>> = RwLock::new(None);

/// Global database pool for async operations
static DB_POOL: RwLock<Option<sqlx::SqlitePool>> = RwLock::new(None);

/// Global runtime handle for async operations
static RUNTIME_HANDLE: RwLock<Option<tokio::runtime::Handle>> = RwLock::new(None);

/// Visual style for a committed drawing. Preview rendering does NOT read this
/// — drafts use hard-coded faded constants because they have no committed
/// style yet.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct DrawingStyle {
    pub color: Rgba,
    pub width: f32,
    pub dash: DashStyle,
    pub opacity: f32,
    pub extend_left: bool,
    pub extend_right: bool,
}

impl Default for DrawingStyle {
    fn default() -> Self {
        // Check if user has set a custom default
        if let Ok(guard) = USER_DEFAULT_STYLE.read() {
            if let Some(user_default) = *guard {
                return user_default;
            }
        }

        // Fall back to system default
        Self {
            color: DEFAULT_COLOR,
            width: 1.5,
            dash: DashStyle::Solid,
            opacity: 1.0,
            extend_left: false,
            extend_right: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DashStyle {
    Solid,
    Dashed,
    Dotted,
}

/// Width presets surfaced in the toolbar width popup.
pub const WIDTH_PRESETS: &[f32] = &[1.0, 1.5, 2.5];

/// Color palette surfaced in the toolbar color popup. Ordered warm → cool.
pub const COLOR_PALETTE: &[Rgba] = &[
    Rgba([255, 255, 255, 255]), // white (default)
    Rgba([255, 193, 7, 255]),   // amber
    Rgba([255, 107, 107, 255]), // red
    Rgba([236, 72, 153, 255]),  // pink
    Rgba([168, 85, 247, 255]),  // purple
    Rgba([88, 166, 255, 255]),  // blue
    Rgba([34, 211, 238, 255]),  // cyan
    Rgba([78, 205, 196, 255]),  // teal
];

/// Convert a chart-core `Rgba` into an `egui::Color32`, scaling alpha by
/// `opacity` (clamped to 0.0..=1.0). egui premultiplies internally.
pub fn style_color(color: Rgba, opacity: f32) -> Color32 {
    let [r, g, b, a] = color.0;
    let a_scaled = ((a as f32) * opacity.clamp(0.0, 1.0)) as u8;
    Color32::from_rgba_unmultiplied(r, g, b, a_scaled)
}

/// Draw a line segment honoring `DrawingStyle::dash`.
pub fn paint_line(painter: &Painter, a: Pos2, b: Pos2, style: &DrawingStyle) {
    let stroke = Stroke::new(style.width, style_color(style.color, style.opacity));
    match style.dash {
        DashStyle::Solid => {
            painter.line_segment([a, b], stroke);
        }
        DashStyle::Dashed => paint_dashed_segment(painter, a, b, stroke, 8.0, 4.0),
        DashStyle::Dotted => paint_dashed_segment(painter, a, b, stroke, 2.0, 3.0),
    }
}

/// Draw a horizontal line spanning `rect.x_range()` at screen-space `y`,
/// honoring dash style.
pub fn paint_hline(painter: &Painter, rect: Rect, y: f32, style: &DrawingStyle) {
    paint_line(
        painter,
        Pos2::new(rect.left(), y),
        Pos2::new(rect.right(), y),
        style,
    );
}

/// Draw a vertical line spanning `rect.y_range()` at screen-space `x`,
/// honoring dash style.
pub fn paint_vline(painter: &Painter, rect: Rect, x: f32, style: &DrawingStyle) {
    paint_line(
        painter,
        Pos2::new(x, rect.top()),
        Pos2::new(x, rect.bottom()),
        style,
    );
}

fn paint_dashed_segment(
    painter: &Painter,
    a: Pos2,
    b: Pos2,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let total = (dx * dx + dy * dy).sqrt();
    if total < 1e-3 {
        return;
    }
    let period = dash_len + gap_len;
    let ux = dx / total;
    let uy = dy / total;
    let mut t = 0.0_f32;
    while t < total {
        let start = Pos2::new(a.x + ux * t, a.y + uy * t);
        let end_t = (t + dash_len).min(total);
        let end = Pos2::new(a.x + ux * end_t, a.y + uy * end_t);
        painter.line_segment([start, end], stroke);
        t += period;
    }
}

/// Initialize the database pool and runtime handle for drawing defaults
pub fn init_with_database(pool: sqlx::SqlitePool, runtime_handle: tokio::runtime::Handle) {
    // Store pool and runtime handle
    if let Ok(mut guard) = DB_POOL.write() {
        *guard = Some(pool.clone());
    }
    if let Ok(mut guard) = RUNTIME_HANDLE.write() {
        *guard = Some(runtime_handle.clone());
    }

    // Share with the per-symbol drawings persistence layer.
    super::persistence::init(pool.clone(), runtime_handle.clone());

    // Load existing defaults from database
    runtime_handle.spawn(async move {
        match load_from_database(&pool).await {
            Ok(Some(style)) => {
                if let Ok(mut guard) = USER_DEFAULT_STYLE.write() {
                    *guard = Some(style);
                    println!("✓ Loaded drawing defaults from database");
                }
            }
            Ok(None) => {
                println!("No drawing defaults found in database, using system defaults");
            }
            Err(e) => {
                eprintln!("Failed to load drawing defaults from database: {}", e);
            }
        }
    });
}

/// Set the user's default drawing style. This will be persisted to the database.
pub fn set_user_default(style: DrawingStyle) -> Result<(), String> {
    // Update in-memory default
    if let Ok(mut guard) = USER_DEFAULT_STYLE.write() {
        *guard = Some(style);
    } else {
        return Err("Failed to acquire write lock".to_string());
    }

    // Persist to database asynchronously
    if let (Ok(pool_guard), Ok(handle_guard)) = (DB_POOL.read(), RUNTIME_HANDLE.read()) {
        if let (Some(pool), Some(handle)) = (pool_guard.as_ref(), handle_guard.as_ref()) {
            let pool_clone = pool.clone();
            handle.spawn(async move {
                if let Err(e) = save_drawing_defaults(&pool_clone, style).await {
                    eprintln!("Failed to save drawing defaults to database: {}", e);
                }
            });
        }
    }

    Ok(())
}

async fn load_from_database(pool: &sqlx::SqlitePool) -> Result<Option<DrawingStyle>, String> {
    let row: Option<(i64, i64, i64, i64, f64, String, f64, i64, i64)> = sqlx::query_as(
        "SELECT color_r, color_g, color_b, color_a, width, dash, opacity, extend_left, extend_right 
         FROM drawing_defaults WHERE id = 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Database query failed: {}", e))?;

    if let Some((r, g, b, a, width, dash_str, opacity, extend_left, extend_right)) = row {
        let dash = match dash_str.as_str() {
            "Solid" => DashStyle::Solid,
            "Dashed" => DashStyle::Dashed,
            "Dotted" => DashStyle::Dotted,
            _ => DashStyle::Solid,
        };

        Ok(Some(DrawingStyle {
            color: Rgba([r as u8, g as u8, b as u8, a as u8]),
            width: width as f32,
            dash,
            opacity: opacity as f32,
            extend_left: extend_left != 0,
            extend_right: extend_right != 0,
        }))
    } else {
        Ok(None)
    }
}

/// Save drawing defaults to database
async fn save_drawing_defaults(pool: &sqlx::SqlitePool, style: DrawingStyle) -> Result<(), String> {
    let dash_str = match style.dash {
        DashStyle::Solid => "Solid",
        DashStyle::Dashed => "Dashed",
        DashStyle::Dotted => "Dotted",
    };

    sqlx::query(
        "INSERT INTO drawing_defaults (id, color_r, color_g, color_b, color_a, width, dash, opacity, extend_left, extend_right, updated_at)
         VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
         ON CONFLICT(id) DO UPDATE SET
            color_r = excluded.color_r,
            color_g = excluded.color_g,
            color_b = excluded.color_b,
            color_a = excluded.color_a,
            width = excluded.width,
            dash = excluded.dash,
            opacity = excluded.opacity,
            extend_left = excluded.extend_left,
            extend_right = excluded.extend_right,
            updated_at = CURRENT_TIMESTAMP"
    )
    .bind(style.color.0[0] as i64)
    .bind(style.color.0[1] as i64)
    .bind(style.color.0[2] as i64)
    .bind(style.color.0[3] as i64)
    .bind(style.width as f64)
    .bind(dash_str)
    .bind(style.opacity as f64)
    .bind(if style.extend_left { 1 } else { 0 })
    .bind(if style.extend_right { 1 } else { 0 })
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to save to database: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_style_serde_roundtrip() {
        let original = DrawingStyle {
            color: Rgba([10, 20, 30, 255]),
            width: 2.5,
            dash: DashStyle::Dashed,
            opacity: 0.75,
            extend_left: true,
            extend_right: false,
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: DrawingStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }
}
