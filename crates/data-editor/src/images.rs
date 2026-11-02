//! The optional background picture of a map and its name (`docs/plans/done/data-editing.md` sections 6 and 8): stored in `edits.map_images`
//! and `edits.map_names`, uploaded on the page or imported from a folder (`data-editor images import <folder>`). They change nothing in
//! the game, so they are not change sets.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;
use sqlx::AssertSqlSafe;

use crate::store::{Editor, refuse};

/// The largest picture the page accepts.
pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// The world rectangle a picture shows: its left edge is `x1`, right `x2`, top `z1`, bottom `z2` (a flipped axis is `x1 > x2` or `z1 > z2`).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct Rect {
    pub x1: f32,
    pub z1: f32,
    pub x2: f32,
    pub z2: f32,
}

impl Rect {
    pub fn check(&self) -> Result<()> {
        if ![self.x1, self.z1, self.x2, self.z2].iter().all(|v| v.is_finite()) {
            return refuse("the four edges must be numbers");
        }
        if self.x1 == self.x2 || self.z1 == self.z2 {
            return refuse("the picture must cover an area: x1 and x2 differ, z1 and z2 differ");
        }
        Ok(())
    }
}

/// `image/png` or `image/jpeg` by the file's first bytes.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else {
        None
    }
}

impl Editor {
    /// The picture of a map: its type and bytes.
    pub async fn map_image(&self, map: i64) -> Result<Option<(String, Vec<u8>)>> {
        Ok(sqlx::query_as(sql(format!("SELECT content_type, image FROM {}.map_images WHERE map = $1", self.s.edits)))
            .bind(map as i16)
            .fetch_optional(&self.pool)
            .await?)
    }

    async fn check_map(&self, map: i64) -> Result<i16> {
        let Ok(id) = i16::try_from(map) else { return refuse(format!("there is no map {map}")) };
        let found: Option<i16> = sqlx::query_scalar(sql(format!("SELECT id FROM {}.maps WHERE id = $1", self.s.world)))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        match found {
            Some(id) => Ok(id),
            None => refuse(format!("there is no map {map}")),
        }
    }

    /// Stores (or replaces) a map's picture; the type comes from the bytes, not from what the sender says.
    pub async fn set_map_image(&self, map: i64, bytes: &[u8], rect: Rect, source: &str) -> Result<()> {
        let id = self.check_map(map).await?;
        rect.check()?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return refuse(format!("the picture is {} bytes: the limit is {MAX_IMAGE_BYTES} (8 MB)", bytes.len()));
        }
        let Some(content_type) = sniff(bytes) else { return refuse("the picture must be a PNG or a JPEG file") };
        sqlx::query(sql(format!(
            "INSERT INTO {}.map_images (map, content_type, image, x1, z1, x2, z2, source) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (map) DO UPDATE SET content_type = EXCLUDED.content_type, image = EXCLUDED.image, x1 = EXCLUDED.x1, z1 = EXCLUDED.z1, x2 = EXCLUDED.x2, z2 = EXCLUDED.z2,
                                             source = EXCLUDED.source, added_at = now()",
            self.s.edits
        )))
        .bind(id)
        .bind(content_type)
        .bind(bytes)
        .bind(rect.x1)
        .bind(rect.z1)
        .bind(rect.x2)
        .bind(rect.z2)
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Removes a map's picture; false when it had none.
    pub async fn delete_map_image(&self, map: i64) -> Result<bool> {
        Ok(sqlx::query(sql(format!("DELETE FROM {}.map_images WHERE map = $1", self.s.edits)))
            .bind(map as i16)
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    /// Names a map for the page (an empty name removes it).
    pub async fn set_map_name(&self, map: i64, name: &str) -> Result<()> {
        let id = self.check_map(map).await?;
        let name = name.trim();
        if name.chars().count() > 60 {
            return refuse("a map name is at most 60 characters");
        }
        if name.is_empty() {
            sqlx::query(sql(format!("DELETE FROM {}.map_names WHERE map = $1", self.s.edits))).bind(id).execute(&self.pool).await?;
        } else {
            sqlx::query(sql(format!(
                "INSERT INTO {}.map_names (map, name) VALUES ($1, $2) ON CONFLICT (map) DO UPDATE SET name = EXCLUDED.name",
                self.s.edits
            )))
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// `data-editor images import <folder>`: `<map id>.png` or `.jpg` with `<map id>.json` (`{"x1", "z1", "x2", "z2"}`), and `names.json`
    /// (`{"<map id>": "name"}`). Returns what it did, one line per file; a picture that cannot be used is reported, the others still load.
    pub async fn import_images(&self, folder: &Path) -> Result<Vec<String>> {
        let mut report = Vec::new();
        let mut found: Vec<(i64, std::path::PathBuf)> = Vec::new();
        for entry in std::fs::read_dir(folder).with_context(|| format!("reading {}", folder.display()))? {
            let path = entry?.path();
            let ext = path.extension().and_then(|e| e.to_str()).map(str::to_lowercase);
            if matches!(ext.as_deref(), Some("png" | "jpg" | "jpeg"))
                && let Some(id) = path.file_stem().and_then(|s| s.to_str()).and_then(|s| s.parse::<i64>().ok())
            {
                found.push((id, path));
            }
        }
        found.sort();
        for (id, path) in found {
            let file = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_owned();
            let rect_path = path.with_extension("json");
            let rect: Result<Rect> = std::fs::read_to_string(&rect_path)
                .with_context(|| format!("{} is missing", rect_path.display()))
                .and_then(|t| serde_json::from_str(&t).with_context(|| format!("{} must hold x1, z1, x2, z2", rect_path.display())));
            let outcome = match rect {
                Ok(rect) => self.set_map_image(id, &std::fs::read(&path)?, rect, &file).await,
                Err(e) => Err(e),
            };
            match outcome {
                Ok(()) => report.push(format!("map {id}: {file} loaded")),
                Err(e) => report.push(format!("map {id}: {file} skipped: {e:#}")),
            }
        }
        let names_path = folder.join("names.json");
        if names_path.exists() {
            let names: std::collections::BTreeMap<String, String> =
                serde_json::from_str(&std::fs::read_to_string(&names_path)?).context("names.json must be an object of map id to name")?;
            let mut count = 0;
            for (id, name) in names {
                match id.parse::<i64>() {
                    Ok(id) => match self.set_map_name(id, &name).await {
                        Ok(()) => count += 1,
                        Err(e) => report.push(format!("name of map {id} skipped: {e:#}")),
                    },
                    Err(_) => report.push(format!("name {id:?} skipped: not a map id")),
                }
            }
            report.push(format!("{count} map names loaded"));
        }
        if report.is_empty() {
            report.push(format!("nothing to import in {}", folder.display()));
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_are_recognised_by_their_bytes() {
        assert_eq!(sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("image/png"));
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff(b"GIF89a"), None);
        assert_eq!(sniff(b"<svg>"), None);
    }

    #[test]
    fn a_rectangle_needs_an_area() {
        assert!(Rect { x1: 0.0, z1: 0.0, x2: 100.0, z2: 100.0 }.check().is_ok());
        assert!(Rect { x1: 100.0, z1: 100.0, x2: 0.0, z2: 0.0 }.check().is_ok(), "flipped axes are allowed");
        assert!(Rect { x1: 5.0, z1: 0.0, x2: 5.0, z2: 100.0 }.check().is_err());
        assert!(Rect { x1: f32::NAN, z1: 0.0, x2: 5.0, z2: 100.0 }.check().is_err());
    }
}
