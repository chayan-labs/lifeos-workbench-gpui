use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::{Any, CorsLayer};
use libsql::{Builder, Connection};

#[derive(Serialize, Deserialize)]
struct HealthStatus {
    status: String,
    workspace_id: String,
}

struct AppState {
    conn: Arc<Connection>,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Initialize database
    let db_path = "lifeos.db";
    let db = Builder::new_local(db_path).build().await.unwrap();
    let conn = Arc::new(db.connect().unwrap());

    // Execute migrations
    execute_migrations(&conn).await;

    // Seed default workspace and user
    seed_database(&conn).await;

    // Set up CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build State
    let state = Arc::new(AppState { conn });

    // Build API router
    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/entity", post(entity_create_handler).get(entity_query_handler))
        .route("/api/module-request", post(module_request_handler))
        .route("/api/register", post(register_handler))
        .layer(cors)
        .with_state(state);

    // Bind to localhost
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::info!("Life OS local API listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn execute_migrations(conn: &Connection) {
    let m1 = std::fs::read_to_string("../../migrations/0001_core.sql")
        .or_else(|_| std::fs::read_to_string("migrations/0001_core.sql"))
        .or_else(|_| std::fs::read_to_string("../migrations/0001_core.sql"))
        .expect("Failed to read migrations/0001_core.sql");
        
    let m2 = std::fs::read_to_string("../../migrations/0002_control_plane.sql")
        .or_else(|_| std::fs::read_to_string("migrations/0002_control_plane.sql"))
        .or_else(|_| std::fs::read_to_string("../migrations/0002_control_plane.sql"))
        .expect("Failed to read migrations/0002_control_plane.sql");

    conn.execute_batch(&m1).await.expect("Failed to run 0001_core migration");
    conn.execute_batch(&m2).await.expect("Failed to run 0002_control_plane migration");
    tracing::info!("Database migrations executed successfully.");
}

async fn seed_database(conn: &Connection) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    // Check if default workspace exists
    let mut stmt = conn
        .prepare("SELECT id FROM workspaces WHERE id = ?1")
        .await
        .unwrap();
    let mut rows = stmt.query(libsql::params!["default-personal-workspace"]).await.unwrap();
    if rows.next().await.unwrap().is_none() {
        tracing::info!("Seeding default personal workspace...");
        conn.execute(
            "INSERT INTO workspaces (id, name, plan, limits, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            libsql::params![
                "default-personal-workspace",
                "Personal Workspace",
                "free",
                "{}",
                now,
                now
            ],
        )
        .await
        .unwrap();
    }

    // Check if default user exists
    let mut stmt = conn
        .prepare("SELECT id FROM users WHERE email = ?1")
        .await
        .unwrap();
    let mut rows = stmt.query(libsql::params!["chayan@lifeos.app"]).await.unwrap();
    if rows.next().await.unwrap().is_none() {
        tracing::info!("Seeding default user...");
        conn.execute(
            "INSERT INTO users (id, email, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            libsql::params![
                "usr_chayan",
                "chayan@lifeos.app",
                "Chayan Aggarwal",
                now,
                now
            ],
        )
        .await
        .unwrap();

        // Seed membership
        conn.execute(
            "INSERT INTO memberships (id, user_id, workspace_id, role, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            libsql::params![
                "memb_default",
                "usr_chayan",
                "default-personal-workspace",
                "owner",
                now,
                now
            ],
        )
        .await
        .unwrap();
    }
}

