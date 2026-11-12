//! Shared by the PostgreSQL tests: throwaway schemas (a scratch `defs`, `world` and `edits` made from the repository's own schema files,
//! with a few rows), dropped afterwards. Skipped (and says so) when DATABASE_URL is not set.

#![allow(dead_code)]

use data_editor::registry::Schemas;
use data_editor::store::{Editor, Line, Op, Refused};
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};

const DEFS_SCHEMA: &str = include_str!("../../../../db/defs/schema.sql");
const WORLD_SCHEMA: &str = include_str!("../../../../db/world/schema.sql");
const EDITS_MIGRATION: &str = include_str!("../../../../db/migrations/0030_data_edits.sql");
const CONTENT_MIGRATION: &str = include_str!("../../../../db/migrations/0031_content.sql");
const RULES_MIGRATION: &str = include_str!("../../../../db/migrations/0032_rules.sql");
const SCRIPTS_MIGRATION: &str = include_str!("../../../../db/migrations/0033_scripts.sql");
const EFFECTS_MIGRATION: &str = include_str!("../../../../db/migrations/0034_effects.sql");
const PETS_MIGRATION: &str = include_str!("../../../../db/migrations/0036_pets.sql");

pub struct Db {
    pub pool: PgPool,
    pub editor: Editor,
    pub s: Schemas,
    /// The schema of the game's own tables, with [`Db::create_full`].
    pub pub_schema: Option<String>,
}

/// What a fresh load of the original files looks like in a test.
#[derive(Clone, Copy, Default)]
pub struct Files {
    /// Monster 10 has 120 HP instead of 100, monster 11 is gone, and area 2/100000 exists.
    pub changed: bool,
}

/// Drops the NOT NULL of every column outside the primary key of the tables of a schema, so that a test inserts the few columns it cares about.
pub async fn relax(pool: &PgPool, schema: &str) {
    run(
        pool,
        format!(
            "DO $$ DECLARE r record; BEGIN
               FOR r IN SELECT n.nspname, c.relname, a.attname FROM pg_attribute a JOIN pg_class c ON c.oid = a.attrelid JOIN pg_namespace n ON n.oid = c.relnamespace
                        WHERE n.nspname = '{schema}' AND c.relkind = 'r' AND a.attnum > 0 AND NOT a.attisdropped AND a.attnotnull
                          AND NOT EXISTS (SELECT 1 FROM pg_index i WHERE i.indrelid = c.oid AND i.indisprimary AND a.attnum = ANY (i.indkey))
               LOOP EXECUTE format('ALTER TABLE %I.%I ALTER COLUMN %I DROP NOT NULL', r.nspname, r.relname, r.attname); END LOOP;
             END $$"
        ),
    )
    .await;
}

pub async fn run(pool: &PgPool, text: String) {
    sqlx::raw_sql(AssertSqlSafe(text)).execute(pool).await.unwrap();
}

impl Db {
    pub async fn create() -> Option<Self> {
        Self::create_with(false).await
    }

    /// As [`Db::create`], and the editor's connections also see a throwaway copy of the game's own tables (`character_items` …), made by
    /// running every migration in a schema of its own: what the holder and reference checks of `ext remove` read.
    pub async fn create_full() -> Option<Self> {
        Self::create_with(true).await
    }

