//! clap command surface. Deliberately allow-listed: only the generic
//! data-plane verbs, metrics, file versioning, and local config. There are NO
//! order/trade write verbs anywhere - trading stays read-only for every agent.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "lifeos", version, about = "Life OS - allow-listed command line companion")]
pub struct Cli {
    /// API base URL (overrides LIFEOS_API_URL and config).
    #[arg(long, global = true)]
    pub api_url: Option<String>,
    /// Bearer token (overrides LIFEOS_TOKEN and config).
    #[arg(long, global = true)]
    pub token: Option<String>,
    /// Workspace id (overrides LIFEOS_WORKSPACE and config).
    #[arg(long, global = true)]
    pub workspace: Option<String>,
    /// Emit raw JSON instead of a human summary.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Check whether lifeos-api is reachable.
    Status,
    /// Generic entities (task/trade/topic/post/...): create, get, list, update.
    Entity {
        #[command(subcommand)]
        cmd: EntityCmd,
    },
    /// Graph relations between entities.
    Edge {
        #[command(subcommand)]
        cmd: EdgeCmd,
    },
    /// Append-only domain + harness log (no update/delete by design).
    Event {
        #[command(subcommand)]
        cmd: EventCmd,
    },
    /// Background job queue.
    Job {
        #[command(subcommand)]
        cmd: JobCmd,
    },
    /// Workspace metrics rollup.
    Metrics,
    /// Gmail: thin proxy read + gated draft (issue #53).
    Gmail {
        #[command(subcommand)]
        cmd: GmailCmd,
    },
    /// Google Calendar: thin proxy read + gated draft (issue #53).
    Calendar {
        #[command(subcommand)]
        cmd: CalendarCmd,
    },
    /// Google Drive: thin proxy read + gated draft (issue #53).
    Drive {
        #[command(subcommand)]
        cmd: DriveCmd,
    },
    /// Notion: thin proxy read + gated draft (issue #53).
    Notion {
        #[command(subcommand)]
        cmd: NotionCmd,
    },
    /// Slack: thin proxy read + gated draft (issue #53).
    Slack {
        #[command(subcommand)]
        cmd: SlackCmd,
    },
    /// File versioning via lifeos-vcs (history/commit).
    File {
        #[command(subcommand)]
        cmd: FileCmd,
    },
    /// Local CLI config (api_url, token, workspace).
    Config {
        #[command(subcommand)]
        cmd: ConfigCmd,
    },
}

#[derive(Subcommand)]
pub enum EntityCmd {
    Create {
        #[arg(long)]
        module: String,
        #[arg(long = "type")]
        r#type: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        parent_id: Option<String>,
        /// Attrs as a JSON object string, e.g. --attrs '{"due":1700000000}'.
        #[arg(long)]
        attrs: Option<String>,
    },
    Get {
        id: String,
    },
    List {
        #[arg(long)]
        module: Option<String>,
        #[arg(long = "type")]
        r#type: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        parent_id: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
    },
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        attrs: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum EdgeCmd {
    Create {
        #[arg(long)]
        src_id: String,
        #[arg(long)]
        rel: String,
        #[arg(long)]
        dst_id: Option<String>,
        #[arg(long)]
        dst_ref: Option<String>,
        #[arg(long)]
        state: Option<String>,
    },
    List {
        #[arg(long)]
        src_id: Option<String>,
        #[arg(long)]
        dst_id: Option<String>,
        #[arg(long)]
        rel: Option<String>,
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Update {
        id: String,
        #[arg(long)]
        state: String,
    },
}

#[derive(Subcommand)]
pub enum EventCmd {
    Create {
        #[arg(long = "type")]
        r#type: String,
        #[arg(long)]
        entity_id: Option<String>,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        attrs: Option<String>,
    },
    List {
        #[arg(long = "type")]
        r#type: Option<String>,
        #[arg(long)]
        entity_id: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
}

#[derive(Subcommand)]
pub enum JobCmd {
    Create {
        #[arg(long)]
        kind: String,
        /// Payload as a JSON string.
        #[arg(long)]
        payload: Option<String>,
        #[arg(long)]
        priority: Option<i64>,
    },
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
}

#[derive(Subcommand)]
pub enum FileCmd {
    History {
        #[arg(long)]
        entity_id: String,
    },
    Commit {
        /// Local file to read content from - the CLI encodes and uploads
        /// the bytes, it never asks the API to read a client-side path.
        #[arg(long)]
        path: String,
        #[arg(long)]
        message: Option<String>,
        /// Existing file entity id to commit a new version onto; omit to create a new file.
        #[arg(long)]
        entity_id: Option<String>,
    },
    /// Retrieves a version's content by hash - the entity's current version
    /// by default, or a specific historical `--blob-ref` from `history`.
    Checkout {
        #[arg(long)]
        entity_id: String,
        #[arg(long)]
        blob_ref: Option<String>,
        #[arg(long)]
        out: String,
    },
}

#[derive(Subcommand)]
pub enum GmailCmd {
    /// Free read: proxies to Gmail's `messages.list`.
    List {
        #[arg(long)]
        q: Option<String>,
    },
    /// Gated (docs/SECURITY.md §2): only creates a draft entity.
    Send {
        #[arg(long)]
        to: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        body: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum CalendarCmd {
    /// Free read: proxies to `events.list` on the primary calendar.
    List,
    /// Gated (docs/SECURITY.md §2): only creates a draft entity.
    Create {
        #[arg(long)]
        summary: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: String,
    },
}

#[derive(Subcommand)]
pub enum DriveCmd {
    /// Free read: proxies to `files.list`.
    List,
    /// Gated (docs/SECURITY.md §2): only creates a draft entity.
    Upload {
        #[arg(long)]
        name: String,
        #[arg(long)]
        source_ref: String,
    },
}

#[derive(Subcommand)]
pub enum NotionCmd {
    /// Free read: proxies to Notion's `/v1/search`.
    List,
    /// Gated (docs/SECURITY.md §2): only creates a draft entity.
    Create {
        #[arg(long)]
        parent_id: String,
        #[arg(long)]
        title: String,
    },
}

#[derive(Subcommand)]
pub enum SlackCmd {
    /// Free read: proxies to `conversations.list`.
    List,
    /// Gated (docs/SECURITY.md §2): only creates a draft entity.
    Post {
        #[arg(long)]
        channel: String,
        #[arg(long)]
        text: String,
    },
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Print the resolved config file path.
    Path,
    /// Show stored settings (token masked).
    List,
    Get {
        key: String,
    },
    Set {
        key: String,
        value: String,
    },
}