async fn health_handler(State(state): State<Arc<AppState>>) -> Result<Json<HealthStatus>, axum::http::StatusCode> {
    let mut stmt = state.conn
        .prepare("SELECT id FROM workspaces LIMIT 1")
        .await
        .map_err(|e| {
            tracing::error!("Database health check prepare failed: {:?}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let mut rows = stmt.query(()).await
        .map_err(|e| {
            tracing::error!("Database health check query failed: {:?}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let workspace_id = if let Some(row) = rows.next().await.unwrap() {
        row.get::<String>(0).unwrap_or_else(|_| "default-personal-workspace".to_string())
    } else {
        "default-personal-workspace".to_string()
    };

    Ok(Json(HealthStatus {
        status: "healthy".to_string(),
        workspace_id,
    }))
}

#[derive(Deserialize)]
struct CreateEntity {
    module: String,
    r#type: String,
    title: String,
    attrs: serde_json::Value,
}

#[derive(Serialize)]
struct EntityResponse {
    id: String,
    status: String,
}

async fn entity_create_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateEntity>,
) -> Result<Json<EntityResponse>, axum::http::StatusCode> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let id = format!("ent_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string());
    let workspace_id = "default-personal-workspace";
    let attrs_str = serde_json::to_string(&payload.attrs).unwrap_or_else(|_| "{}".to_string());

    tracing::info!("Inserting entity: id={}, module={}, type={}", id, payload.module, payload.r#type);

    state.conn.execute(
        "INSERT INTO entities (id, workspace_id, module, type, parent_id, title, status, tier, attrs, source, blob_ref, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, 'active', NULL, ?6, 'api', NULL, ?7, ?8)",
        libsql::params![
            id.clone(),
            workspace_id,
            payload.module,
            payload.r#type,
            payload.title,
            attrs_str,
            now,
            now
        ],
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to insert entity: {:?}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(EntityResponse {
        id,
        status: "created".to_string(),
    }))
}

#[derive(Deserialize)]
struct EntityQueryParams {
    module: Option<String>,
    id: Option<String>,
}

#[derive(Serialize)]
struct QueryEntityResponse {
    id: String,
    workspace_id: String,
    module: String,
    r#type: String,
    title: Option<String>,
    status: Option<String>,
    attrs: serde_json::Value,
    created_at: i64,
}

async fn entity_query_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<EntityQueryParams>,
) -> Result<Json<Vec<QueryEntityResponse>>, axum::http::StatusCode> {
    let mut query_str = "SELECT id, workspace_id, module, type, title, status, attrs, created_at FROM entities WHERE 1=1".to_string();
    let mut query_params = Vec::new();

    if let Some(ref m) = params.module {
        query_str.push_str(" AND module = ?");
        query_params.push(m.clone());
    }

    if let Some(ref id) = params.id {
        query_str.push_str(" AND id = ?");
        query_params.push(id.clone());
    }

    query_str.push_str(" ORDER BY created_at DESC");

    let mut stmt = state.conn.prepare(&query_str).await.map_err(|e| {
        tracing::error!("Failed to prepare query statement: {:?}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let libsql_params = libsql::params_from_iter(query_params);
    let mut rows = stmt.query(libsql_params).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut entities = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        let id: String = row.get(0).unwrap();
        let workspace_id: String = row.get(1).unwrap();
        let module: String = row.get(2).unwrap();
        let r#type: String = row.get(3).unwrap();
        let title: Option<String> = row.get(4).unwrap();
        let status: Option<String> = row.get(5).unwrap();
        let attrs_str: String = row.get(6).unwrap();
        let created_at: i64 = row.get(7).unwrap();

        let attrs: serde_json::Value = serde_json::from_str(&attrs_str).unwrap_or(serde_json::Value::Null);

        entities.push(QueryEntityResponse {
            id,
            workspace_id,
            module,
            r#type,
            title,
            status,
            attrs,
            created_at,
        });
    }

    Ok(Json(entities))
}

#[derive(Deserialize)]
struct ModuleRequest {
    prompt: String,
    workspace_id: String,
}

#[derive(Serialize)]
struct ModuleRequestResponse {
    request_id: String,
    status: String,
}

async fn module_request_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ModuleRequest>,
) -> Result<Json<ModuleRequestResponse>, axum::http::StatusCode> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let request_id = format!("req_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string());
    
    tracing::info!("Inserting module request: id={}, prompt={}", request_id, payload.prompt);

    state.conn.execute(
        "INSERT INTO module_requests (id, workspace_id, prompt, status, error, created_at, updated_at) \
         VALUES (?1, ?2, ?3, 'queued', NULL, ?4, ?5)",
        libsql::params![
            request_id.clone(),
            payload.workspace_id,
            payload.prompt,
            now,
            now
        ],
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to insert module request: {:?}", e);
        axum::http::StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(ModuleRequestResponse {
        request_id,
        status: "queued".to_string(),
    }))
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    name: String,
    workspace_name: String,
}

#[derive(Serialize)]
struct RegisterResponse {
    user_id: String,
    workspace_id: String,
    key_token: String,
    status: String,
}

async fn register_handler(Json(payload): Json<RegisterRequest>) -> Json<RegisterResponse> {
    tracing::info!("Registering user: {} with workspace: {}", payload.email, payload.workspace_name);
    
    let user_id = format!("usr_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string());
    let workspace_id = format!("ws_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string());
    let key_token = format!("key_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..16].to_string());

    Json(RegisterResponse {
        user_id,
        workspace_id,
        key_token,
        status: "registered".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_setup_and_seed() {
        let db_file = "test_temp_lifeos.db";
        // Clean up previous run if any
        let _ = std::fs::remove_file(db_file);

        let db = Builder::new_local(db_file).build().await.unwrap();
        let conn = db.connect().unwrap();

        execute_migrations(&conn).await;
        seed_database(&conn).await;

        let mut stmt = conn
            .prepare("SELECT id, name FROM workspaces WHERE id = ?1")
            .await
            .unwrap();
        let mut rows = stmt.query(libsql::params!["default-personal-workspace"]).await.unwrap();
        let row = rows.next().await.unwrap().expect("Workspace should be seeded");
        let id: String = row.get(0).unwrap();
        let name: String = row.get(1).unwrap();
        assert_eq!(id, "default-personal-workspace");
        assert_eq!(name, "Personal Workspace");

        let mut stmt = conn
            .prepare("SELECT name FROM users WHERE email = ?1")
            .await
            .unwrap();
        let mut rows = stmt.query(libsql::params!["chayan@lifeos.app"]).await.unwrap();
        let row = rows.next().await.unwrap().expect("User should be seeded");
        let user_name: String = row.get(0).unwrap();
        assert_eq!(user_name, "Chayan Aggarwal");

        let _ = std::fs::remove_file(db_file);
    }
}
