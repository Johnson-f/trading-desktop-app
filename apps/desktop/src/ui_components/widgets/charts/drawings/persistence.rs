//! Per-symbol drawing persistence.
//!
//! Uses the global `DB_POOL` and `RUNTIME_HANDLE` already initialized by
//! `style::init_with_database` so the chart widget can fire-and-forget saves
//! without needing access to the pool itself.
//!
//! Storage strategy: full-set replacement per symbol. On every mutation the
//! caller hands us the entire `committed` Vec; we delete the symbol's existing
//! rows and re-insert. Simpler than tracking per-row dirty state — the table is
//! tiny (low hundreds of rows even for power users) and writes are async.
//!
//! `def_id: &'static str` round-trips via the registry: persisted as `String`,
//! resolved back to `&'static str` on load via `registry::get`. Drawings whose
//! `def_id` is no longer registered are dropped on load with a warning rather
//! than failing the whole load.
//!
//! Async work goes through the runtime handle stored in `style.rs`. The
//! ChartWidget owns oneshot receivers for in-flight loads and polls them each
//! frame — see `ChartWidget::poll_pending_drawings_load`.

use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::oneshot;

use super::kind_style::KindStyle;
use super::registry;
use super::style::DrawingStyle;
use super::trait_def::{CommittedDrawing, WorldPoint};

/// Globals mirror the ones in `style.rs`. We can't easily share without a
/// mutual-import dance, so we re-declare and re-initialize from the same
/// `init_with_database` call site.
static DB_POOL: RwLock<Option<SqlitePool>> = RwLock::new(None);
static RUNTIME_HANDLE: RwLock<Option<tokio::runtime::Handle>> = RwLock::new(None);

/// Called from `style::init_with_database` so the persistence layer shares the
/// same pool and runtime handle as the defaults layer.
pub fn init(pool: SqlitePool, handle: tokio::runtime::Handle) {
    if let Ok(mut g) = DB_POOL.write() {
        *g = Some(pool);
    }
    if let Ok(mut g) = RUNTIME_HANDLE.write() {
        *g = Some(handle);
    }
}

/// DTO for serde. `def_id` is `String` so we can deserialize; the in-memory
/// `CommittedDrawing` uses `&'static str` resolved via registry on load.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedDrawing {
    id: u64,
    def_id: String,
    points: Vec<WorldPoint>,
    point_dates: Vec<String>,
    style: DrawingStyle,
    kind_style: KindStyle,
    locked: bool,
}

impl PersistedDrawing {
    fn from_committed(c: &CommittedDrawing) -> Self {
        Self {
            id: c.id,
            def_id: c.def_id.to_string(),
            points: c.points.clone(),
            point_dates: c.point_dates.clone(),
            style: c.style,
            kind_style: c.kind_style,
            locked: c.locked,
        }
    }

    fn into_committed(self) -> Option<CommittedDrawing> {
        let def = registry::get(&self.def_id)?;
        Some(CommittedDrawing {
            id: self.id,
            def_id: def.id,
            points: self.points,
            point_dates: self.point_dates,
            style: self.style,
            kind_style: self.kind_style,
            locked: self.locked,
        })
    }
}

/// Replace all persisted drawings for `symbol` with the given set. Async
/// fire-and-forget — failures are logged, never bubble up to the UI.
pub fn save_all(symbol: String, drawings: Vec<CommittedDrawing>) {
    let (Ok(pool_guard), Ok(handle_guard)) = (DB_POOL.read(), RUNTIME_HANDLE.read()) else {
        return;
    };
    let (Some(pool), Some(handle)) = (pool_guard.as_ref().cloned(), handle_guard.as_ref().cloned())
    else {
        return;
    };
    let payload: Vec<PersistedDrawing> = drawings
        .iter()
        .map(PersistedDrawing::from_committed)
        .collect();
    handle.spawn(async move {
        if let Err(e) = save_all_async(&pool, &symbol, &payload).await {
            eprintln!("save drawings for {symbol}: {e}");
        }
    });
}