    async fn create_with(full: bool) -> Option<Self> {
        dotenvy::dotenv().ok();
        let Ok(url) = std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")) else {
            eprintln!("DATABASE_URL not set: skipping the PostgreSQL tests");
            return None;
        };
        let tag = format!("t{:08x}", rand::random::<u32>());
        let s = Schemas {
            defs: format!("{tag}_defs"),
            world: format!("{tag}_world"),
            edits: format!("{tag}_edits"),
            content: format!("{tag}_content"),
            original_defs: format!("{tag}_original_defs"),
            original_world: format!("{tag}_original_world"),
            client: format!("{tag}_client"),
            ext_prefix: format!("{tag}_ext_"),
        };
        let pub_schema = format!("{tag}_pub");
        let pool = if full {
            let admin = PgPoolOptions::new().max_connections(1).connect(&url).await.unwrap();
            run(&admin, format!("CREATE SCHEMA {pub_schema}")).await;
            let path = pub_schema.clone();
            let pool = PgPoolOptions::new()
                .max_connections(4)
                .after_connect(move |conn, _| {
                    let path = path.clone();
                    Box::pin(async move {
                        sqlx::query(AssertSqlSafe(format!("SET search_path TO {path}"))).execute(conn).await?;
                        Ok(())
                    })
                })
                .connect(&url)
                .await
                .unwrap();
            sqlx::migrate!("../../db/migrations").run(&pool).await.unwrap();
            relax(&pool, &pub_schema).await;
            pool
        } else {
            PgPoolOptions::new().max_connections(4).connect(&url).await.unwrap()
        };
        run(
            &pool,
            EDITS_MIGRATION
                .replace("CREATE SCHEMA IF NOT EXISTS edits;", &format!("CREATE SCHEMA {};", s.edits))
                .replace("edits.", &format!("{}.", s.edits)),
        )
        .await;
        run(
            &pool,
            CONTENT_MIGRATION
                .replace("CREATE SCHEMA IF NOT EXISTS content;", &format!("CREATE SCHEMA {};", s.content))
                .replace("content.", &format!("{}.", s.content)),
        )
        .await;
        run(&pool, RULES_MIGRATION.replace("content.", &format!("{}.", s.content))).await;
        // Without the game's own tables (`characters`) the stored values have nothing to point at.
        let scripts = if full {
            SCRIPTS_MIGRATION.to_owned()
        } else {
            SCRIPTS_MIGRATION.replace(" REFERENCES characters (id) ON DELETE CASCADE", "")
        };
        run(&pool, scripts.replace("content.", &format!("{}.", s.content))).await;
        run(&pool, EFFECTS_MIGRATION.replace("content.", &format!("{}.", s.content))).await;
        // The pets' migration without its game table (`character_pets` is the game's, and made by the full migrations when they run).
        let pets = PETS_MIGRATION.split("CREATE TABLE IF NOT EXISTS character_pets").next().unwrap_or_default();
        run(&pool, pets.replace("content.", &format!("{}.", s.content))).await;
        let db = Self { editor: Editor::new(pool.clone(), s.clone()), pool, s, pub_schema: full.then_some(pub_schema) };
        db.load(Files::default()).await;
        Some(db)
    }

    /// Drops and recreates the scratch `defs` and `world` as a load of the original files does (no markers).
    pub async fn load(&self, files: Files) {
        let s = &self.s;
        run(&self.pool, format!("DROP SCHEMA IF EXISTS {} CASCADE; DROP SCHEMA IF EXISTS {} CASCADE", s.defs, s.world)).await;
        run(
            &self.pool,
            DEFS_SCHEMA
                .replace("CREATE SCHEMA IF NOT EXISTS defs;", &format!("CREATE SCHEMA {};", s.defs))
                .replace("defs.", &format!("{}.", s.defs)),
        )
        .await;
        run(
            &self.pool,
            WORLD_SCHEMA
                .replace("CREATE SCHEMA world;", &format!("CREATE SCHEMA {};", s.world))
                .replace("world.", &format!("{}.", s.world)),
        )
        .await;
        for schema in [&s.defs, &s.world] {
            relax(&self.pool, schema).await;
        }
        let (d, w) = (&s.defs, &s.world);
        let hp10 = if files.changed { 120 } else { 100 };
        run(&self.pool, format!("INSERT INTO {d}.mobs (rowid, mobid, mobname, level, hp, exp, money1, money2) VALUES (1, 10, 'Rat', 5, {hp10}, 3, 5, 3), (3, 2133, 'Boss', 50, 5000, 900, 100, 50), (4, 12, 'Ghost', 1, 0, 1, 0, 0)")).await;
        if !files.changed {
            run(
                &self.pool,
                format!(
                    "INSERT INTO {d}.mobs (rowid, mobid, mobname, level, hp, exp, money1, money2) VALUES (2, 11, 'Bat', 6, 200, 4, 6, 2)"
                ),
            )
            .await;
        }
        for order in 0..=9 {
            run(
                &self.pool,
                format!(
                    "INSERT INTO {d}.mob_items (rowid, mobid, itemorder, grade, droprate) VALUES ({order}, 2133, {order}, {}, {})",
                    order * 10,
                    order * 5
                ),
            )
            .await;
        }
        run(
            &self.pool,
            format!("INSERT INTO {d}.items (rowid, itemid, itemname, type, typeid, grade) VALUES (1, 30071, 'Sword', 30, 71, 5)"),
        )
        .await;
        run(
            &self.pool,
            format!("INSERT INTO {w}.maps (id, size, blocked, source_file) VALUES (1, 1000, '\\x00', 'x'), (2, 1024, '\\x00', 'x')"),
        )
        .await;
        run(&self.pool, format!("INSERT INTO {w}.mob_areas (map, seq, x1, y1, z1, x2, y2, z2) VALUES (2, 57, 10, 0, 10, 20, 0, 20), (2, 58, 30, 0, 30, 40, 0, 40)")).await;
        run(
            &self.pool,
            format!("INSERT INTO {w}.mob_area_mobs (mob_area_id, seq, mob_id, count) SELECT id, 1000, 10, 3 FROM {w}.mob_areas WHERE seq = 57 UNION ALL SELECT id, 1001, 2133, 1 FROM {w}.mob_areas WHERE seq = 57"),
        )
        .await;
        run(
            &self.pool,
            format!(
                "INSERT INTO {w}.map_portals (map, seq, pos_x, pos_y, pos_z, country, level_min, level_max, to_map, to_x, to_y, to_z)
                 VALUES (2, 0, 1, 0, 1, 0, 0, 0, 1, 5, 0, 5), (2, 1, 2, 0, 2, 1005, 0, 0, 1, 6, 0, 6)"
            ),
        )
        .await;
        run(&self.pool, format!("INSERT INTO {w}.obelisks (seq, map, number, pos_x, pos_y, pos_z, mob_neutral, mob_light, mob_fury, country) VALUES (1, 2, 1, 100, 0, 100, 10, 10, 10, 0)")).await;
        run(&self.pool, format!("INSERT INTO {w}.obelisk_guards (obelisk_seq, seq, pos_x, pos_y, pos_z, radius, mob_neutral, mob_light, mob_fury, count) VALUES (1, 1, 100, 0, 100, 5, 10, 10, 10, 2)")).await;
        run(
            &self.pool,
            format!(
                "INSERT INTO {w}.boss_groups (seq, number, map, stages, respawn_secs, portal_map, portal_id, pos_x, pos_y, pos_z, mobs) VALUES (1, 1, 2, 2, 60, 0, 0, 50, 0, 50, '{{10,2133}}');
                 INSERT INTO {w}.boss_followers (group_seq, seq, pos_x, pos_y, pos_z, radius, mobs, count) VALUES (1, 1, 50, 0, 50, 3, '{{0,10}}', 1)"
            ),
        )
        .await;
        if files.changed {
            run(
                &self.pool,
                format!("INSERT INTO {w}.mob_areas (map, seq, x1, y1, z1, x2, y2, z2) VALUES (2, 100000, 500, 0, 500, 510, 0, 510)"),
            )
            .await;
        }
    }

    pub async fn drop(self) {
        self.pool.close().await;
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("TEST_DATABASE_URL").or_else(|_| std::env::var("DATABASE_URL")).unwrap())
            .await
            .unwrap();
        let s = &self.s;
        for schema in [&s.defs, &s.world, &s.edits, &s.content, &s.original_defs, &s.original_world, &s.client] {
            run(&admin, format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).await;
        }
        if let Some(schema) = &self.pub_schema {
            run(&admin, format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).await;
        }
        let ext: Vec<String> = sqlx::query_scalar("SELECT nspname::text FROM pg_namespace WHERE nspname LIKE $1")
            .bind(format!("{}%", s.ext_prefix.replace('_', "\\_")))
            .fetch_all(&admin)
            .await
            .unwrap();
        for schema in ext {
            run(&admin, format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).await;
        }
    }

    /// The rows an extension like the Kill Boost Stone works on, in the live `defs` (before [`Db::freeze`]): the Mini Sun EXP Stone
    /// 100/108 and its buff 250/1, the empty placeholders 100/212 to 214 and 200/1 to 3, and a mall product.
    pub async fn seed_kill_boost(&self) {
        let d = &self.s.defs;
        run(&self.pool, format!(
            "INSERT INTO {d}.items (rowid, itemid, itemname, type, typeid, reqlevel, country, grow, special, slot, quality, buy, sell, grade, drop, server, count, range, attacktime)
             VALUES (10, 100108, 'Mini Sun EXP Stone', 100, 108, 1, 2, 0, 0, 0, 0, 100, 10, 3, 0, 0, 1, 250, 1),
                    (11, 100212, '???177', 100, 212, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0),
                    (12, 100213, '???177', 100, 213, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0),
                    (13, 100214, '???177', 100, 214, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0)")).await;
        run(&self.pool, format!(
            "INSERT INTO {d}.skills (rowid, skillid, skilllevel, skillname, keeptime, fixrange, abilitytype1, abilityvalue1, abilitytype2, abilityvalue2, abilitytype3, abilityvalue3, typedetail, updatedate)
             VALUES (50, 250, 1, 'Mini Sun EXP', 2, 0, 5, 100, 6, 7, 0, 0, 82, '2012-03-04 05:06:07'),
                    (51, 200, 1, '??', 0, 0, 0, 0, 0, 0, 0, 0, 0, '2001-01-01 00:00:01'),
                    (52, 200, 2, '??', 0, 0, 0, 0, 0, 0, 0, 0, 0, '2001-01-01 00:00:02'),
                    (53, 200, 3, '??', 0, 0, 0, 0, 0, 0, 0, 0, 0, '2001-01-01 00:00:03')")).await;
        run(&self.pool, format!(
            "INSERT INTO {d}.product_list (rowid, productname, productcode, buycost, itemid1, itemcount1) VALUES (1, 'Stone Pack', 'P1', 100, 100108, 1)")).await;
    }

    /// A frozen `original_defs` made from the real DDL (every column NOT NULL) with two rows in `exp_defs`, recorded as imported: what a
    /// live schema made from the DDL can be filled from.
    pub async fn freeze_from_ddl(&self) {
        let s = &self.s;
        let ddl = DEFS_SCHEMA
            .replace("CREATE SCHEMA IF NOT EXISTS defs;", &format!("CREATE SCHEMA {};", s.original_defs))
            .replace("defs.", &format!("{}.", s.original_defs));
        run(&self.pool, format!("DROP SCHEMA IF EXISTS {} CASCADE", s.original_defs)).await;
        run(&self.pool, ddl).await;
        run(
            &self.pool,
            format!("INSERT INTO {}.exp_defs (rowid, level, grow, exp) VALUES (1, 1, 0, 100), (2, 2, 0, 250)", s.original_defs),
        )
        .await;
        let mut conn = self.pool.acquire().await.unwrap();
        let mut recorded = serde_json::Map::new();
        for table in data_editor::content::tables_of(&mut conn, &s.original_defs).await.unwrap() {
            let (rows, md5) = data_editor::content::table_hash(&mut conn, &s.original_defs, &table).await.unwrap();
            recorded.insert(table, json!({"rows": rows, "md5": md5}));
        }
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO {}.imports (part, source, files, tables) VALUES ('defs', 'test', '{{}}', $1)",
            s.content
        )))
        .bind(Value::Object(recorded))
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// An extension row: the extension (made when missing) owns `tbl` `key`, written as `action` with the whole row `row`.
    pub async fn ext_row(&self, extension: &str, tbl: &str, key: &str, action: &str, row: Value) {
        let c = &self.s.content;
        run(&self.pool, format!("INSERT INTO {c}.extensions (name, version, title, description, manifest) VALUES ('{extension}', '1.0.0', 't', 'd', '{{}}') ON CONFLICT DO NOTHING")).await;
        let seq: i64 = self.count(format!("SELECT count(*) FROM {c}.ext_rows")).await;
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO {c}.ext_rows (extension, tbl, key, action, row, seq) VALUES ($1, $2, $3, $4, $5, {seq})"
        )))
        .bind(extension)
        .bind(tbl)
        .bind(key)
        .bind(action)
        .bind(row)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    /// "Imports" what the scratch `defs` and `world` hold now: copies them as the frozen `original_defs` / `original_world` and records
    /// the import in `content.imports` as the importers do (`part` is `defs`, `world` or `both`).
    pub async fn freeze(&self, part: &str) {
        let s = &self.s;
        let parts: Vec<(&str, &String, &String)> = [("defs", &s.defs, &s.original_defs), ("world", &s.world, &s.original_world)]
            .into_iter()
            .filter(|(p, _, _)| part == "both" || part == *p)
            .collect();
        for (name, live, frozen) in parts {
            run(&self.pool, format!("DROP SCHEMA IF EXISTS {frozen} CASCADE; CREATE SCHEMA {frozen}")).await;
            let mut conn = self.pool.acquire().await.unwrap();
            let mut recorded = serde_json::Map::new();
            for table in data_editor::content::tables_of(&mut conn, live).await.unwrap() {
                run(&self.pool, format!("CREATE TABLE {frozen}.{table} (LIKE {live}.{table} INCLUDING ALL); INSERT INTO {frozen}.{table} SELECT * FROM {live}.{table}")).await;
                let (rows, md5) = data_editor::content::table_hash(&mut conn, frozen, &table).await.unwrap();
                recorded.insert(table, json!({"rows": rows, "md5": md5}));
            }
            sqlx::query(AssertSqlSafe(format!("INSERT INTO {}.imports (part, source, files, tables) VALUES ($1, 'test', '{{}}', $2) ON CONFLICT (part) DO UPDATE SET tables = EXCLUDED.tables, rebuilt_at = NULL", s.content)))
                .bind(name)
                .bind(Value::Object(recorded))
                .execute(&self.pool)
                .await
                .unwrap();
        }
    }

    pub async fn scalar(&self, query: String) -> Value {
        sqlx::query_scalar::<_, Value>(AssertSqlSafe(format!("SELECT to_jsonb(x) FROM ({query}) x"))).fetch_one(&self.pool).await.unwrap()
    }

    /// One value of a table: `value("defs", "mobs", "hp", "mobid = 10")`.
    pub async fn value(&self, table: &str, column: &str, filter: &str) -> Value {
        let schema = if table.starts_with("mob_area") || table == "map_portals" || table.starts_with("obelisk") || table.starts_with("boss")
        {
            &self.s.world
        } else {
            &self.s.defs
        };
        let row = self.scalar(format!("SELECT {column} AS v FROM {schema}.{table} WHERE {filter}")).await;
        row["v"].clone()
    }

    /// One text value of a fully named table: `value_text("content.extensions", "version", "name = 'kb'")`.
    pub async fn value_text(&self, table: &str, column: &str, filter: &str) -> String {
        sqlx::query_scalar::<_, String>(AssertSqlSafe(format!("SELECT {column}::text FROM {table} WHERE {filter}")))
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    pub async fn count(&self, query: String) -> i64 {
        sqlx::query_scalar(AssertSqlSafe(query)).fetch_one(&self.pool).await.unwrap()
    }

    pub async fn draft(&self, line: Line) -> i32 {
        self.editor.draft_add(&line, "tester").await.unwrap()
    }

    pub async fn apply(&self, message: &str) -> data_editor::store::Applied {
        self.editor.apply_draft(message, "tester").await.unwrap()
    }
}

pub fn set(target: &str, key: &str, field: &str, value: Value) -> Line {
    Line { target: target.into(), key: key.into(), op: Op::Set { field: field.into(), value } }
}

pub fn unset(target: &str, key: &str, field: &str) -> Line {
    Line { target: target.into(), key: key.into(), op: Op::Unset { field: field.into() } }
}

pub fn add(target: &str, key: &str, fields: Value) -> Line {
    Line { target: target.into(), key: key.into(), op: Op::Add { fields } }
}

pub fn remove(target: &str, key: &str) -> Line {
    Line { target: target.into(), key: key.into(), op: Op::Remove }
}

pub fn area_fields(x: f64) -> Value {
    json!({"x1": x, "y1": 0, "z1": x, "x2": x + 10.0, "y2": 0, "z2": x + 10.0})
}

pub fn refusal(error: anyhow::Error) -> String {
    error.downcast::<Refused>().expect("a refusal, not a database failure").0
}
