use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Parser)]
#[command(name = "lifeos")]
#[command(about = "Life OS - command line companion", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check the status of local Life OS services
    Status,
    /// Inspect or create database entities
    Entity {
        #[arg(short, long)]
        module: Option<String>,
        #[arg(short, long)]
        id: Option<String>,
        #[arg(long)]
        create: bool,
        #[arg(short, long)]
        r#type: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        attrs: Option<String>,
    },
    /// Interact with VCS
    Vcs {
        #[arg(short, long)]
        commit: Option<String>,
    },
}

#[derive(Deserialize, Serialize)]
struct HealthStatus {
    status: String,
    workspace_id: String,
}

#[derive(Serialize)]
struct CreateEntity {
    module: String,
    r#type: String,
    title: String,
    attrs: Value,
}

#[derive(Deserialize)]
struct EntityResponse {
    id: String,
    status: String,
}

#[derive(Deserialize, Debug)]
struct QueryEntityResponse {
    id: String,
    workspace_id: String,
    module: String,
    r#type: String,
    title: Option<String>,
    status: Option<String>,
    attrs: Value,
    created_at: i64,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let client = reqwest::Client::new();

    match &cli.command {
        Commands::Status => {
            println!("Life OS Services Status:");
            
            // Check health of API
            match client.get("http://127.0.0.1:8080/api/health").send().await {
                Ok(resp) => {
                    if let Ok(health) = resp.json::<HealthStatus>().await {
                        println!("  lifeos-api      : ONLINE (Workspace: {})", health.workspace_id);
                    } else {
                        println!("  lifeos-api      : ONLINE (Response format error)");
                    }
                }
                Err(_) => {
                    println!("  lifeos-api      : OFFLINE (Check if running on port 8080)");
                }
            }
            println!("  lifeos-vcs      : Idle");
            println!("  lifeos-ingest   : Idle");
            println!("  lifeos-pipelines: Idle");
            println!("  lifeos-drain    : Idle");
        }
        Commands::Entity {
            module,
            id,
            create,
            r#type,
            title,
            attrs,
        } => {
            if *create {
                let m = module.clone().expect("Module is required when creating an entity (--module)");
                let t = r#type.clone().expect("Type is required when creating an entity (--type)");
                let title_val = title.clone().unwrap_or_else(|| format!("Untitled {}", t));
                let attrs_json: Value = if let Some(ref a) = attrs {
                    serde_json::from_str(a).expect("Invalid JSON string in attrs")
                } else {
                    serde_json::json!({})
                };

                let payload = CreateEntity {
                    module: m,
                    r#type: t,
                    title: title_val,
                    attrs: attrs_json,
                };

                match client.post("http://127.0.0.1:8080/api/entity")
                    .json(&payload)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(res) = resp.json::<EntityResponse>().await {
                                println!("✅ Entity successfully created!");
                                println!("  ID    : {}", res.id);
                                println!("  Status: {}", res.status);
                            } else {
                                println!("❌ Failed to parse entity creation response.");
                            }
                        } else {
                            println!("❌ API returned error status: {}", resp.status());
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to connect to lifeos-api: {:?}", e);
                        println!("👉 Is the API server running? Run `cargo run --bin lifeos-api` to start it.");
                    }
                }
            } else {
                // Querying entities
                let mut url = "http://127.0.0.1:8080/api/entity?".to_string();
                if let Some(ref m) = module {
                    url.push_str(&format!("module={}&", m));
                }
                if let Some(ref i) = id {
                    url.push_str(&format!("id={}&", i));
                }

                match client.get(&url).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(entities) = resp.json::<Vec<QueryEntityResponse>>().await {
                                if entities.is_empty() {
                                    println!("No entities found matching filters.");
                                } else {
                                    println!("Found {} entities:", entities.len());
                                    for ent in entities {
                                        println!("--------------------------------------------------");
                                        println!("ID         : {}", ent.id);
                                        println!("Workspace  : {}", ent.workspace_id);
                                        println!("Module     : {}", ent.module);
                                        println!("Type       : {}", ent.r#type);
                                        if let Some(ref t) = ent.title {
                                            println!("Title      : {}", t);
                                        }
                                        if let Some(ref s) = ent.status {
                                            println!("Status     : {}", s);
                                        }
                                        println!("Created At : {}", ent.created_at);
                                        println!("Attributes : {}", serde_json::to_string_pretty(&ent.attrs).unwrap_or_default());
                                    }
                                    println!("--------------------------------------------------");
                                }
                            } else {
                                println!("❌ Failed to parse query response as entities.");
                            }
                        } else {
                            println!("❌ API returned error status: {}", resp.status());
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to connect to lifeos-api: {:?}", e);
                        println!("👉 Is the API server running? Run `cargo run --bin lifeos-api` to start it.");
                    }
                }
            }
        }
        Commands::Vcs { commit } => {
            println!("VCS engine status:");
            if let Some(c) = commit {
                println!("  Reviewing commit: {}", c);
            } else {
                println!("  No file versions committed yet.");
            }
        }
    }
}
