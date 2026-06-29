use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

#[derive(Serialize, Deserialize)]
struct HealthStatus {
    status: String,
    workspace_id: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Set up CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build API router
    let app = Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/entity", post(entity_create_handler))
        .route("/api/module-request", post(module_request_handler))
        .route("/api/register", post(register_handler))
        .layer(cors);

    // Bind to localhost
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::info!("Life OS local API listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_handler() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy".to_string(),
        workspace_id: "default-personal-workspace".to_string(),
    })
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

async fn entity_create_handler(Json(payload): Json<CreateEntity>) -> Json<EntityResponse> {
    tracing::info!("Creating entity: module={}, type={}", payload.module, payload.r#type);
    Json(EntityResponse {
        id: uuid::Uuid::new_v4().to_string(),
        status: "created".to_string(),
    })
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

async fn module_request_handler(Json(payload): Json<ModuleRequest>) -> Json<ModuleRequestResponse> {
    tracing::info!("Received module scaffold request: '{}' for workspace={}", payload.prompt, payload.workspace_id);
    Json(ModuleRequestResponse {
        request_id: uuid::Uuid::new_v4().to_string(),
        status: "queued".to_string(),
    })
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
