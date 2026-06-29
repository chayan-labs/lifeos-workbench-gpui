use clap::{Parser, Subcommand};

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
    },
    /// Interact with VCS
    Vcs {
        #[arg(short, long)]
        commit: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Status => {
            println!("Life OS Services Status:");
            println!("  lifeos-api      : Running (127.0.0.1:8080)");
            println!("  lifeos-vcs      : Idle");
            println!("  lifeos-ingest   : Idle");
            println!("  lifeos-pipelines: Idle");
            println!("  lifeos-drain    : Idle");
        }
        Commands::Entity { module, id } => {
            println!("Querying entities...");
            if let Some(m) = module {
                println!("  Filter by module: {}", m);
            }
            if let Some(i) = id {
                println!("  Filter by ID: {}", i);
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
