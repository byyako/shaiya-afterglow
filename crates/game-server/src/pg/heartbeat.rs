//! The heartbeat of the PostgreSQL backend, and the loaders of the definitions and the world that follow it.

use super::*;

impl Heartbeat for PgBackend {
    fn pool_gauges(&self) -> Option<(u32, u32)> {
        // `num_idle` is a count taken at the moment and can be a little over `size` while a connection is being closed: the line
        // takes the smaller.
        let size = self.pool.size();
        Some((size, u32::try_from(self.pool.num_idle()).unwrap_or(u32::MAX).min(size)))
    }

    async fn load_mob_id(&self, server_id: u8) -> Result<u32> {
        let id: Option<i64> = sqlx::query_scalar("SELECT mob_id FROM game_servers WHERE id = $1")
            .bind(i16::from(server_id))
            .fetch_optional(&self.pool)
            .await?;
        Ok(u32::try_from(id.unwrap_or(1))?)
    }

    async fn save_mob_id(&self, server_id: u8, id: u32) -> Result<()> {
        let done = sqlx::query("UPDATE game_servers SET mob_id = $2 WHERE id = $1")
            .bind(i16::from(server_id))
            .bind(i64::from(id))
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            bail!("game server {server_id} is not registered (servers add {server_id} <name> <ip>)");
        }
        Ok(())
    }

    async fn load_kill_counts(&self, server_id: u8) -> Result<[u32; 2]> {
        let rows: Vec<(i16, i64)> = sqlx::query_as("SELECT country, count FROM faction_kill_counts WHERE server_id = $1")
            .bind(i16::from(server_id))
            .fetch_all(&self.pool)
            .await?;
        let mut counts = [0; 2];
        for (country, count) in rows {
            if let Some(c) = counts.get_mut(usize::try_from(country)?) {
                *c = u32::try_from(count)?;
            }
        }
        Ok(counts)
    }

    async fn save_kill_counts(&self, server_id: u8, counts: [u32; 2]) -> Result<()> {
        for (country, count) in counts.into_iter().enumerate() {
            sqlx::query(
                "INSERT INTO faction_kill_counts (server_id, country, count) VALUES ($1, $2, $3)
                 ON CONFLICT (server_id, country) DO UPDATE SET count = EXCLUDED.count",
            )
            .bind(i16::from(server_id))
            .bind(country as i16)
            .bind(i64::from(count))
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn load_obelisk_owners(&self, server_id: u8) -> Result<Vec<crate::obelisk::Owner>> {
        let rows: Vec<(i16, i16, i16, i16)> =
            sqlx::query_as("SELECT map, number, country, last_destroy FROM obelisk_owners WHERE server_id = $1 ORDER BY map, number")
                .bind(i16::from(server_id))
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|(map, number, country, last)| {
                Ok(crate::obelisk::Owner {
                    map: u16::try_from(map)?,
                    number: u16::try_from(number)?,
                    country: u8::try_from(country)?,
                    last: u8::try_from(last)?,
                })
            })
            .collect()
    }

    async fn save_obelisk_owner(&self, server_id: u8, owner: crate::obelisk::Owner) -> Result<()> {
        sqlx::query(
            "INSERT INTO obelisk_owners (server_id, map, number, country, last_destroy) VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (server_id, map, number) DO UPDATE SET country = EXCLUDED.country, last_destroy = EXCLUDED.last_destroy",
        )
        .bind(i16::from(server_id))
        .bind(i16::try_from(owner.map)?)
        .bind(i16::try_from(owner.number)?)
        .bind(i16::from(owner.country))
        .bind(i16::from(owner.last))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn heartbeat(&self, server_id: u8, players: u16, open: bool, online: &[u32]) -> Result<()> {
        // No `last_seen` is how the login server lists a server as closed.
        let done = sqlx::query("UPDATE game_servers SET players = $2, last_seen = CASE WHEN $3 THEN now() END WHERE id = $1")
            .bind(i16::from(server_id))
            .bind(i32::from(players))
            .bind(open)
            .execute(&self.pool)
            .await?;
        if done.rows_affected() == 0 {
            bail!("game server {server_id} is not registered (servers add {server_id} <name> <ip>)");
        }
        // Only the accounts this process holds: a row whose end was lost (or that another process of this server id left) is not
        // renewed and runs out within `SESSION_GAME_SECS`. An id that does not fit the column cannot be a row.
        let online: Vec<i32> = online.iter().filter_map(|&user| i32::try_from(user).ok()).collect();
        if online.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "UPDATE sessions SET expires_at = now() + make_interval(secs => $2)
              WHERE server_id = $1 AND game_connected_at IS NOT NULL AND user_id = ANY($3)",
        )
        .bind(i16::from(server_id))
        .bind(SESSION_GAME_SECS)
        .bind(&online)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

/// Level 1 HP, SP and MP per class from `defs.status_defs` (loaded from PS_GameDefs by `tools/defs`).
/// The whole status table (`defs.status_defs`): HP, SP and MP by class and level (`crate::stats`).
pub async fn load_status_table(pool: &PgPool) -> Result<StatusTable> {
    let rows: Vec<(i16, i16, i16, i16, i16)> =
        sqlx::query_as("SELECT job, level, hp, sp, mp FROM defs.status_defs").fetch_all(pool).await?;
    if rows.len() < 6 * crate::enter::MAX_LEVEL {
        bail!("defs.status_defs is incomplete; load the game definitions (tools/defs)");
    }
    let rows = rows
        .into_iter()
        .map(|(job, level, hp, sp, mp)| {
            Ok((u8::try_from(job)?, u16::try_from(level)?, [u16::try_from(hp)?, u16::try_from(sp)?, u16::try_from(mp)?]))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(StatusTable::new(rows))
}

pub async fn load_class_base(pool: &PgPool) -> Result<[crate::creation::ClassBase; 6]> {
    let rows: Vec<(i16, i16, i16, i16)> =
        sqlx::query_as("SELECT job, hp, sp, mp FROM defs.status_defs WHERE level = 1 ORDER BY job").fetch_all(pool).await?;
    let mut base = [crate::creation::ClassBase::default(); 6];
    let mut seen = [false; 6];
    for (job, hp, sp, mp) in rows {
        let job = usize::try_from(job)?;
        if job >= 6 {
            continue;
        }
        base[job] = crate::creation::ClassBase { hp: u16::try_from(hp)?, sp: u16::try_from(sp)?, mp: u16::try_from(mp)? };
        seen[job] = true;
    }
    if !seen.iter().all(|&s| s) {
        bail!("defs.status_defs has no level 1 row for every class; load the game definitions (tools/defs)");
    }
    Ok(base)
}

/// Starting items from `defs.base_items_defs` in row order, with the quality of each item's definition; rows whose item does
/// not exist are skipped, as the original did (`CGameData::LoadBaseItem`).
pub async fn load_base_items(pool: &PgPool) -> Result<Vec<BaseItemDef>> {
    let rows: Vec<(i16, i16, i32, i16, i16, i32, i16)> = sqlx::query_as(
        "SELECT b.family, b.job, b.itemid, b.type, b.typeid, b.count, i.quality
           FROM defs.base_items_defs b JOIN defs.items i ON i.type = b.type AND i.typeid = b.typeid
          WHERE b.family BETWEEN 0 AND 3 AND b.job BETWEEN 0 AND 5
          ORDER BY b.rowid",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(family, job, item_id, item_type, type_id, count, quality)| {
            Ok(BaseItemDef {
                family: u8::try_from(family)?,
                job: u8::try_from(job)?,
                item_id: u32::try_from(item_id)?,
                item_type: u8::try_from(item_type)?,
                type_id: u8::try_from(type_id)?,
                count: u8::try_from(count.clamp(1, 255))?,
                quality: u16::try_from(quality)?,
            })
        })
        .collect()
}

/// The experience table from `defs.exp_defs`.
pub async fn load_exp(pool: &PgPool) -> Result<ExpTable> {
    let rows: Vec<(i16, i16, i32)> = sqlx::query_as("SELECT level, grow, exp FROM defs.exp_defs").fetch_all(pool).await?;
    if rows.len() < 4 * crate::enter::MAX_LEVEL {
        bail!("defs.exp_defs is incomplete; load the game definitions (tools/defs)");
    }
    let rows = rows
        .into_iter()
        .map(|(level, grow, exp)| Ok((u16::try_from(level)?, u8::try_from(grow)?, u32::try_from(exp)?)))
        .collect::<Result<Vec<_>>>()?;
    Ok(ExpTable::from_rows(rows))
}

/// A placement with its first position; the position columns are NULL when it has none.
#[derive(sqlx::FromRow)]
struct MapNpcDbRow {
    map: i16,
    kind: i16,
    type_id: i16,
    pos_x: Option<f32>,
    pos_y: Option<f32>,
    pos_z: Option<f32>,
    direction: Option<f32>,
    /// Every position's x, y, z in order.
    walk: Option<Vec<f32>>,
}

#[derive(sqlx::FromRow)]
struct MobAreaRow {
    map: i16,
    seq: i32,
    x1: f32,
    y1: f32,
    z1: f32,
    x2: f32,
    y2: f32,
    z2: f32,
    mob_id: i32,
    count: i32,
}

#[derive(sqlx::FromRow)]
struct GateRow {
    npc_type_id: i16,
    seq: i16,
    map: i16,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    name: String,
    cost: i64,
}

#[derive(sqlx::FromRow)]
struct MapPortalRow {
    map: i16,
    pos_x: f32,
    pos_y: f32,
    pos_z: f32,
    country: i32,
    level_min: i32,
    level_max: i32,
    to_map: i32,
    to_x: f32,
    to_y: f32,
    to_z: f32,
}

/// The maps and NPC placements from the `world` schema (`tools/world/import.py`, `db/world/schema.sql`).
/// `gambling` places the gambling house's NPCs (type 5, `mini_game`).
pub async fn load_world(pool: &PgPool, gambling: bool) -> Result<World> {
    let maps: Vec<(i16, i32)> = sqlx::query_as("SELECT id, size FROM world.maps").fetch_all(pool).await?;
    if maps.is_empty() {
        bail!("world.maps is empty; import the maps (tools/world/import.py)");
    }
    let maps = maps.into_iter().map(|(id, size)| Ok((u16::try_from(id)?, u32::try_from(size)?))).collect::<Result<Vec<_>>>()?;
    // Zone keys keep the map in their low byte (docs/original/instances.md section 18).
    if let Some((id, _)) = maps.iter().find(|(id, _)| *id > crate::world::MAX_MAP_ID) {
        bail!("world.maps has map {id}; this server runs maps 0 to {}", crate::world::MAX_MAP_ID);
    }
    // value_b and value_c are `nMoveDist` and `nMoveSpeed` (docs/original/guards.md section 1).
    let npc_rows: Vec<(i16, i16, i64, i64, i16)> =
        sqlx::query_as("SELECT type, type_id, value_b, value_c, country FROM world.npcs").fetch_all(pool).await?;
    let mut npc_defs = HashMap::new();
    for (kind, id, move_dist, move_speed, country) in npc_rows {
        let (Ok(kind), Ok(id)) = (u8::try_from(kind), u16::try_from(id)) else { continue };
        let def = crate::world::NpcDef {
            move_dist: u32::try_from(move_dist).unwrap_or(0),
            move_speed: u32::try_from(move_speed).unwrap_or(0),
            country: u8::try_from(country)?,
        };
        npc_defs.insert((kind, id), def);
    }
    let defined = npc_defs.keys().copied().collect();
    let rows: Vec<MapNpcDbRow> = sqlx::query_as(
        "SELECT n.map, n.type AS kind, n.type_id, p.pos_x, p.pos_y, p.pos_z, p.direction,
                (SELECT array_agg(v ORDER BY q.seq, u.i)
                 FROM world.map_npc_positions q, unnest(ARRAY[q.pos_x, q.pos_y, q.pos_z]) WITH ORDINALITY AS u(v, i)
                 WHERE q.map_npc_id = n.id) AS walk
         FROM world.map_npcs n LEFT JOIN world.map_npc_positions p ON p.map_npc_id = n.id AND p.seq = 0
         ORDER BY n.map, n.seq",
    )
    .fetch_all(pool)
    .await?;
    let placements = rows
        .into_iter()
        .map(|r| {
            let first = match (r.pos_x, r.pos_y, r.pos_z, r.direction) {
                (Some(x), Some(y), Some(z), Some(d)) => Some([x, y, z, d]),
                _ => None,
            };
            let walk = r.walk.unwrap_or_default().as_chunks::<3>().0.to_vec();
            Ok(MapNpcRow { map: u16::try_from(r.map)?, kind: r.kind, type_id: r.type_id, first, walk })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut world = World::build_with(maps, &defined, placements, gambling);
    world.set_npc_defs(npc_defs);
    let goods: Vec<(i16, i16, i16)> =
        sqlx::query_as("SELECT npc_type_id, item_type, item_type_id FROM world.npc_shop_items ORDER BY npc_type_id, seq")
            .fetch_all(pool)
            .await?;
    let mut shops: HashMap<u16, Vec<(u8, u8)>> = HashMap::new();
    for (merchant, item_type, type_id) in goods {
        shops.entry(u16::try_from(merchant)?).or_default().push((u8::try_from(item_type)?, u8::try_from(type_id)?));
    }
    world.set_shops(shops);
    let shop_types: Vec<(i16, i16)> =
        sqlx::query_as("SELECT type_id, merchant_byte FROM world.npcs WHERE type = 1 AND merchant_byte IS NOT NULL")
            .fetch_all(pool)
            .await?;
    world.set_shop_types(shop_types.into_iter().map(|(id, t)| Ok((u16::try_from(id)?, u8::try_from(t)?))).collect::<Result<_>>()?);

    let keepers: Vec<(i16, i16)> = sqlx::query_as("SELECT type_id, country FROM world.npcs WHERE type = 2").fetch_all(pool).await?;
    let mut gatekeepers: HashMap<u16, GateKeeper> = HashMap::new();
    for (type_id, country) in keepers {
        gatekeepers.insert(u16::try_from(type_id)?, GateKeeper { country: u8::try_from(country)?, gates: [None; 3] });
    }
    let gates: Vec<GateRow> =
        sqlx::query_as("SELECT npc_type_id, seq, map, pos_x, pos_y, pos_z, name, cost FROM world.npc_gates").fetch_all(pool).await?;
    for g in gates {
        let (Some(keeper), Ok(seq)) = (gatekeepers.get_mut(&u16::try_from(g.npc_type_id)?), usize::try_from(g.seq)) else { continue };
        if seq < 3 && !g.name.is_empty() {
            // The original adds 0.9 to y when it loads a destination (CNpcData::Load).
            let gate = Gate { map: u16::try_from(g.map)?, position: [g.pos_x, g.pos_y + 0.9, g.pos_z], cost: u32::try_from(g.cost)? };
            keeper.gates[seq] = Some(gate);
        }
    }
    world.set_gatekeepers(gatekeepers);
    world.set_guild_house(load_guild_house(pool).await?);

    let types: Vec<(i16, i16, String)> = sqlx::query_as("SELECT id, create_type, map_type FROM world.maps").fetch_all(pool).await?;
    // `CMap::LoadConfig`: a `MapType` of "D" in any case is a dungeon.
    let dungeons: Vec<u16> = types
        .iter()
        .filter(|(_, _, kind)| kind.eq_ignore_ascii_case("d"))
        .map(|&(id, _, _)| u16::try_from(id))
        .collect::<Result<_, _>>()?;
    world.set_dungeons(dungeons);
    let war: Vec<i16> = sqlx::query_scalar("SELECT id FROM world.maps WHERE war_type = 1").fetch_all(pool).await?;
    world.set_war_maps(war.into_iter().map(u16::try_from).collect::<Result<Vec<_>, _>>()?);
    let create_types = types.into_iter().map(|(id, t, _)| Ok((u16::try_from(id)?, u8::try_from(t)?))).collect::<Result<_>>()?;
    // id, CreateType, CreateTime triples, CreateMinUser, EnterMaxUser, ExpireTime.
    type RulesRow = (i16, i16, Option<Vec<i16>>, i32, i32, i32);
    let rules: Vec<RulesRow> = sqlx::query_as(
        "SELECT id, create_type, create_time, create_min_user, enter_max_user, expire_time FROM world.maps WHERE create_type IN (3, 5, 6)",
    )
    .fetch_all(pool)
    .await?;
    let rules = rules
        .into_iter()
        .map(|(id, kind, times, min, max, expire)| {
            let mut times: Vec<u16> = times.unwrap_or_default().into_iter().map(|t| t as u16).collect();
            if kind == i16::from(crate::world::create_type::GUILD_RANKING) {
                // The ranking battle reads two entries (`docs/original/guild-ranking.md` section 2).
                times.truncate(6);
            }
            let rules = crate::world::InstanceRules::new(&times, min.max(0) as u32, max.max(0) as u32, expire.max(0) as u32);
            Ok((u16::try_from(id)?, rules))
        })
        .collect::<Result<_>>()?;
    world.set_instance_rules(rules);
    let rows: Vec<MapPortalRow> = sqlx::query_as(
        "SELECT map, pos_x, pos_y, pos_z, country, level_min, level_max, to_map, to_x, to_y, to_z FROM world.map_portals ORDER BY map, seq",
    )
    .fetch_all(pool)
    .await?;
    let mut portals: HashMap<u16, Vec<Portal>> = HashMap::new();
    for r in rows {
        portals.entry(u16::try_from(r.map)?).or_default().push(Portal {
            position: [r.pos_x, r.pos_y, r.pos_z],
            country: u32::try_from(r.country)?,
            level_min: u32::try_from(r.level_min)?,
            level_max: u32::try_from(r.level_max)?,
            to_map: u32::try_from(r.to_map)?,
            to: [r.to_x, r.to_y, r.to_z],
        });
    }
    world.set_portals(portals, create_types);

    let bitmaps: Vec<(i16, Vec<u8>)> = sqlx::query_as("SELECT id, blocked FROM world.maps").fetch_all(pool).await?;
    let blocked = bitmaps.into_iter().map(|(id, bits)| Ok((u16::try_from(id)?, bits))).collect::<Result<_>>()?;
    let rows: Vec<MobAreaRow> = sqlx::query_as(
        "SELECT a.map, a.seq, a.x1, a.y1, a.z1, a.x2, a.y2, a.z2, m.mob_id, m.count
           FROM world.mob_areas a JOIN world.mob_area_mobs m ON m.mob_area_id = a.id
          ORDER BY a.map, a.seq, m.seq",
    )
    .fetch_all(pool)
    .await?;
    let mut spawn_areas: HashMap<u16, Vec<SpawnArea>> = HashMap::new();
    let mut last = None;
    for r in rows {
        let map = u16::try_from(r.map)?;
        let entry = (u16::try_from(r.mob_id).unwrap_or(0), u32::try_from(r.count).unwrap_or(0));
        let areas = spawn_areas.entry(map).or_default();
        if last == Some((map, r.seq)) {
            areas.last_mut().expect("the area of the previous row").entries.push(entry);
        } else {
            areas.push(SpawnArea::new([r.x1, r.y1, r.z1, r.x2, r.y2, r.z2], vec![entry]));
            last = Some((map, r.seq));
        }
    }
    world.set_mob_data(blocked, spawn_areas);

    // Rebirth points: the common one for both factions, else each faction's (Map.ini, docs/original/mob-fight.md section 8).
    type PointRow = (i16, Option<i16>, Option<Vec<f32>>, Option<i16>, Option<Vec<f32>>, Option<i16>, Option<Vec<f32>>);
    let rows: Vec<PointRow> = sqlx::query_as(
        "SELECT id, rebirth_map, rebirth_pos, rebirth_light_map, rebirth_light_pos, rebirth_fury_map, rebirth_fury_pos FROM world.maps",
    )
    .fetch_all(pool)
    .await?;
    let point = |map: Option<i16>, pos: Option<Vec<f32>>| -> Option<(u16, [f32; 3])> {
        let map = u16::try_from(map?).ok()?;
        let pos: [f32; 3] = pos?.try_into().ok()?;
        Some((map, pos))
    };
    let mut rebirth = HashMap::new();
    for (id, common_map, common_pos, light_map, light_pos, fury_map, fury_pos) in rows {
        let common = point(common_map, common_pos);
        let points = [common.or_else(|| point(light_map, light_pos)), common.or_else(|| point(fury_map, fury_pos))];
        rebirth.insert(u16::try_from(id)?, points);
    }
    let rows: Vec<(i16, i32, Vec<f32>)> =
        sqlx::query_as("SELECT map, country, boxes FROM world.map_binds ORDER BY map, seq").fetch_all(pool).await?;
    let mut binds: HashMap<u16, Vec<BindArea>> = HashMap::new();
    for (map, country, boxes) in rows {
        let boxes = boxes.as_chunks::<6>().0.to_vec();
        binds.entry(u16::try_from(map)?).or_default().push(BindArea { country: u32::try_from(country).unwrap_or(0), boxes });
    }
    world.set_rebirth(rebirth, binds);
    Ok(world)
}

/// The monsters and the drop tables, as the server starts with them and as `reload` loads them again
/// (`docs/plans/done/data-editing.md` section 7): `defs.mobs` with the skills attached, then the tables of `items`' drop grades with the
/// boxes, quest links and sequence groups. `items` is the item definitions as loaded now (only their grades matter here). Also
/// returns how many monster attacks kept a skill. Nothing is partly loaded: any error ends the call.
pub async fn load_live(pool: &PgPool, items: &ItemDefs, skills: &SkillDefs) -> Result<(Live, usize)> {
    use anyhow::Context as _;
    let mut mobs = load_mob_defs(pool).await.context("loading the monster definitions")?;
    let mob_skills = mobs.attach_skills(skills);
    let mut drops = DropTables::new(items);
    let box_lines =
        load_box_lines(pool).await.context("loading the boxes (re-run tools/world/import.py if world.item_create is missing)")?;
    let box_sets = drops.load_boxes(&box_lines);
    tracing::info!(box_sets, "box sets loaded");
    let link_rows = load_quest_links(pool)
        .await
        .context("loading the item quest links (re-run tools/world/import.py if world.item_quest_links is missing)")?;
    let quest_links = drops.load_quest_links(&link_rows);
    let sequence_rows = load_sequence_groups(pool)
        .await
        .context("loading the sequence items (re-run tools/world/import.py if world.sequence_groups is missing)")?;
    let sequence_groups = drops.load_sequences(&sequence_rows);
    tracing::info!(quest_links, sequence_groups, "quest item links and sequence groups loaded");
    Ok((Live { mobs, drops }, mob_skills))
}

/// The boxes' sets of `world.item_create` in file order (`docs/original/item-origins.md` section 5.1); the rules apply in
/// `DropTables::load_boxes`.
pub async fn load_box_lines(pool: &PgPool) -> Result<Vec<crate::drops::BoxLine>> {
    Ok(sqlx::query_as("SELECT set_no, grade, weight FROM world.item_create ORDER BY set_no, seq").fetch_all(pool).await?)
}

/// The item -> quest links of `world.item_quest_links` (`docs/original/quest-drops.md` section 1.3); the rules apply in
/// `DropTables::load_quest_links`.
pub async fn load_quest_links(pool: &PgPool) -> Result<Vec<crate::drops::QuestLinkRow>> {
    Ok(sqlx::query_as("SELECT item_type, item_type_id, started, ended FROM world.item_quest_links ORDER BY item_type, item_type_id")
        .fetch_all(pool)
        .await?)
}

/// The sequence groups of `world.sequence_groups` (section 4.1), in group order; the rules apply in `DropTables::load_sequences`.
pub async fn load_sequence_groups(pool: &PgPool) -> Result<Vec<crate::drops::SequenceRow>> {
    Ok(sqlx::query_as("SELECT group_no, sequence, grades FROM world.sequence_groups ORDER BY group_no").fetch_all(pool).await?)
}

/// `world.guild_house` and its NPC tables (`docs/original/guild-house.md` section 2); the built-in values when the import has none.
async fn load_guild_house(pool: &PgPool) -> Result<crate::guild_house::HouseRules> {
    type Row = (i32, i32, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>, Vec<i32>, Vec<i32>);
    let row: Option<Row> = sqlx::query_as(
        "SELECT buy_money, keep_etin, gate_rates, shop_rates, potion_rates, repair_rates, mix_rates, mix_bonus, bank_slots
           FROM world.guild_house WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    let mut rules = crate::guild_house::HouseRules::default();
    let Some((buy, keep, gate, shop, potion, repair, mix, bonus, slots)) = row else {
        tracing::warn!("world.guild_house is empty: the built-in guild house values apply");
        return Ok(rules);
    };
    let unsigned = |v: Vec<i32>| v.into_iter().map(|x| x.max(0) as u32).collect::<Vec<u32>>();
    rules.buy_money = buy.max(0) as u32;
    rules.keep_etin = keep.max(0) as u32;
    (rules.gate_rates, rules.shop_rates, rules.potion_rates, rules.repair_rates, rules.mix_rates) = (gate, shop, potion, repair, mix);
    rules.mix_bonus = bonus;
    rules.bank_slots = unsigned(slots);
    let npcs: Vec<(i32, Vec<i32>, Vec<i32>)> =
        sqlx::query_as("SELECT npc_key, keep_etin, upgrade_etin FROM world.guild_house_npcs").fetch_all(pool).await?;
    rules.npcs = npcs
        .into_iter()
        .map(|(key, keep, upgrade)| {
            Ok((u16::try_from(key)?, crate::guild_house::NpcCosts { keep: unsigned(keep), upgrade: unsigned(upgrade) }))
        })
        .collect::<Result<_>>()?;
    let levels: Vec<(i16, i16, i16, i32)> =
        sqlx::query_as("SELECT country, kind, level, npc_type_id FROM world.guild_house_levels").fetch_all(pool).await?;
    rules.levels = levels
        .into_iter()
        .map(|(country, kind, level, npc)| Ok(((u8::try_from(country)?, u8::try_from(kind)?, u8::try_from(level)?), u16::try_from(npc)?)))
        .collect::<Result<_>>()?;
    Ok(rules)
}

/// The factions' kill-count grades of `world.kill_status` in file order (`docs/original/pvp-2.md` section 8); the rules apply in
/// `kc::Grades::new`.
pub async fn load_kill_status(pool: &PgPool) -> Result<Vec<crate::kc::Row>> {
    let rows: Vec<(i16, i64, Vec<i16>, Vec<i32>)> =
        sqlx::query_as("SELECT faction, kills, bonus_types, bonus_values FROM world.kill_status ORDER BY seq").fetch_all(pool).await?;
    rows.into_iter()
        .map(|(faction, kills, types, values)| {
            let mut bonuses = [(0, 0); 6];
            for (b, (t, v)) in bonuses.iter_mut().zip(types.into_iter().zip(values)) {
                *b = (u8::try_from(t)?, v);
            }
            Ok(crate::kc::Row { faction: u8::try_from(faction)?, kills: u32::try_from(kills)?, bonuses })
        })
        .collect()
}

/// The scripted monsters' AIs of `world.mob_ai`, parsed (`docs/original/scripted-mobs.md` section 2), and the lines of
/// `world.ai_strings` (the English where there is one, deviation 1). An AI that does not parse is logged and left out: its monsters
/// stay ordinary (the original does not start). Returns the library and the AIs left out.
pub async fn load_mob_ai(pool: &PgPool) -> Result<(crate::mob_ai::Library, Vec<(u8, String)>)> {
    let rows: Vec<(i16, String, String, String, String)> =
        sqlx::query_as("SELECT ai, alo, afs, ai_text, ah FROM world.mob_ai ORDER BY ai").fetch_all(pool).await?;
    let mut library = crate::mob_ai::Library::default();
    let mut failed = Vec::new();
    for (ai, alo, afs, text, ah) in rows {
        let Ok(ai) = u8::try_from(ai) else { continue };
        match crate::mob_ai::Machine::parse(&alo, &afs, &text, &ah) {
            Ok(machine) => {
                library.machines.insert(ai, machine);
            }
            Err(e) => failed.push((ai, e)),
        }
    }
    let lines: Vec<(i32, Vec<u8>, Option<String>)> =
        sqlx::query_as("SELECT id, text, english FROM world.ai_strings").fetch_all(pool).await?;
    library.lines = lines.into_iter().map(|(id, text, english)| (id, english.map_or(text, String::into_bytes))).collect();
    Ok((library, failed))
}

/// The Lua monsters' scripts (`world.mob_lua`, `docs/original/instances.md` section 9), the files' bytes by `AI` number.
pub async fn load_mob_lua(pool: &PgPool) -> Result<crate::mob_lua::Scripts> {
    let rows: Vec<(i16, Vec<u8>)> = sqlx::query_as("SELECT ai, source FROM world.mob_lua ORDER BY ai").fetch_all(pool).await?;
    Ok(crate::mob_lua::Scripts(rows.into_iter().filter_map(|(ai, source)| Some((u8::try_from(ai).ok()?, source))).collect()))
}

/// The obelisks of `world.obelisks` with their guard groups, in file order (`docs/original/obelisks.md` section 1); the rules apply
/// in `World::set_obelisks`.
pub async fn load_obelisks(pool: &PgPool) -> Result<Vec<crate::obelisk::FileObelisk>> {
    /// seq, map, number, x, y, z, the neutral, light and fury monsters, the file's country.
    type ObeliskRow = (i32, i32, i32, f32, f32, f32, i32, i32, i32, i32);
    /// The obelisk's seq, x, y, z, radius, the neutral, light and fury monsters, count.
    type GuardRow = (i32, f32, f32, f32, f32, i32, i32, i32, i32);
    let rows: Vec<ObeliskRow> = sqlx::query_as(
        "SELECT seq, map, number, pos_x, pos_y, pos_z, mob_neutral, mob_light, mob_fury, country FROM world.obelisks ORDER BY seq",
    )
    .fetch_all(pool)
    .await?;
    let guards: Vec<GuardRow> = sqlx::query_as(
        "SELECT obelisk_seq, pos_x, pos_y, pos_z, radius, mob_neutral, mob_light, mob_fury, count FROM world.obelisk_guards
         ORDER BY obelisk_seq, seq",
    )
    .fetch_all(pool)
    .await?;
    // A monster id the definitions cannot hold counts as none.
    let mob = |id: i32| u16::try_from(id).unwrap_or(0);
    let mut by_seq: HashMap<i32, Vec<crate::obelisk::FileGuards>> = HashMap::new();
    for (seq, x, y, z, radius, n, l, f, count) in guards {
        by_seq.entry(seq).or_default().push(crate::obelisk::FileGuards {
            centre: [x, y, z],
            radius,
            mobs: [mob(n), mob(l), mob(f)],
            count: u32::try_from(count).unwrap_or(0),
        });
    }
    rows.into_iter()
        .filter_map(|(seq, map, number, x, y, z, n, l, f, country)| {
            // A map or number out of range names no map the server has.
            let (map, number) = (u16::try_from(map).ok()?, u16::try_from(number).ok()?);
            Some(Ok(crate::obelisk::FileObelisk {
                map,
                number,
                position: [x, y, z],
                mobs: [mob(n), mob(l), mob(f)],
                country: u8::try_from(country).unwrap_or(0),
                guards: by_seq.remove(&seq).unwrap_or_default(),
            }))
        })
        .collect()
}

/// The boss groups of `world.boss_groups` with their follower groups, in file order (`docs/original/obelisks.md` section 11.1); the
/// rules apply in `World::set_boss_groups`.
pub async fn load_boss_groups(pool: &PgPool) -> Result<Vec<crate::boss::FileBossGroup>> {
    /// seq, number, map, respawn seconds, the portal's map and id, x, y, z, the monsters, the random minutes and spots.
    type GroupRow = (i32, i32, i32, i32, i32, i32, f32, f32, f32, Vec<i32>, Option<i32>, Option<i32>, Option<Vec<f32>>);
    /// The group's seq, x, y, z, radius, the monsters, count.
    type FollowerRow = (i32, f32, f32, f32, f32, Vec<i32>, i32);
    let rows: Vec<GroupRow> = sqlx::query_as(
        "SELECT seq, number, map, respawn_secs, portal_map, portal_id, pos_x, pos_y, pos_z, mobs, random_min, random_max, random_spots
         FROM world.boss_groups ORDER BY seq",
    )
    .fetch_all(pool)
    .await?;
    let followers: Vec<FollowerRow> =
        sqlx::query_as("SELECT group_seq, pos_x, pos_y, pos_z, radius, mobs, count FROM world.boss_followers ORDER BY group_seq, seq")
            .fetch_all(pool)
            .await?;
    // A monster id the definitions cannot hold counts as none.
    let mob = |id: &i32| u16::try_from(*id).unwrap_or(0);
    let mut by_seq: HashMap<i32, Vec<crate::boss::FileFollowers>> = HashMap::new();
    for (seq, x, y, z, radius, mobs, count) in followers {
        by_seq.entry(seq).or_default().push(crate::boss::FileFollowers {
            centre: [x, y, z],
            radius,
            mobs: mobs.iter().map(mob).collect(),
            count: u32::try_from(count).unwrap_or(0),
        });
    }
    Ok(rows
        .into_iter()
        .filter_map(|(seq, number, map, respawn, pmap, pid, x, y, z, mobs, min, max, spots)| {
            // A map out of range names no map the server has.
            let map = u16::try_from(map).ok()?;
            let random = match (min, max, spots) {
                (Some(min), Some(max), Some(s)) if s.len() >= 9 => Some((
                    u32::try_from(min).unwrap_or(0),
                    u32::try_from(max).unwrap_or(0),
                    [[s[0], s[1], s[2]], [s[3], s[4], s[5]], [s[6], s[7], s[8]]],
                )),
                _ => None,
            };
            Some(crate::boss::FileBossGroup {
                number: u32::try_from(number).unwrap_or(0),
                map,
                respawn_secs: u32::try_from(respawn).unwrap_or(0),
                portal: (u16::try_from(pmap).unwrap_or(u16::MAX), u32::try_from(pid).unwrap_or(0)),
                position: [x, y, z],
                mobs: mobs.iter().map(mob).collect(),
                random,
                followers: by_seq.remove(&seq).unwrap_or_default(),
            })
        })
        .collect())
}

/// Monster definitions (`defs.mobs`); the respawn time comes from `attackspecial3` (`docs/original/mobs.md` section 1).
pub async fn load_mob_defs(pool: &PgPool) -> Result<MobDefs> {
    let rows: Vec<MobRow> =
        sqlx::query_as(
            "SELECT mobid, level, hp, sp, mp, attackspecial3, ai, normaltime, normalstep, chasestep, exp, dex, luc, wis, defense, magic, attrib,
                    ARRAY[resiststate1, resiststate2, resiststate3, resiststate4, resiststate5, resiststate6, resiststate7, resiststate8,
                    resiststate9, resiststate10, resiststate11, resiststate12, resiststate13, resiststate14, resiststate15] AS resiststates,
                    ARRAY[resistskill1, resistskill2, resistskill3, resistskill4, resistskill5, resistskill6] AS resistskills,
                    size, attacktime3, attackattrib3, money1, money2, questitemid, chasetime, chaserange,
                    attacktype1, attackok1, attacktime1, attackrange1, attack1, attackplus1,
                    attacktype2, attackok2, attacktime2, attackrange2, attack2, attackplus2,
                    attacktype3, attackok3, attackrange3, attack3, attackplus3
               FROM defs.mobs",
        )
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        bail!("defs.mobs is empty; load the game definitions (tools/defs)");
    }
    let defs = rows
        .into_iter()
        .map(|r| {
            Ok(MobDef {
                id: u16::try_from(r.mobid)?,
                level: u16::try_from(r.level)?,
                hp: u32::try_from(r.hp)?,
                sp: u16::try_from(r.sp)?,
                mp: u16::try_from(r.mp)?,
                resist_states: std::array::from_fn(|i| r.resiststates.get(i).is_some_and(|&v| v != 0)),
                resist_skills: std::array::from_fn(|i| r.resistskills.get(i).map_or(0, |&v| v.max(0) as u16)),
                respawn: respawn_time(u8::try_from(r.attackspecial3).unwrap_or(0)),
                ai: u8::try_from(r.ai)?,
                normal_time: Duration::from_millis(u64::try_from(r.normaltime)?),
                normal_step: u8::try_from(r.normalstep)?,
                chase_step: u8::try_from(r.chasestep)?,
                exp: u16::try_from(r.exp)?,
                dex: u16::try_from(r.dex)?,
                luc: u16::try_from(r.luc)?,
                wis: u16::try_from(r.wis)?,
                defense: u16::try_from(r.defense)?,
                magic: u16::try_from(r.magic)?,
                attrib: u8::try_from(r.attrib)?,
                size: u8::try_from(r.size)?,
                defense_add: u32::try_from(r.attacktime3)?,
                country: mob_country(u8::try_from(r.attackattrib3)?),
                respawn_code: u8::try_from(r.attackspecial3).unwrap_or(0),
                money: u16::try_from(r.money1)?,
                money_add: u16::try_from(r.money2)?,
                quest_item: crate::mobs::quest_item_of(r.questitemid),
                drops: [(0, 0); 10],
                chase_time: Duration::from_millis(u64::try_from(r.chasetime)?),
                chase_range: u8::try_from(r.chaserange)?,
                attacks: [
                    attack_entry(r.attacktype1, r.attackok1, r.attacktime1, r.attackrange1, r.attack1, r.attackplus1)?,
                    attack_entry(r.attacktype2, r.attackok2, r.attacktime2, r.attackrange2, r.attack2, r.attackplus2)?,
                    attack_entry(r.attacktype3, r.attackok3, r.attacktime3, r.attackrange3, r.attack3, r.attackplus3)?,
                ],
                // nRecoverValue is the int bound to entry 3's damage word pair; the column holds it as is.
                recover: u32::try_from(r.attack3).unwrap_or(0),
                obelisk: r.attackplus3 != 0,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut defs = MobDefs::new(defs);
    let drops: Vec<(i16, i16, i16, i32)> =
        sqlx::query_as("SELECT mobid, itemorder, grade, droprate FROM defs.mob_items WHERE grade > 0").fetch_all(pool).await?;
    for (mob_id, entry, grade, rate) in drops {
        defs.set_drop(u16::try_from(mob_id)?, usize::try_from(entry)?, u16::try_from(grade)?, u32::try_from(rate)?);
    }
    Ok(defs)
}

/// An attack entry as `CGameData::LoadConstMob` keeps it: normal attacks reach at most 60 (`docs/original/mob-fight.md` section 7).
fn attack_entry(skill: i16, ok: i16, time: i32, range: i16, damage: i16, damage_add: i16) -> Result<MobAttack> {
    let skill = u16::try_from(skill)?;
    let range = u8::try_from(range)?;
    Ok(MobAttack {
        skill,
        info: None,
        ok: u8::try_from(ok)?,
        time: Duration::from_millis(u64::try_from(time).unwrap_or(0)),
        range: if skill == 0 { range.min(60) } else { range },
        damage: damage as u16,
        damage_add: damage_add as u16,
    })
}

#[derive(sqlx::FromRow)]
struct MobRow {
    mobid: i16,
    level: i16,
    hp: i32,
    sp: i16,
    mp: i16,
    resiststates: Vec<i16>,
    resistskills: Vec<i16>,
    attackspecial3: i16,
    ai: i16,
    normaltime: i32,
    normalstep: i16,
    chasestep: i16,
    exp: i16,
    dex: i16,
    luc: i16,
    wis: i16,
    defense: i16,
    magic: i16,
    attrib: i16,
    size: i16,
    attacktime3: i32,
    attackattrib3: i16,
    chasetime: i32,
    chaserange: i16,
    attacktype1: i16,
    attackok1: i16,
    attacktime1: i32,
    attackrange1: i16,
    attack1: i16,
    attackplus1: i16,
    attacktype2: i16,
    attackok2: i16,
    attacktime2: i32,
    attackrange2: i16,
    attack2: i16,
    attackplus2: i16,
    attacktype3: i16,
    attackok3: i16,
    attackrange3: i16,
    attack3: i16,
    attackplus3: i16,
    money1: i16,
    questitemid: i32,
    money2: i16,
}

#[derive(sqlx::FromRow)]
struct ItemDefRow {
    itemid: i32,
    reqog: i16,
    reqig: i16,
    reqvg: i16,
    item_type: i16,
    typeid: i16,
    count: i16,
    quality: i16,
    buy: i32,
    sell: i32,
    reqlevel: i16,
    country: i16,
    attackfighter: i16,
    defensefighter: i16,
    patrolrogue: i16,
    shootrogue: i16,
    attackmage: i16,
    defensemage: i16,
    grow: i16,
    special: i16,
    conststr: i16,
    constdex: i16,
    constrec: i16,
    constint: i16,
    constwis: i16,
    constluc: i16,
    consthp: i16,
    constmp: i16,
    constsp: i16,
    attacktime: i16,
    speed: i16,
    server: i16,
    reqwis: i16,
    reqluc: i16,
    reqstr: i16,
    reqdex: i16,
    slot: i16,
    exp: i16,
    effect1: i16,
    effect2: i16,
    effect3: i16,
    effect4: i16,
    range: i16,
    attrib: i16,
    grade: i16,
    drop: i16,
}

/// A character's quest: id, finished, success, counts, seconds left.
pub(super) type QuestRowSaved = (i16, bool, bool, Vec<i16>, Option<i32>);
/// A quest result: quest, seq, need monster (index, count), need item (index, count), exp, money, item type, type id.
type QuestResultRow = (i16, i16, i32, i16, i16, i16, i64, i64, i16, i16);

#[derive(sqlx::FromRow)]
struct QuestRow {
    id: i16,
    name: String,
    level_min: i32,
    level_max: i32,
    country: i16,
    grow: i16,
    jobs: Vec<i16>,
    prev_quest: i32,
    prev_quest_2: i32,
    prev_quest_3: i32,
    time_limit: i64,
    repeat_count: i64,
    start_type: i16,
    start_npc_type: i16,
    start_npc_id: i32,
    start_item_type: i16,
    start_item_id: i16,
    start_items: Vec<i16>,
    end_type: i16,
    end_npc_type: i16,
    end_npc_id: i32,
    end_items: Vec<i16>,
    pc_kill_count: i16,
    mob_kills: Vec<i32>,
    result_type: i16,
    result_user_select: i16,
}

/// The quests of `world.quests` and `world.quest_results` (`docs/original/quest.md` section 1). The ids must run from 1 without gaps,
/// since the original finds quest n at position n.
pub async fn load_quest_defs(pool: &PgPool) -> Result<QuestDefs> {
    let rows: Vec<QuestRow> = sqlx::query_as(
        "SELECT id, name, level_min, level_max, country, grow, jobs, prev_quest, prev_quest_2, prev_quest_3, time_limit, repeat_count,
                start_type, start_npc_type, start_npc_id, start_item_type, start_item_id, start_items, end_type, end_npc_type, end_npc_id,
                end_items, pc_kill_count, mob_kills, result_type, result_user_select
           FROM world.quests ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let results: Vec<QuestResultRow> = sqlx::query_as(
        "SELECT quest_id, seq, need_mob_id, need_mob_count, need_item_id, need_item_count, exp, money, item_type, item_type_id
           FROM world.quest_results ORDER BY quest_id, seq",
    )
    .fetch_all(pool)
    .await?;
    let small = |v: i64| -> Result<u8> { Ok(u8::try_from(v)?) };
    let triples = |v: &[i16]| -> Result<[(u8, u8, u8); 3]> {
        let b = |i: usize| -> Result<u8> { Ok(u8::try_from(*v.get(i).unwrap_or(&0))?) };
        Ok([(b(0)?, b(1)?, b(2)?), (b(3)?, b(4)?, b(5)?), (b(6)?, b(7)?, b(8)?)])
    };
    let mut defs = Vec::with_capacity(rows.len());
    for r in rows {
        let id = u16::try_from(r.id)?;
        if usize::from(id) != defs.len() + 1 {
            bail!("world.quests: quest {id} is not at its position {} (the ids must run from 1 without gaps)", defs.len() + 1);
        }
        let mut jobs = [false; 6];
        for (j, &v) in r.jobs.iter().take(6).enumerate() {
            jobs[j] = v != 0;
        }
        let mob = |i: usize| -> Result<(u16, u8)> {
            Ok((u16::try_from(*r.mob_kills.get(i * 2).unwrap_or(&0))?, u8::try_from(*r.mob_kills.get(i * 2 + 1).unwrap_or(&0))?))
        };
        defs.push(QuestDef {
            id,
            name: r.name.clone(),
            level_min: u16::try_from(r.level_min.max(0))?,
            level_max: u16::try_from(r.level_max.clamp(0, i32::from(u16::MAX)))?,
            country: small(i64::from(r.country))?,
            grow: small(i64::from(r.grow))?,
            jobs,
            prev: [u16::try_from(r.prev_quest)?, u16::try_from(r.prev_quest_2)?, u16::try_from(r.prev_quest_3)?],
            time_limit: u32::try_from(r.time_limit)?,
            repeatable: r.repeat_count != 0,
            start_type: small(i64::from(r.start_type))?,
            start_npc: (small(i64::from(r.start_npc_type))?, u16::try_from(r.start_npc_id)?),
            start_item: (small(i64::from(r.start_item_type))?, small(i64::from(r.start_item_id))?),
            start_items: triples(&r.start_items)?,
            end_type: small(i64::from(r.end_type))?,
            end_npc: (small(i64::from(r.end_npc_type))?, u16::try_from(r.end_npc_id)?),
            end_items: triples(&r.end_items)?,
            pc_kills: small(i64::from(r.pc_kill_count))?,
            mob_kills: [mob(0)?, mob(1)?],
            result_type: small(i64::from(r.result_type))?,
            user_select: small(i64::from(r.result_user_select))?,
            results: Default::default(),
        });
    }
    for (quest_id, seq, need_mob_id, need_mob_count, need_item_id, need_item_count, exp, money, item_type, item_type_id) in results {
        let Some(def) = usize::try_from(quest_id).ok().and_then(|q| q.checked_sub(1)).and_then(|q| defs.get_mut(q)) else { continue };
        let Some(result) = def.results.get_mut(usize::try_from(seq)?) else { continue };
        *result = QuestResult {
            need_mob: (small(i64::from(need_mob_id))?, small(i64::from(need_mob_count))?),
            need_item: (small(i64::from(need_item_id))?, small(i64::from(need_item_count))?),
            exp: u32::try_from(exp)?,
            money: u32::try_from(money)?,
            item: (small(i64::from(item_type))?, small(i64::from(item_type_id))?),
        };
    }
    Ok(QuestDefs::new(defs))
}

/// Skill definitions of levels 1 to 3 and 100 (`docs/original/skills.md` section 1: `arrow` is the aggro, `fixrange` "kept on death").
pub async fn load_skill_defs(pool: &PgPool) -> Result<SkillDefs> {
    let rows: Vec<SkillDefRow> = sqlx::query_as(
        "SELECT skillid, skilllevel, country, attackfighter, defensefighter, patrolrogue, shootrogue, attackmage, defensemage, reqlevel,
                grow, skillpoint, typeshow, typeeffect, typedetail, weapon1, weapon2, weaponvalue, bag,
                coalesce(abilitytype1, 0::smallint) AS abilitytype1, coalesce(abilityvalue1, 0::smallint) AS abilityvalue1,
                coalesce(abilitytype2, 0::smallint) AS abilitytype2, coalesce(abilityvalue2, 0::smallint) AS abilityvalue2,
                coalesce(abilitytype3, 0::smallint) AS abilitytype3, coalesce(abilityvalue3, 0::smallint) AS abilityvalue3,
                prevskillid, typeattack, ARRAY[needweapon1, needweapon2, needweapon3, needweapon4, needweapon5, needweapon6, needweapon7,
                needweapon8, needweapon9, needweapon10, needweapon11, needweapon12, needweapon13, needweapon14, needweapon15] AS needweapons,
                shield, sp, mp, readytime, resettime, attackrange, applyrange, statetype, attrtype, successtype, successvalue, targettype,
                multiattack, arrow, damagetype, damagehp, damagesp, damagemp, adddamagehp, adddamagesp, adddamagemp, healhp, healsp, healmp,
                keeptime, timedamagetype, timedamagehp, timedamagesp, timedamagemp, timehealhp, timehealsp, timehealmp, defencetype,
                defencevalue, limithp, fixrange, changetype, changelevel
           FROM defs.skills WHERE skilllevel BETWEEN 1 AND 3 OR skilllevel = 100",
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        bail!("defs.skills is empty; load the game definitions (tools/defs)");
    }
    let defs = rows
        .into_iter()
        .map(|r| {
            let u8 = |v: i16| u8::try_from(v);
            Ok(SkillDef {
                id: u16::try_from(r.skillid)?,
                level: u8(r.skilllevel)?,
                country: u8(r.country)?,
                jobs: [r.attackfighter, r.defensefighter, r.patrolrogue, r.shootrogue, r.attackmage, r.defensemage].map(|j| j != 0),
                grow: u8(r.grow)?,
                points: u8(r.skillpoint)?,
                required_level: u16::try_from(r.reqlevel)?,
                type_show: u8(r.typeshow)?,
                type_effect: u8(r.typeeffect)?,
                kind: u16::try_from(r.typedetail)?,
                weapons: [u8(r.weapon1)?, u8(r.weapon2)?],
                weapon_value: u8(r.weaponvalue)?,
                bags: u8(r.bag)?,
                abilities: [
                    (u8(r.abilitytype1)?, u16::try_from(r.abilityvalue1)?),
                    (u8(r.abilitytype2)?, u16::try_from(r.abilityvalue2)?),
                    (u8(r.abilitytype3)?, u16::try_from(r.abilityvalue3)?),
                ],
                prev_skill: u16::try_from(r.prevskillid)?,
                attack_kind: u8(r.typeattack)?,
                need_weapons: std::array::from_fn(|i| r.needweapons.get(i).is_some_and(|&w| w != 0)),
                need_shield: r.shield != 0,
                need_sp: u16::try_from(r.sp)?,
                need_mp: u16::try_from(r.mp)?,
                ready_time: u8(r.readytime)?,
                reset_time: u16::try_from(r.resettime)?,
                attack_range: u8(r.attackrange)?,
                apply_range: u8(r.applyrange)?,
                state_type: u8(r.statetype)?,
                element: u8(r.attrtype)?,
                success_type: u8(r.successtype)?,
                success_value: u8(r.successvalue)?,
                target_type: u8(r.targettype)?,
                multi_attack: u8(r.multiattack)?,
                aggro: u16::try_from(r.arrow)?,
                damage_type: u8(r.damagetype)?,
                damage: [u16::try_from(r.damagehp)?, u16::try_from(r.damagesp)?, u16::try_from(r.damagemp)?],
                add_damage: [u16::try_from(r.adddamagehp)?, u16::try_from(r.adddamagesp)?, u16::try_from(r.adddamagemp)?],
                heal: [u16::try_from(r.healhp)?, u16::try_from(r.healsp)?, u16::try_from(r.healmp)?],
                // dwKeepTime is unsigned: a few item rows hold values above 32767, read back negative.
                keep_time: u32::from(r.keeptime as u16),
                time_damage_type: u8(r.timedamagetype)?,
                time_damage: [u16::try_from(r.timedamagehp)?, u16::try_from(r.timedamagesp)?, u16::try_from(r.timedamagemp)?],
                time_heal: [u16::try_from(r.timehealhp)?, u16::try_from(r.timehealsp)?, u16::try_from(r.timehealmp)?],
                defense_type: u8(r.defencetype)?,
                defense_value: u8(r.defencevalue)?,
                limit_hp: u8(r.limithp)?,
                keep_on_death: r.fixrange == 1,
                change_type: u16::try_from(r.changetype)?,
                change_level: u16::try_from(r.changelevel)?,
                ext: Default::default(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(SkillDefs::new(defs))
}

#[derive(sqlx::FromRow)]
struct SkillDefRow {
    skillid: i16,
    skilllevel: i16,
    country: i16,
    attackfighter: i16,
    defensefighter: i16,
    patrolrogue: i16,
    shootrogue: i16,
    attackmage: i16,
    defensemage: i16,
    reqlevel: i16,
    grow: i16,
    skillpoint: i16,
    typeshow: i16,
    typeeffect: i16,
    typedetail: i16,
    weapon1: i16,
    weapon2: i16,
    weaponvalue: i16,
    bag: i16,
    abilitytype1: i16,
    abilityvalue1: i16,
    abilitytype2: i16,
    abilityvalue2: i16,
    abilitytype3: i16,
    abilityvalue3: i16,
    prevskillid: i16,
    typeattack: i16,
    needweapons: Vec<i16>,
    shield: i16,
    sp: i16,
    mp: i16,
    readytime: i16,
    resettime: i16,
    attackrange: i16,
    applyrange: i16,
    statetype: i16,
    attrtype: i16,
    successtype: i16,
    successvalue: i16,
    targettype: i16,
    multiattack: i16,
    arrow: i16,
    damagetype: i16,
    damagehp: i16,
    damagesp: i16,
    damagemp: i16,
    adddamagehp: i16,
    adddamagesp: i16,
    adddamagemp: i16,
    healhp: i16,
    healsp: i16,
    healmp: i16,
    keeptime: i16,
    timedamagetype: i16,
    timedamagehp: i16,
    timedamagesp: i16,
    timedamagemp: i16,
    timehealhp: i16,
    timehealsp: i16,
    timehealmp: i16,
    defencetype: i16,
    defencevalue: i16,
    limithp: i16,
    fixrange: i16,
    changetype: i16,
    changelevel: i16,
}

/// The enchant tables of `world.item_enchant`, the imported `Data/ItemEnchant.ini` (`docs/plans/enchant-table.md`): `None` without rows
/// (the caller keeps the built-in tables, as the original without the file); any other count than 21, or a step outside 0 to 20, is an
/// error naming the table.
pub async fn load_item_enchant(pool: &PgPool) -> Result<Option<crate::lapis::EnchantTable>> {
    let rows: Vec<(i16, i32, i32, i16, i16)> =
        sqlx::query_as("SELECT step, weapon_rate, armour_rate, weapon_add, armour_add FROM world.item_enchant ORDER BY step")
            .fetch_all(pool)
            .await?;
    enchant_table(&rows)
}

/// [`load_item_enchant`]'s rows as a table (`None` for no rows).
pub fn enchant_table(rows: &[(i16, i32, i32, i16, i16)]) -> Result<Option<crate::lapis::EnchantTable>> {
    use anyhow::Context as _;
    if rows.is_empty() {
        return Ok(None);
    }
    let rows = rows
        .iter()
        .map(|&(step, weapon_rate, armour_rate, weapon_add, armour_add)| {
            Ok(crate::lapis::EnchantRow {
                step: u8::try_from(step)?,
                weapon_rate: u32::try_from(weapon_rate)?,
                armour_rate: u32::try_from(armour_rate)?,
                weapon_add: u16::try_from(weapon_add)?,
                armour_add: u16::try_from(armour_add)?,
            })
        })
        .collect::<Result<Vec<_>>>()
        .context("world.item_enchant: a negative value")?;
    crate::lapis::EnchantTable::from_rows(&rows).map(Some).map_err(|e| anyhow::anyhow!("world.item_enchant: {e}"))
}

/// The item definitions shops and equipping need (`defs.items`; ps_game.exe copies these columns as they are).
pub async fn load_item_defs(pool: &PgPool) -> Result<ItemDefs> {
    let rows: Vec<ItemDefRow> = sqlx::query_as(
        "SELECT itemid, type AS item_type, typeid, count, quality, buy, sell, reqlevel, country,
                attackfighter, defensefighter, patrolrogue, shootrogue, attackmage, defensemage, grow, special,
                conststr, constdex, constrec, constint, constwis, constluc, consthp, constmp, constsp, attacktime, speed, server, reqwis,
                effect1, effect2, effect3, effect4, range, attrib, grade, drop, reqog, reqig, reqvg, reqluc, reqstr, reqdex, slot, exp
           FROM defs.items",
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        bail!("defs.items is empty; load the game definitions (tools/defs)");
    }
    let defs = rows
        .into_iter()
        .map(|r| {
            let jobs = [r.attackfighter, r.defensefighter, r.patrolrogue, r.shootrogue, r.attackmage, r.defensemage].map(|j| j != 0);
            Ok(ItemDef {
                item_id: u32::try_from(r.itemid)?,
                item_type: u8::try_from(r.item_type)?,
                type_id: u8::try_from(r.typeid)?,
                max_count: u8::try_from(r.count)?,
                quality: u16::try_from(r.quality)?,
                buy: u32::try_from(r.buy)?,
                sell: u32::try_from(r.sell)?,
                level: u16::try_from(r.reqlevel)?,
                country: u8::try_from(r.country)?,
                jobs,
                grow: u8::try_from(r.grow)?,
                special: u8::try_from(r.special)?,
                const_stats: [r.conststr, r.constdex, r.constrec, r.constint, r.constwis, r.constluc].map(|v| v.max(0) as u16),
                const_health: [r.consthp, r.constmp, r.constsp].map(|v| v.max(0) as u16),
                attack_time: u8::try_from(r.attacktime)?,
                speed: u8::try_from(r.speed)?,
                // CItem::CopyCraftExpassion: stItemInfo +0x4a (server) not 0 and +0x3e (reqwis) 1 to 99.
                craftable: r.server != 0 && (1..100).contains(&r.reqwis),
                effects: [r.effect1, r.effect2, r.effect3, r.effect4].map(|v| v.max(0) as u16),
                range: u16::try_from(r.range)?,
                attrib: u8::try_from(r.attrib)?,
                grade: u16::try_from(r.grade)?,
                drop_limit: u16::try_from(r.drop)?,
                server: u8::try_from(r.server)?,
                bound: r.reqog != 0,
                reuse_group: r.reqig.clamp(0, 255) as u8,
                vg: r.reqvg,
                room: r.slot.clamp(0, 255) as u8,
                // A u16 column kept as smallint: its 16 bits.
                material: r.reqluc as u16,
                craft_max: r.reqwis.max(0) as u16,
                req_str: r.reqstr as u16,
                req_dex: r.reqdex as u16,
                absorption: r.exp.clamp(0, 255) as u8,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ItemDefs::new(defs))
}
