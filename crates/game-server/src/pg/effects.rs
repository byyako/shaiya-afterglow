//! The item effects and skill abilities of the enabled extensions (`docs/plans/ext-3-effects.md` section 3.4): read at start from
//! `content.ext_item_effects` and `content.ext_skill_abilities`, parsed by the same parser the data editor checked them with.

use anyhow::{Context, Result, bail};

use super::*;
use crate::effects::{self, Abilities, ItemEffects};
use crate::skills::SkillDefs;

/// The effects of enabled extensions, by item id. An effect whose item is not in `defs.items`, one of a type this server does not
/// know (a newer data editor) or one that does not parse stops the start, naming the extension and the item. An effect that no longer
/// suits its item's row (an edit changed the item's type or special after the install) is skipped with a warning: the item then does
/// what its own columns say.
pub async fn load_item_effects(pool: &PgPool, items: &ItemDefs) -> Result<ItemEffects> {
    load_item_effects_in(pool, items, "content").await
}

/// As [`load_item_effects`] with the `content` schema named (the data editor's tests keep theirs apart; the name is the caller's own,
/// never a user's). Empty before migration 0034.
pub async fn load_item_effects_in(pool: &PgPool, items: &ItemDefs, content: &str) -> Result<ItemEffects> {
    use sqlx::AssertSqlSafe;
    if !present(pool, content, "ext_item_effects").await? {
        return Ok(ItemEffects::default());
    }
    let rows: Vec<(String, i32, String)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT f.extension, f.item_id, f.effect::text FROM {content}.ext_item_effects f JOIN {content}.extensions e ON e.name = f.extension
          WHERE e.enabled ORDER BY f.item_id"
    )))
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for (extension, item_id, text) in rows {
        let (item_type, type_id) = (u8::try_from(item_id / 1000)?, u8::try_from(item_id % 1000)?);
        let at = || format!("extension {extension}: item {item_type}/{type_id}: effect");
        let value: serde_json::Value = serde_json::from_str(&text).with_context(at)?;
        let effect = effects::check_effect(&value).with_context(at)?;
        let Some(def) = items.get(item_type, type_id) else { bail!("{}: the item is not in defs.items", at()) };
        if let Err(why) = effects::fits_row(&effect, def.item_type, def.special) {
            tracing::warn!(extension, item = item_id, %why, "item effect skipped: it no longer suits the item");
            continue;
        }
        out.push((u32::try_from(item_id)?, effect));
    }
    Ok(ItemEffects::new(out))
}

/// The abilities of enabled extensions' skill levels, as `(skill id, level, abilities)`. A list that does not parse stops the start,
/// naming the extension and the skill.
pub async fn load_skill_abilities(pool: &PgPool, skills: &SkillDefs) -> Result<Vec<(u16, u8, Abilities)>> {
    load_skill_abilities_in(pool, skills, "content").await
}

/// As [`load_skill_abilities`] with the `content` schema named.
pub async fn load_skill_abilities_in(pool: &PgPool, skills: &SkillDefs, content: &str) -> Result<Vec<(u16, u8, Abilities)>> {
    use sqlx::AssertSqlSafe;
    if !present(pool, content, "ext_skill_abilities").await? {
        return Ok(Vec::new());
    }
    let rows: Vec<(String, i16, i16, String)> = sqlx::query_as(AssertSqlSafe(format!(
        "SELECT a.extension, a.skill_id, a.level, a.abilities::text FROM {content}.ext_skill_abilities a
           JOIN {content}.extensions e ON e.name = a.extension WHERE e.enabled ORDER BY a.skill_id, a.level"
    )))
    .fetch_all(pool)
    .await?;
    let mut out = Vec::new();
    for (extension, skill_id, level, text) in rows {
        let (id, level) = (u16::try_from(skill_id)?, u8::try_from(level)?);
        let at = || format!("extension {extension}: skill {id}/{level}: abilities");
        let value: serde_json::Value = serde_json::from_str(&text).with_context(at)?;
        let abilities = effects::check_abilities(&value).with_context(at)?;
        if skills.get(id, level).is_none() {
            bail!("{}: the skill level is not in defs.skills", at());
        }
        out.push((id, level, abilities));
    }
    Ok(out)
}

/// Whether `content` has the table (a database from before migration 0034 has none).
async fn present(pool: &PgPool, content: &str, table: &str) -> Result<bool> {
    Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT to_regclass('{content}.{table}') IS NOT NULL"))).fetch_one(pool).await?)
}
