//! Database bootstrap: open the libSQL file, run migrations, seed the personal
//! tenant. Migrations are embedded at compile time so the binary runs correctly
//! regardless of the working directory it is launched from.

use crate::config::DEFAULT_WORKSPACE;
use crate::ids::now_secs;
use libsql::{Builder, Connection};

/// Migrations are baked into the binary. Paths are relative to this source file
/// (`services/lifeos-api/src/db.rs` -> repo-root `migrations/`).
const MIGRATION_CORE: &str = include_str!("../../../migrations/0001_core.sql");
const MIGRATION_CONTROL: &str = include_str!("../../../migrations/0002_control_plane.sql");

/// Open the DB, apply migrations, and seed the default workspace/user.
pub async fn connect(db_path: &str) -> Result<Connection, libsql::Error> {
    let db = Builder::new_local(db_path).build().await?;
    let conn = db.connect()?;
    run_migrations(&conn).await?;
    seed(&conn).await?;
    Ok(conn)
}

pub async fn run_migrations(conn: &Connection) -> Result<(), libsql::Error> {
    conn.execute_batch(MIGRATION_CORE).await?;
    conn.execute_batch(MIGRATION_CONTROL).await?;
    tracing::info!("migrations applied (core + control plane)");
    Ok(())
}

/// Idempotently seed the single personal workspace, its owner user, and the
/// membership joining them. Safe to call on every boot.
pub async fn seed(conn: &Connection) -> Result<(), libsql::Error> {
    let now = now_secs();

    // Billing catalog: the 'free' plan must exist because workspaces default to
    // plan='free'. Stub limits; SaaS later gates modules/quota off this JSON.
    let plan_exists = scalar_exists(conn, "SELECT 1 FROM plans WHERE id = ?1", "free").await?;
    if !plan_exists {
        tracing::info!("seeding default 'free' plan");
        conn.execute(
            "INSERT INTO plans (id, name, price_cents, currency, limits, created_at, updated_at) \
             VALUES ('free', 'Free', 0, 'usd', '{}', ?1, ?2)",
            libsql::params![now, now],
        )
        .await?;
    }

    let exists = scalar_exists(conn, "SELECT 1 FROM workspaces WHERE id = ?1", DEFAULT_WORKSPACE).await?;
    if !exists {
        tracing::info!("seeding default personal workspace");
        conn.execute(
            "INSERT INTO workspaces (id, name, plan, limits, created_at, updated_at) \
             VALUES (?1, ?2, 'free', '{}', ?3, ?4)",
            libsql::params![DEFAULT_WORKSPACE, "Personal Workspace", now, now],
        )
        .await?;
    }

    let user_exists = scalar_exists(conn, "SELECT 1 FROM users WHERE email = ?1", "chayan@lifeos.app").await?;
    if !user_exists {
        tracing::info!("seeding default user + membership");
        conn.execute(
            "INSERT INTO users (id, email, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            libsql::params!["usr_chayan", "chayan@lifeos.app", "Chayan Aggarwal", now, now],
        )
        .await?;
        conn.execute(
            "INSERT INTO memberships (id, user_id, workspace_id, role, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'owner', ?4, ?5)",
            libsql::params!["memb_default", "usr_chayan", DEFAULT_WORKSPACE, now, now],
        )
        .await?;
    }

    Ok(())
}

/// True if the given single-param query returns at least one row.
async fn scalar_exists(conn: &Connection, sql: &str, param: &str) -> Result<bool, libsql::Error> {
    let mut rows = conn.query(sql, libsql::params![param]).await?;
    Ok(rows.next().await?.is_some())
}

/// True if a workspace row exists. Used to validate tenant scope before writes.
pub async fn workspace_exists(conn: &Connection, workspace_id: &str) -> Result<bool, libsql::Error> {
    scalar_exists(conn, "SELECT 1 FROM workspaces WHERE id = ?1", workspace_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh(path: &str) -> Connection {
        let _ = std::fs::remove_file(path);
        connect(path).await.unwrap()
    }

    #[tokio::test]
    async fn migrations_and_seed_are_idempotent() {
        let path = "test_db_seed.db";
        let conn = fresh(path).await;
        // Second connect must not error (idempotent seed).
        run_migrations(&conn).await.unwrap();
        seed(&conn).await.unwrap();

        assert!(workspace_exists(&conn, DEFAULT_WORKSPACE).await.unwrap());

        let mut rows = conn
            .query("SELECT name FROM users WHERE email = ?1", libsql::params!["chayan@lifeos.app"])
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("user seeded");
        let name: String = row.get(0).unwrap();
        assert_eq!(name, "Chayan Aggarwal");

        // Acceptance (#2): seed creates exactly one workspace, one user, one owner
        // membership - even after a second migrate+seed pass.
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM workspaces").await, 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM users").await, 1);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM memberships WHERE role='owner'").await, 1);
        // Billing catalog seeded with the 'free' plan (control-plane stub).
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM plans WHERE id='free'").await, 1);

        let _ = std::fs::remove_file(path);
    }

    /// Helper: run a `SELECT COUNT(*)` and return the integer.
    async fn count(conn: &Connection, sql: &str) -> i64 {
        let mut rows = conn.query(sql, ()).await.unwrap();
        rows.next().await.unwrap().unwrap().get(0).unwrap()
    }

    #[tokio::test]
    async fn annotations_table_and_due_generated_column_exist() {
        let path = "test_db_schema.db";
        let conn = fresh(path).await;

        // annotations table (spec §2.4) accepts a workspace-scoped note.
        conn.execute(
            "INSERT INTO annotations (id, workspace_id, entity_id, kind, body, created_by, created_at, updated_at) \
             VALUES ('anno_1', ?1, 'ent_1', 'note', 'hello', 'user', 1, 1)",
            libsql::params![DEFAULT_WORKSPACE],
        )
        .await
        .unwrap();

        // `due` is a GENERATED VIRTUAL column lifted from attrs (§7); it must be
        // queryable and reflect json_extract(attrs,'$.due') without an explicit write.
        conn.execute(
            "INSERT INTO entities (id, workspace_id, module, type, attrs, created_at, updated_at) \
             VALUES ('ent_1', ?1, 'tasks', 'task', '{\"due\": 1700000000}', 1, 1)",
            libsql::params![DEFAULT_WORKSPACE],
        )
        .await
        .unwrap();

        let mut rows = conn
            .query("SELECT due FROM entities WHERE id = 'ent_1'", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().expect("entity row");
        let due: i64 = row.get(0).unwrap();
        assert_eq!(due, 1_700_000_000);

        let _ = std::fs::remove_file(path);
    }
}
