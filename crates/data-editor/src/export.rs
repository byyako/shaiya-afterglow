//! Export and Import of all edits as one file (`docs/plans/done/data-editing.md` section 4): backup, moving edits to another server, sharing.
//! The file is written in a stable order (and its `exported_at` is the time of the newest change set), so two exports of the same edits
//! are identical.

use std::path::Path;

use anyhow::{Context, Result};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, PgConnection};

use crate::apply::{Report, apply_all};
use crate::store::{Editor, refuse};

pub const FORMAT: &str = "newshaiya-edits";
pub const VERSION: i64 = 1;

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

async fn rows_of(conn: &mut PgConnection, query: String) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar(sql(query)).fetch_all(&mut *conn).await?)
}

impl Editor {
    /// All edits as a JSON document; the map pictures only with `images`.
    pub async fn export(&self, images: bool) -> Result<Value> {
        let e = &self.s.edits;
        let mut conn = self.pool.acquire().await?;
        let c = &mut *conn;
        let change_sets = rows_of(c, format!("SELECT to_jsonb(x) FROM {e}.change_sets x ORDER BY id")).await?;
        let log = rows_of(c, format!("SELECT to_jsonb(x) FROM {e}.log x ORDER BY change_set, seq")).await?;
        let fields = rows_of(c, format!("SELECT to_jsonb(x) FROM {e}.fields x ORDER BY target, key, field")).await?;
        let rows = rows_of(c, format!("SELECT to_jsonb(x) FROM {e}.rows x ORDER BY target, key")).await?;
        let map_names = rows_of(c, format!("SELECT to_jsonb(x) FROM {e}.map_names x ORDER BY map")).await?;
        let map_images = if images {
            rows_of(
                c,
                format!(
                    "SELECT jsonb_build_object('map', map, 'content_type', content_type, 'image', translate(encode(image, 'base64'), chr(10), ''),
                            'x1', x1, 'z1', z1, 'x2', x2, 'z2', z2, 'source', source, 'added_at', added_at)
                     FROM {e}.map_images ORDER BY map"
                ),
            )
            .await?
        } else {
            Vec::new()
        };
        let exported_at = change_sets
            .iter()
            .filter_map(|c| c.get("made_at").cloned())
            .max_by_key(|v| v.as_str().unwrap_or("").to_owned())
            .unwrap_or(Value::Null);
        Ok(json!({
            "format": FORMAT, "version": VERSION, "exported_at": exported_at,
            "change_sets": change_sets, "log": log, "fields": fields, "rows": rows, "map_names": map_names, "map_images": map_images,
        }))
    }

    /// Writes the export to a file (through a temporary file next to it, so a failure leaves the old file).
    pub async fn export_to(&self, path: &Path, images: bool) -> Result<()> {
        let doc = self.export(images).await?;
        let mut text = serde_json::to_string_pretty(&doc)?;
        text.push('\n');
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, text).with_context(|| format!("writing {}", temp.display()))?;
        std::fs::rename(&temp, path).with_context(|| format!("replacing {}", path.display()))?;
        Ok(())
    }

    /// Import (section 4): into empty edits tables, or with `replace` after reverting every edit in force. The schemas count as fresh
    /// afterwards, so `apply` runs at once and refreshes the originals from this server's tables.
    pub async fn import(&self, doc: &Value, replace: bool) -> Result<Report> {
        if doc.get("format").and_then(Value::as_str) != Some(FORMAT) {
            return refuse(format!("not an edits file (its \"format\" is not {FORMAT:?})"));
        }
        let version = doc.get("version").and_then(Value::as_i64).unwrap_or(0);
        if version != VERSION {
            return refuse(format!("the edits file is version {version}; this program reads version {VERSION}"));
        }
        let e = &self.s.edits;
        let part = |name: &str| -> Value { doc.get(name).cloned().unwrap_or_else(|| Value::Array(Vec::new())) };
        let mut tx = self.begin_locked().await?;
        let present: i64 = sqlx::query_scalar(sql(format!(
            "SELECT (SELECT count(*) FROM {e}.change_sets) + (SELECT count(*) FROM {e}.fields) + (SELECT count(*) FROM {e}.rows)"
        )))
        .fetch_one(&mut *tx)
        .await?;
        if present > 0 {
            if !replace {
                return refuse("the edits tables are not empty: import with --replace to revert and replace the edits in force");
            }
            self.revert_all(&mut tx).await?;
        }
        for table in ["draft", "log", "fields", "rows", "change_sets", "map_names"] {
            sqlx::query(sql(format!("DELETE FROM {e}.{table}"))).execute(&mut *tx).await?;
        }
        let images = part("map_images");
        if images.as_array().is_some_and(|a| !a.is_empty()) {
            sqlx::query(sql(format!("DELETE FROM {e}.map_images"))).execute(&mut *tx).await?;
        }
        for table in ["change_sets", "fields", "rows", "log", "map_names"] {
            sqlx::query(sql(format!("INSERT INTO {e}.{table} SELECT * FROM jsonb_populate_recordset(NULL::{e}.{table}, $1::jsonb)")))
                .bind(part(table))
                .execute(&mut *tx)
                .await
                .with_context(|| format!("importing {table}"))?;
        }
        sqlx::query(sql(format!(
            "SELECT setval(pg_get_serial_sequence('{e}.change_sets', 'id'), GREATEST((SELECT COALESCE(max(id), 1) FROM {e}.change_sets), 1), (SELECT max(id) IS NOT NULL FROM {e}.change_sets))"
        )))
        .execute(&mut *tx)
        .await?;
        for image in images.as_array().into_iter().flatten() {
            let text = |k: &str| image.get(k).and_then(Value::as_str).unwrap_or("");
            let real = |k: &str| image.get(k).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            let bytes = STANDARD.decode(text("image")).context("a map picture is not base64")?;
            sqlx::query(sql(format!(
                "INSERT INTO {e}.map_images (map, content_type, image, x1, z1, x2, z2, source, added_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE(NULLIF($9, '')::timestamptz, now()))"
            )))
            .bind(image.get("map").and_then(Value::as_i64).unwrap_or(0) as i16)
            .bind(text("content_type"))
            .bind(bytes)
            .bind(real("x1"))
            .bind(real("z1"))
            .bind(real("x2"))
            .bind(real("z2"))
            .bind(text("source"))
            .bind(text("added_at"))
            .execute(&mut *tx)
            .await?;
        }
        for schema in [&self.s.defs, &self.s.world] {
            sqlx::query(sql(format!("DROP TABLE IF EXISTS {schema}.edits_applied"))).execute(&mut *tx).await?;
        }
        let report = apply_all(&mut tx, &self.s).await?;
        tx.commit().await?;
        Ok(report)
    }

    /// Reads an export file.
    pub fn read_export(path: &Path) -> Result<Value> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("{} is not JSON", path.display()))
    }
}
