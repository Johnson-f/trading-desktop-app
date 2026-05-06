use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(rename_all = "PascalCase")]
pub enum DrawingDash {
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, FromRow)]
pub struct DrawingDefaults {
    pub color_r: i64,
    pub color_g: i64,
    pub color_b: i64,
    pub color_a: i64,
    pub width: f64,
    pub dash: DrawingDash,
    pub opacity: f64,
    pub extend_left: bool,
    pub extend_right: bool,
}

/// Fetch the singleton drawing defaults row, if it has been set.
pub async fn get(pool: &SqlitePool) -> Result<Option<DrawingDefaults>> {
    let row = sqlx::query_as::<_, DrawingDefaults>(
        r#"
        SELECT color_r, color_g, color_b, color_a,
               width, dash, opacity,
               extend_left, extend_right
        FROM drawing_defaults
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Insert or replace the singleton drawing defaults row.
pub async fn upsert(pool: &SqlitePool, defaults: &DrawingDefaults) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO drawing_defaults (
            id, color_r, color_g, color_b, color_a,
            width, dash, opacity,
            extend_left, extend_right, updated_at
        )
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
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(defaults.color_r)
    .bind(defaults.color_g)
    .bind(defaults.color_b)
    .bind(defaults.color_a)
    .bind(defaults.width)
    .bind(defaults.dash)
    .bind(defaults.opacity)
    .bind(defaults.extend_left)
    .bind(defaults.extend_right)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove the singleton drawing defaults row.
pub async fn delete(pool: &SqlitePool) -> Result<()> {
    sqlx::query("DELETE FROM drawing_defaults WHERE id = 1")
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn fresh_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        schema::migrate(&pool).await.unwrap();
        pool
    }

    fn sample() -> DrawingDefaults {
        DrawingDefaults {
            color_r: 255,
            color_g: 128,
            color_b: 0,
            color_a: 255,
            width: 1.5,
            dash: DrawingDash::Dashed,
            opacity: 0.8,
            extend_left: false,
            extend_right: true,
        }
    }

    #[tokio::test]
    async fn get_returns_none_when_unset() {
        let pool = fresh_pool().await;
        assert!(get(&pool).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn upsert_then_get_roundtrips() {
        let pool = fresh_pool().await;
        let defaults = sample();
        upsert(&pool, &defaults).await.unwrap();
        let fetched = get(&pool).await.unwrap().unwrap();
        assert_eq!(fetched, defaults);
    }

    #[tokio::test]
    async fn upsert_overwrites_existing_row() {
        let pool = fresh_pool().await;
        upsert(&pool, &sample()).await.unwrap();

        let updated = DrawingDefaults {
            color_r: 0,
            color_g: 0,
            color_b: 255,
            dash: DrawingDash::Dotted,
            ..sample()
        };
        upsert(&pool, &updated).await.unwrap();

        let fetched = get(&pool).await.unwrap().unwrap();
        assert_eq!(fetched, updated);

        // Still exactly one row — the id=1 CHECK + ON CONFLICT guarantees singleton.
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM drawing_defaults")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn delete_removes_row() {
        let pool = fresh_pool().await;
        upsert(&pool, &sample()).await.unwrap();
        delete(&pool).await.unwrap();
        assert!(get(&pool).await.unwrap().is_none());
    }
}