/// Kick off an async load for `symbol`. The caller polls the returned
/// receiver each frame; once the future resolves it gets back the committed
/// drawings ready to swap into `DrawingsManager`.
pub fn load_async(symbol: String) -> oneshot::Receiver<Vec<CommittedDrawing>> {
    let (tx, rx) = oneshot::channel();
    let (Ok(pool_guard), Ok(handle_guard)) = (DB_POOL.read(), RUNTIME_HANDLE.read()) else {
        let _ = tx.send(Vec::new());
        return rx;
    };
    let (Some(pool), Some(handle)) = (pool_guard.as_ref().cloned(), handle_guard.as_ref().cloned())
    else {
        let _ = tx.send(Vec::new());
        return rx;
    };
    handle.spawn(async move {
        let result = match load_all_async(&pool, &symbol).await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("load drawings for {symbol}: {e}");
                Vec::new()
            }
        };
        let _ = tx.send(result);
    });
    rx
}

/// Delete every persisted drawing for `symbol`. Used by the trash icon's
/// "clear all" action.
pub fn delete_all(symbol: String) {
    let (Ok(pool_guard), Ok(handle_guard)) = (DB_POOL.read(), RUNTIME_HANDLE.read()) else {
        return;
    };
    let (Some(pool), Some(handle)) = (pool_guard.as_ref().cloned(), handle_guard.as_ref().cloned())
    else {
        return;
    };
    handle.spawn(async move {
        if let Err(e) = sqlx::query("DELETE FROM drawings WHERE symbol = ?")
            .bind(&symbol)
            .execute(&pool)
            .await
        {
            eprintln!("delete drawings for {symbol}: {e}");
        }
    });
}

async fn save_all_async(
    pool: &SqlitePool,
    symbol: &str,
    drawings: &[PersistedDrawing],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM drawings WHERE symbol = ?")
        .bind(symbol)
        .execute(&mut *tx)
        .await?;
    for d in drawings {
        let points_json = serde_json::to_string(&d.points).unwrap_or_else(|_| "[]".into());
        let point_dates_json =
            serde_json::to_string(&d.point_dates).unwrap_or_else(|_| "[]".into());
        let style_json = serde_json::to_string(&d.style).unwrap_or_else(|_| "{}".into());
        let kind_style_json =
            serde_json::to_string(&d.kind_style).unwrap_or_else(|_| "null".into());
        sqlx::query(
            "INSERT INTO drawings
             (symbol, drawing_id, def_id, points_json, point_dates_json,
              style_json, kind_style_json, locked, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        )
        .bind(symbol)
        .bind(d.id as i64)
        .bind(&d.def_id)
        .bind(points_json)
        .bind(point_dates_json)
        .bind(style_json)
        .bind(kind_style_json)
        .bind(if d.locked { 1 } else { 0 })
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

async fn load_all_async(
    pool: &SqlitePool,
    symbol: &str,
) -> Result<Vec<CommittedDrawing>, sqlx::Error> {
    let rows: Vec<(i64, String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT drawing_id, def_id, points_json, point_dates_json,
                style_json, kind_style_json, locked
         FROM drawings WHERE symbol = ? ORDER BY drawing_id ASC",
    )
    .bind(symbol)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for (id, def_id, points_json, point_dates_json, style_json, kind_style_json, locked) in rows {
        let points: Vec<WorldPoint> = serde_json::from_str(&points_json).unwrap_or_default();
        let point_dates: Vec<String> = serde_json::from_str(&point_dates_json).unwrap_or_default();
        let style: DrawingStyle = serde_json::from_str(&style_json).unwrap_or_default();
        let kind_style: KindStyle =
            serde_json::from_str(&kind_style_json).unwrap_or(KindStyle::None);
        let dto = PersistedDrawing {
            id: id as u64,
            def_id,
            points,
            point_dates,
            style,
            kind_style,
            locked: locked != 0,
        };
        match dto.into_committed() {
            Some(c) => out.push(c),
            None => eprintln!("dropping drawing {id} for {symbol}: def_id no longer registered"),
        }
    }
    Ok(out)
}
