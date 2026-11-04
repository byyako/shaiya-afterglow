//! What the page reads (`docs/plans/done/data-editing.md` section 6): the maps with their spawn areas, monsters, portals, NPCs, obelisks and
//! bosses, a monster with its drop table, a grade with its users, the draft, the servers. Everything is JSON built by the database where
//! it can be; the rules that need code (which items can drop, the respawn time) are the game server's own.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use anyhow::Result;
use game_server::inventory::real_type;
use serde_json::{Map, Value, json};
use sqlx::AssertSqlSafe;

use crate::console;
use crate::registry::{self, Effect, Kind};
use crate::store::{DraftLine, Editor, Line, Op, read_row, refuse};

/// The real item type that never drops (`drops::NO_DROP_REAL_TYPE`).
const NO_DROP_REAL_TYPE: u8 = 27;
const MAX_GRADE: i64 = 1024;
/// The `game_servers` columns `api_servers` reads: id, name, address, capacity, enabled, players, last heartbeat.
type ServerCols = (i16, String, String, i32, bool, i32, Option<String>);
/// How many names of a grade's items the page gets with a monster's drop table, and how many users a grade lists.
const SAMPLE_ITEMS: usize = 12;
const LISTED_USERS: usize = 300;
/// How long `info` waits for a game server's answer when the page lists the servers.
const INFO_WITHIN: Duration = Duration::from_secs(2);

fn sql(text: String) -> AssertSqlSafe<String> {
    AssertSqlSafe(text)
}

/// One item that sits in a drop grade (`DropTables::new`: grade 1 to 1024, type and type id not 0, real type not 27).
#[derive(Clone, Debug)]
struct GradeItem {
    id: i64,
    name: String,
    item_type: i64,
    type_id: i64,
    /// A limited item (`drop` above 0) counts in the grade but never drops here.
    limited: bool,
}

impl GradeItem {
    fn json(&self) -> Value {
        json!({"id": self.id, "name": self.name, "type": self.item_type, "type_id": self.type_id, "limited": self.limited})
    }
}

/// Whether the drop tables hold an item of this grade, type and type id.
pub fn can_be_in_a_grade(grade: i64, item_type: i64, type_id: i64) -> bool {
    (1..=MAX_GRADE).contains(&grade)
        && item_type != 0
        && type_id != 0
        && u8::try_from(item_type).is_ok_and(|t| real_type(t) != NO_DROP_REAL_TYPE)
}

/// A kind of value as the page shows it.
fn kind_json(kind: Kind) -> Value {
    match kind {
        Kind::Int(min, max) => json!({"type": "int", "min": min, "max": max}),
        Kind::Coord(axis) => json!({"type": "coord", "axis": format!("{axis:?}").to_lowercase()}),
        Kind::Radius => json!({"type": "radius"}),
        Kind::Mob => json!({"type": "mob"}),
        Kind::MapId => json!({"type": "map"}),
        Kind::DropRate => json!({"type": "drop_rate"}),
        Kind::PortalCountry => json!({"type": "portal_country"}),
        Kind::Mobs { first_required } => json!({"type": "mobs", "first_required": first_required}),
        Kind::Spots => json!({"type": "spots"}),
        Kind::Setting => json!({"type": "setting"}),
    }
}

/// The registry as the page needs it: targets, fields with kind and effect, refused fields with the reason.
pub fn targets_json() -> Value {
    let targets: Vec<Value> = registry::all()
        .iter()
        .map(|t| {
            let fields: Vec<Value> = t.fields.iter().map(|f| json!({"name": f.name, "kind": kind_json(f.kind), "effect": f.effect.name(), "nullable": f.nullable})).collect();
            let refused: Vec<Value> = t.refused.iter().map(|(n, why)| json!({"name": n, "why": why})).collect();
            json!({"name": t.name, "key": t.key_names, "fields": fields, "refused": refused, "can_add": t.can_add, "can_remove": t.can_remove, "new_seq_from": t.new_seq_from})
        })
        .collect();
    json!(targets)
}

