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

pub struct Db {
    pub pool: PgPool,
    pub editor: Editor,
    pub s: Schemas,
}

/// What a fresh load of the original files looks like in a test.
#[derive(Clone, Copy, Default)]
pub struct Files {
    /// Monster 10 has 120 HP instead of 100, monster 11 is gone, and area 2/100000 exists.
    pub changed: bool,
}

pub async fn run(pool: &PgPool, text: String) {
    sqlx::raw_sql(AssertSqlSafe(text)).execute(pool).await.unwrap();
}

impl Db {
    pub async fn create() -> Option<Self> {
        dotenvy::dotenv().ok();
        let Ok(url) = std::env::var("DATABASE_URL") else {
            eprintln!("DATABASE_URL not set: skipping the PostgreSQL tests");
            return None;
        };
        let tag = format!("t{:08x}", rand::random::<u32>());
        let s = Schemas { defs: format!("{tag}_defs"), world: format!("{tag}_world"), edits: format!("{tag}_edits") };
        let pool = PgPoolOptions::new().max_connections(4).connect(&url).await.unwrap();
        run(
            &pool,
            EDITS_MIGRATION
                .replace("CREATE SCHEMA IF NOT EXISTS edits;", &format!("CREATE SCHEMA {};", s.edits))
                .replace("edits.", &format!("{}.", s.edits)),
        )
        .await;
        let db = Self { editor: Editor::new(pool.clone(), s.clone()), pool, s };
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
            run(
                &self.pool,
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
        let admin = PgPoolOptions::new().max_connections(1).connect(&std::env::var("DATABASE_URL").unwrap()).await.unwrap();
        for schema in [&self.s.defs, &self.s.world, &self.s.edits] {
            run(&admin, format!("DROP SCHEMA IF EXISTS {schema} CASCADE")).await;
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