impl Editor {
    async fn json_one(&self, query: String, binds: &[i64]) -> Result<Option<Value>> {
        let mut q = sqlx::query_scalar::<_, Value>(sql(query));
        for b in binds {
            q = q.bind(*b);
        }
        Ok(q.fetch_optional(&self.pool).await?)
    }

    /// A query that returns one JSON value (an array through `coalesce(jsonb_agg(...), '[]')`).
    async fn json_value(&self, query: String, binds: &[i64]) -> Result<Value> {
        Ok(self.json_one(query, binds).await?.unwrap_or(Value::Null))
    }

    /// The maps for the list on the left.
    pub async fn api_maps(&self) -> Result<Value> {
        let (w, e) = (&self.s.world, &self.s.edits);
        self.json_value(
            format!(
                "SELECT coalesce(jsonb_agg(m ORDER BY m.id), '[]'::jsonb) FROM (
                   SELECT m.id, n.name, m.map_type, m.create_type, m.size,
                          (SELECT count(*) FROM {w}.mob_areas a WHERE a.map = m.id) AS areas,
                          (SELECT count(DISTINCT am.mob_id) FROM {w}.mob_area_mobs am JOIN {w}.mob_areas a ON a.id = am.mob_area_id WHERE a.map = m.id) AS monsters,
                          (SELECT count(*) FROM {w}.map_portals p WHERE p.map = m.id) AS portals,
                          EXISTS (SELECT 1 FROM {e}.map_images i WHERE i.map = m.id) AS has_image
                   FROM {w}.maps m LEFT JOIN {e}.map_names n ON n.map = m.id) m"
            ),
            &[],
        )
        .await
    }

    /// One map with everything drawn on it, or `None` when there is no such map.
    pub async fn api_map(&self, id: i64) -> Result<Option<Value>> {
        let (w, e, d) = (&self.s.world, &self.s.edits, &self.s.defs);
        let Some(map) = self
            .json_one(
                format!(
                    "SELECT to_jsonb(x) FROM (SELECT m.id, m.size, m.map_type, m.create_type, m.war_type, n.name, m.rebirth_map, m.rebirth_pos, m.rebirth_light_map, m.rebirth_light_pos,
                            m.rebirth_fury_map, m.rebirth_fury_pos FROM {w}.maps m LEFT JOIN {e}.map_names n ON n.map = m.id WHERE m.id = $1) x"
                ),
                &[id],
            )
            .await?
        else {
            return Ok(None);
        };
        let image = self
            .json_one(format!("SELECT jsonb_build_object('content_type', content_type, 'x1', x1, 'z1', z1, 'x2', x2, 'z2', z2, 'source', source, 'added_at', added_at) FROM {e}.map_images WHERE map = $1"), &[id])
            .await?;
        let areas = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(jsonb_build_object('seq', a.seq, 'x1', a.x1, 'y1', a.y1, 'z1', a.z1, 'x2', a.x2, 'y2', a.y2, 'z2', a.z2,
                        'mobs', (SELECT coalesce(jsonb_agg(jsonb_build_object('seq', am.seq, 'mob_id', am.mob_id, 'count', am.count) ORDER BY am.seq), '[]'::jsonb)
                                 FROM {w}.mob_area_mobs am WHERE am.mob_area_id = a.id)) ORDER BY a.seq), '[]'::jsonb)
                     FROM {w}.mob_areas a WHERE a.map = $1"
                ),
                &[id],
            )
            .await?;
        let mut monsters = self
            .json_value(
                format!(
                    "WITH here AS (
                       SELECT am.mob_id, sum(am.count)::bigint AS n, count(DISTINCT a.id) AS areas
                       FROM {w}.mob_area_mobs am JOIN {w}.mob_areas a ON a.id = am.mob_area_id WHERE a.map = $1 GROUP BY am.mob_id),
                     spread AS (
                       SELECT am.mob_id, array_agg(DISTINCT a.map ORDER BY a.map) AS maps
                       FROM {w}.mob_area_mobs am JOIN {w}.mob_areas a ON a.id = am.mob_area_id
                       WHERE am.mob_id IN (SELECT mob_id FROM here) GROUP BY am.mob_id)
                     SELECT coalesce(jsonb_agg(jsonb_build_object('mobid', h.mob_id, 'name', d.mobname, 'level', d.level, 'hp', d.hp, 'exp', d.exp, 'money1', d.money1, 'money2', d.money2,
                            'respawn_code', d.attackspecial3, 'here', h.n, 'areas', h.areas,
                            'other_maps', (SELECT coalesce(jsonb_agg(m) FILTER (WHERE m <> $1), '[]'::jsonb) FROM unnest(s.maps) m)) ORDER BY d.level, h.mob_id), '[]'::jsonb)
                     FROM here h LEFT JOIN {d}.mobs d ON d.mobid = h.mob_id LEFT JOIN spread s ON s.mob_id = h.mob_id"
                ),
                &[id],
            )
            .await?;
        if let Some(list) = monsters.as_array_mut() {
            for m in list {
                let secs =
                    m["respawn_code"].as_u64().and_then(|c| u8::try_from(c).ok()).map(|c| game_server::mobs::respawn_time(c).as_secs());
                m["respawn_secs"] = json!(secs);
            }
        }
        let portals = self
            .json_value(
                format!("SELECT coalesce(jsonb_agg(to_jsonb(p) ORDER BY p.seq), '[]'::jsonb) FROM {w}.map_portals p WHERE p.map = $1"),
                &[id],
            )
            .await?;
        let npcs = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(jsonb_build_object('seq', n.seq, 'type', n.type, 'type_id', n.type_id, 'name', nn.name,
                        'positions', (SELECT coalesce(jsonb_agg(jsonb_build_array(p.pos_x, p.pos_y, p.pos_z) ORDER BY p.seq), '[]'::jsonb) FROM {w}.map_npc_positions p WHERE p.map_npc_id = n.id)) ORDER BY n.seq), '[]'::jsonb)
                     FROM {w}.map_npcs n LEFT JOIN {w}.npcs nn ON nn.type = n.type AND nn.type_id = n.type_id WHERE n.map = $1"
                ),
                &[id],
            )
            .await?;
        let binds = self
            .json_value(
                format!("SELECT coalesce(jsonb_agg(to_jsonb(b) ORDER BY b.seq), '[]'::jsonb) FROM {w}.map_binds b WHERE b.map = $1"),
                &[id],
            )
            .await?;
        let obelisks = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(to_jsonb(o) || jsonb_build_object('guards', (SELECT coalesce(jsonb_agg(to_jsonb(g) ORDER BY g.seq), '[]'::jsonb) FROM {w}.obelisk_guards g WHERE g.obelisk_seq = o.seq)) ORDER BY o.seq), '[]'::jsonb)
                     FROM {w}.obelisks o WHERE o.map = $1"
                ),
                &[id],
            )
            .await?;
        let bosses = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(to_jsonb(b) || jsonb_build_object('followers', (SELECT coalesce(jsonb_agg(to_jsonb(f) ORDER BY f.seq), '[]'::jsonb) FROM {w}.boss_followers f WHERE f.group_seq = b.seq)) ORDER BY b.seq), '[]'::jsonb)
                     FROM {w}.boss_groups b WHERE b.map = $1"
                ),
                &[id],
            )
            .await?;
        // Names and levels of every monster the map mentions, whether in a spawn area or not.
        let mut wanted: BTreeSet<i64> = BTreeSet::new();
        let mut collect = |v: &Value| {
            if let Some(n) = v.as_i64() {
                wanted.insert(n);
            }
        };
        for a in areas.as_array().into_iter().flatten() {
            a["mobs"].as_array().into_iter().flatten().for_each(|m| collect(&m["mob_id"]));
        }
        for o in obelisks.as_array().into_iter().flatten() {
            ["mob_neutral", "mob_light", "mob_fury"].iter().for_each(|k| collect(&o[*k]));
            for g in o["guards"].as_array().into_iter().flatten() {
                ["mob_neutral", "mob_light", "mob_fury"].iter().for_each(|k| collect(&g[*k]));
            }
        }
        for b in bosses.as_array().into_iter().flatten() {
            b["mobs"].as_array().into_iter().flatten().for_each(&mut collect);
            for f in b["followers"].as_array().into_iter().flatten() {
                f["mobs"].as_array().into_iter().flatten().for_each(&mut collect);
            }
        }
        wanted.remove(&0);
        let ids: Vec<i32> = wanted.iter().filter_map(|n| i32::try_from(*n).ok()).collect();
        let rows: Vec<(i16, String, i16, i32)> =
            sqlx::query_as(sql(format!("SELECT mobid, mobname, level, hp FROM {d}.mobs WHERE mobid::integer = ANY($1)")))
                .bind(&ids)
                .fetch_all(&self.pool)
                .await?;
        let mob_info: Map<String, Value> =
            rows.into_iter().map(|(id, name, level, hp)| (id.to_string(), json!({"name": name, "level": level, "hp": hp}))).collect();
        Ok(Some(json!({
            "map": map, "image": image, "areas": areas, "monsters": monsters, "portals": portals, "npcs": npcs, "binds": binds,
            "obelisks": obelisks, "bosses": bosses, "mob_info": mob_info,
        })))
    }

    /// The map's blocked cells: its side and one bit per unit square, least significant bit first.
    pub async fn api_blocked(&self, id: i64) -> Result<Option<(i32, Vec<u8>)>> {
        let w = &self.s.world;
        Ok(sqlx::query_as(sql(format!("SELECT size, blocked FROM {w}.maps WHERE id = $1")))
            .bind(id as i16)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// The items of every drop grade, as the drop tables hold them (`DropTables::new`).
    async fn grade_items(&self) -> Result<BTreeMap<i64, Vec<GradeItem>>> {
        let d = &self.s.defs;
        let rows: Vec<(i32, String, i16, i16, i16, i16)> = sqlx::query_as(sql(format!(
            "SELECT itemid, itemname, type, typeid, grade, coalesce(\"drop\", 0)::smallint FROM {d}.items WHERE grade BETWEEN 1 AND {MAX_GRADE} ORDER BY type, typeid"
        )))
        .fetch_all(&self.pool)
        .await?;
        let mut grades: BTreeMap<i64, Vec<GradeItem>> = BTreeMap::new();
        for (id, name, item_type, type_id, grade, drop_limit) in rows {
            if can_be_in_a_grade(grade.into(), item_type.into(), type_id.into()) {
                grades.entry(grade.into()).or_default().push(GradeItem {
                    id: id.into(),
                    name,
                    item_type: item_type.into(),
                    type_id: type_id.into(),
                    limited: drop_limit != 0,
                });
            }
        }
        Ok(grades)
    }

    /// A monster: its definition, its nine drop entries (with the items of each grade), the maps it is spawned on and the edits in force.
    pub async fn api_mob(&self, id: i64) -> Result<Option<Value>> {
        let (w, e, d) = (&self.s.world, &self.s.edits, &self.s.defs);
        let Some(mut def) = self.json_one(format!("SELECT to_jsonb(m) - 'rowid' FROM {d}.mobs m WHERE mobid = $1"), &[id]).await? else {
            return Ok(None);
        };
        let secs = def["attackspecial3"].as_u64().and_then(|c| u8::try_from(c).ok()).map(|c| game_server::mobs::respawn_time(c).as_secs());
        def["respawn_secs"] = json!(secs);
        let entries = self
            .json_value(format!("SELECT coalesce(jsonb_agg(jsonb_build_object('order', itemorder, 'grade', grade, 'droprate', droprate) ORDER BY itemorder), '[]'::jsonb) FROM {d}.mob_items WHERE mobid = $1 AND itemorder BETWEEN 1 AND 9"), &[id])
            .await?;
        let grades = self.grade_items().await?;
        let mut drops = Vec::new();
        for entry in entries.as_array().into_iter().flatten() {
            let grade = entry["grade"].as_i64().unwrap_or(0);
            let items = grades.get(&grade).map(Vec::as_slice).unwrap_or(&[]);
            let rate = entry["droprate"].as_f64().unwrap_or(0.0);
            let order = entry["order"].as_i64().unwrap_or(0);
            // Entries 1 to 6 are percents, 7 to 9 per million (section 2).
            let percent = if order <= 6 { rate } else { rate / 10_000.0 };
            let live = items.iter().filter(|i| !i.limited).count();
            let per_item = if items.is_empty() { 0.0 } else { percent / items.len() as f64 };
            drops.push(json!({
                "order": order, "grade": grade, "droprate": entry["droprate"], "percent": percent, "items": items.len(), "dropping": live, "percent_per_item": per_item,
                "sample": items.iter().take(SAMPLE_ITEMS).map(GradeItem::json).collect::<Vec<_>>(),
            }));
        }
        let maps = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(jsonb_build_object('map', x.map, 'count', x.n, 'areas', x.areas) ORDER BY x.map), '[]'::jsonb) FROM (
                       SELECT a.map, sum(am.count)::bigint AS n, count(DISTINCT a.id) AS areas FROM {w}.mob_area_mobs am JOIN {w}.mob_areas a ON a.id = am.mob_area_id WHERE am.mob_id = $1 GROUP BY a.map) x"
                ),
                &[id],
            )
            .await?;
        let edits = self
            .json_value(
                format!("SELECT coalesce(jsonb_agg(jsonb_build_object('target', target, 'key', key, 'field', field, 'value', value, 'original', original, 'change_set', change_set) ORDER BY target, key, field), '[]'::jsonb) FROM {e}.fields WHERE (target = 'mob' AND key = $1::text) OR (target = 'mob_drop' AND key LIKE $1::text || '/%')"),
                &[id],
            )
            .await?;
        Ok(Some(json!({"def": def, "drops": drops, "maps": maps, "edits": edits})))
    }

    /// A grade: its items and who uses it (monsters' entries, box sets, sequence groups).
    pub async fn api_grade(&self, grade: i64) -> Result<Value> {
        let (w, d) = (&self.s.world, &self.s.defs);
        let all = self.grade_items().await?;
        let items: Vec<Value> = all.get(&grade).map(|v| v.iter().map(GradeItem::json).collect()).unwrap_or_default();
        // Items with this grade the drop tables do not hold (type 0, real type 27): the page lists them apart.
        let ignored = self
            .json_value(format!("SELECT coalesce(jsonb_agg(jsonb_build_object('id', itemid, 'name', itemname, 'type', type, 'type_id', typeid) ORDER BY itemid), '[]'::jsonb) FROM {d}.items WHERE grade = $1"), &[grade])
            .await?
            .as_array()
            .map(|a| a.iter().filter(|i| !items.iter().any(|k| k["id"] == i["id"])).cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let entries_total: i64 =
            sqlx::query_scalar(sql(format!("SELECT count(*) FROM {d}.mob_items WHERE grade = $1 AND itemorder BETWEEN 1 AND 9")))
                .bind(grade as i16)
                .fetch_one(&self.pool)
                .await?;
        let entries = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(x), '[]'::jsonb) FROM (
                       SELECT mi.mobid, m.mobname AS name, m.level, mi.itemorder AS \"order\", mi.droprate FROM {d}.mob_items mi LEFT JOIN {d}.mobs m ON m.mobid = mi.mobid
                       WHERE mi.grade = $1 AND mi.itemorder BETWEEN 1 AND 9 ORDER BY mi.mobid, mi.itemorder LIMIT {LISTED_USERS}) x"
                ),
                &[grade],
            )
            .await?;
        let boxes = self
            .json_value(
                format!(
                    "SELECT coalesce(jsonb_agg(jsonb_build_object('set', c.set_no, 'weight', c.weight, 'total', (SELECT sum(weight) FROM {w}.item_create t WHERE t.set_no = c.set_no)) ORDER BY c.set_no, c.seq), '[]'::jsonb)
                     FROM {w}.item_create c WHERE c.grade = $1"
                ),
                &[grade],
            )
            .await?;
        let sequences = self
            .json_value(format!("SELECT coalesce(jsonb_agg(jsonb_build_object('group', group_no, 'sequence', sequence) ORDER BY group_no), '[]'::jsonb) FROM {w}.sequence_groups WHERE $1::integer = ANY(grades)"), &[grade])
            .await?;
        Ok(
            json!({"grade": grade, "items": items, "ignored_items": ignored, "entries_total": entries_total, "entries": entries, "box_sets": boxes, "sequence_groups": sequences}),
        )
    }

    /// Items by id or by part of the name.
    pub async fn api_items(&self, q: &str) -> Result<Value> {
        let d = &self.s.defs;
        let q = q.trim();
        let rows: Vec<(i32, String, i16, i16, i16, i16)> = if let Ok(id) = q.parse::<i32>() {
            sqlx::query_as(sql(format!("SELECT itemid, itemname, type, typeid, grade, coalesce(\"drop\", 0)::smallint FROM {d}.items WHERE itemid = $1 ORDER BY itemid"))).bind(id).fetch_all(&self.pool).await?
        } else if q.is_empty() {
            Vec::new()
        } else {
            let like = format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"));
            sqlx::query_as(sql(format!("SELECT itemid, itemname, type, typeid, grade, coalesce(\"drop\", 0)::smallint FROM {d}.items WHERE itemname ILIKE $1 ORDER BY itemid LIMIT 40"))).bind(like).fetch_all(&self.pool).await?
        };
        Ok(Value::Array(
            rows.into_iter()
                .map(|(id, name, t, tid, grade, drop_limit)| {
                    json!({"id": id, "name": name, "type": t, "type_id": tid, "grade": grade, "limited": drop_limit != 0, "in_a_grade": can_be_in_a_grade(grade.into(), t.into(), tid.into())})
                })
                .collect(),
        ))
    }

    /// The facts behind the "drop this one item" helper (section 6): the item, who loses it when it leaves its grade, and the lowest grade
    /// that nothing uses (items, drop entries, box sets, sequence groups, and the draft's own grade lines).
    pub async fn api_helper(&self, item: i64) -> Result<Value> {
        let (w, d) = (&self.s.world, &self.s.defs);
        let Some(row) = self.api_items(&item.to_string()).await?.as_array().and_then(|a| a.first().cloned()) else {
            return refuse(format!("there is no item {item}"));
        };
        let grade = row["grade"].as_i64().unwrap_or(0);
        let old = self.api_grade(grade).await?;
        let mut used: BTreeSet<i64> =
            sqlx::query_scalar::<_, i16>(sql(format!("SELECT DISTINCT grade FROM {d}.items UNION SELECT grade FROM {d}.mob_items")))
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(i64::from)
                .collect();
        used.extend(
            sqlx::query_scalar::<_, i32>(sql(format!(
                "SELECT grade FROM {w}.item_create UNION SELECT unnest(grades) FROM {w}.sequence_groups"
            )))
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(i64::from),
        );
        for line in self.draft_list().await? {
            if line.field.as_deref() == Some("grade") && matches!(line.target.as_str(), "item_grade" | "mob_drop") {
                used.extend(line.value.as_ref().and_then(Value::as_i64));
            }
        }
        let free = (1..=MAX_GRADE).find(|g| !used.contains(g));
        Ok(json!({"item": row, "old_grade": old, "new_grade": free}))
    }

    /// Every edit in force on existing rows and every row added or removed, for marking them on the page.
    pub async fn api_edits(&self) -> Result<Value> {
        let e = &self.s.edits;
        let fields = self
            .json_value(format!("SELECT coalesce(jsonb_agg(jsonb_build_object('target', target, 'key', key, 'field', field, 'value', value, 'original', original, 'change_set', change_set) ORDER BY target, key, field), '[]'::jsonb) FROM {e}.fields"), &[])
            .await?;
        let rows = self.json_value(format!("SELECT coalesce(jsonb_agg(jsonb_build_object('target', target, 'key', key, 'action', action, 'change_set', change_set) ORDER BY target, key), '[]'::jsonb) FROM {e}.rows"), &[]).await?;
        Ok(json!({"fields": fields, "rows": rows}))
    }

    /// The draft, each line with the value now in force and when it takes effect.
    pub async fn api_draft(&self) -> Result<Value> {
        let d = &self.s.defs;
        let mut conn = self.pool.acquire().await?;
        let mut out = Vec::new();
        for line in self.draft_list().await? {
            let mut v = serde_json::to_value(&line)?;
            if let Ok(t) = registry::target(&line.target)
                && let Ok(key) = t.parse_key(&line.key)
            {
                let row = read_row(&mut conn, &self.s, t, &key).await?;
                v["now"] = match (&line.field, &row) {
                    (Some(f), Some(row)) => row.get(f).cloned().unwrap_or(Value::Null),
                    _ => Value::Null,
                };
                v["effect"] = json!(match (line.action.as_str(), line.field.as_deref().map(|f| t.field(f))) {
                    ("set" | "unset", Some(Ok(f))) => f.effect.name(),
                    _ => Effect::Restart.name(),
                });
                if matches!(line.target.as_str(), "mob" | "mob_drop") {
                    v["mob"] = json!(key[0]);
                }
            }
            out.push(v);
        }
        // Names of the monsters and items the lines are about.
        let mobs: Vec<i32> = out.iter().filter_map(|v| v["mob"].as_i64()).filter_map(|n| i32::try_from(n).ok()).collect();
        let names: BTreeMap<i32, String> =
            sqlx::query_as::<_, (i16, String)>(sql(format!("SELECT mobid, mobname FROM {d}.mobs WHERE mobid::integer = ANY($1)")))
                .bind(&mobs)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|(i, n)| (i.into(), n))
                .collect();
        let items: Vec<i32> = out.iter().filter(|v| v["target"] == "item_grade").filter_map(|v| v["key"].as_str()?.parse().ok()).collect();
        let item_names: BTreeMap<i32, String> =
            sqlx::query_as::<_, (i32, String)>(sql(format!("SELECT itemid, itemname FROM {d}.items WHERE itemid = ANY($1)")))
                .bind(&items)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .collect();
        for v in &mut out {
            let label = match v["target"].as_str() {
                Some("item_grade") => v["key"].as_str().and_then(|k| k.parse::<i32>().ok()).and_then(|k| item_names.get(&k)).cloned(),
                _ => v["mob"].as_i64().and_then(|n| i32::try_from(n).ok()).and_then(|n| names.get(&n)).cloned(),
            };
            v["name"] = label.map_or(Value::Null, Value::String);
        }
        Ok(Value::Array(out))
    }

    /// Adds lines to the draft as one request: a `set` or `unset` of a field that the draft already changes replaces that line (a drag
    /// that goes on is one change). If any line is refused, the draft is left as it was.
    pub async fn draft_put(&self, lines: &[Line], by: &str) -> Result<Vec<i32>> {
        let e = &self.s.edits;
        let existing = self.draft_list().await?;
        let mut replaced: Vec<DraftLine> = Vec::new();
        for line in lines {
            let (Op::Set { field, .. } | Op::Unset { field }) = &line.op else { continue };
            let key = registry::target(&line.target).ok().and_then(|t| t.key_text(&line.key).ok());
            for d in &existing {
                if Some(&d.key) == key.as_ref()
                    && d.target == line.target
                    && d.field.as_deref() == Some(field)
                    && matches!(d.action.as_str(), "set" | "unset")
                    && !replaced.iter().any(|r| r.seq == d.seq)
                {
                    replaced.push(d.clone());
                }
            }
        }
        for r in &replaced {
            self.draft_remove(r.seq).await?;
        }
        let mut added = Vec::new();
        for line in lines {
            match self.draft_add(line, by).await {
                Ok(seq) => added.push(seq),
                Err(err) => {
                    for seq in added {
                        self.draft_remove(seq).await?;
                    }
                    for r in &replaced {
                        sqlx::query(sql(format!("INSERT INTO {e}.draft (seq, target, key, field, action, value, made_by, made_at) OVERRIDING SYSTEM VALUE VALUES ($1, $2, $3, $4, $5, $6, $7, $8::text::timestamptz)")))
                            .bind(r.seq)
                            .bind(&r.target)
                            .bind(&r.key)
                            .bind(&r.field)
                            .bind(&r.action)
                            .bind(&r.value)
                            .bind(&r.made_by)
                            .bind(&r.made_at)
                            .execute(&self.pool)
                            .await?;
                    }
                    return Err(err);
                }
            }
        }
        Ok(added)
    }

    /// The game servers with what `info` says (who is online). A server that does not answer within two seconds has `info: null`.
    pub async fn api_servers(&self) -> Result<Value> {
        let rows: Vec<ServerCols> = sqlx::query_as(
            "SELECT id, name::text, host(address), capacity, enabled, players, last_seen::text FROM game_servers ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut asks = Vec::new();
        for (id, ..) in &rows {
            let pool = self.pool.clone();
            let id = *id;
            asks.push(tokio::spawn(async move { console::request_within(&pool, id as u8, "info", INFO_WITHIN).await.ok() }));
        }
        let mut out = Vec::new();
        for ((id, name, address, capacity, enabled, players, last_seen), ask) in rows.into_iter().zip(asks) {
            let info = ask.await.ok().flatten();
            out.push(json!({"id": id, "name": name, "address": address, "capacity": capacity, "enabled": enabled, "players": players, "last_seen": last_seen, "info": info}));
        }
        Ok(Value::Array(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn which_items_a_grade_can_hold() {
        assert!(can_be_in_a_grade(5, 30, 71));
        assert!(!can_be_in_a_grade(0, 30, 71), "grade 0 is off");
        assert!(!can_be_in_a_grade(1025, 30, 71), "above the highest grade");
        assert!(!can_be_in_a_grade(5, 0, 71));
        assert!(!can_be_in_a_grade(5, 30, 0));
    }

    #[test]
    fn the_registry_is_described_for_the_page() {
        let all = targets_json();
        let mob = all.as_array().unwrap().iter().find(|t| t["name"] == "mob").unwrap();
        assert!(mob["fields"].as_array().unwrap().iter().any(|f| f["name"] == "hp" && f["effect"] == "next spawn"));
        assert!(mob["refused"].as_array().unwrap().iter().any(|r| r["name"] == "mobname"));
        let drop = all.as_array().unwrap().iter().find(|t| t["name"] == "mob_drop").unwrap();
        assert!(drop["fields"].as_array().unwrap().iter().any(|f| f["kind"]["type"] == "drop_rate"));
    }
}
