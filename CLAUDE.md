# Life OS Workbench (GPU-native) — Claude working notes

Agent-native **fork of Life OS**, shipping as a **standalone desktop app** with a real GPU-native GUI.
Same Rust backend, no browser, no Electron/webview: every Life OS module renders as native GUI widgets over the _same_ declarative manifests, `lifeos-api` is linked **in-process**, and the app is simultaneously a terminal, a code editor, an AI agent, and a Mirrorscope DAP client.
The face is its **own GPU-rendered window** built on **`gpui`** (Zed's GPU/Metal UI engine) + **`gpui-component`** (native component library) - a genuine retained GUI, not a character cell grid.
Release bar: Cursor-class features, Zed-grade visuals, native-lightweight weight (no Electron/webview), Lua-configurable, everything mouse-reachable, and first-launch config import from VS Code + WezTerm.
Read `README.md` for the narrative spec and `docs/ARCHITECTURE.md` (+ the upstream `docs/` tree) for the authoritative **backend** spec before non-trivial work.

## Why this fork exists (the pivot)

The upstream Workbench rendered its entire UI as a monospace character cell grid (ratatui into a wgpu glyph grid).
That is _why_ it looked like a terminal no matter how it was styled: every pixel was a character cell.
This fork **lifts the terminal-weight / glyph-grid constraint** and replaces the renderer with a real GUI: `gpui` + `gpui-component`.
Still native and lightweight (Metal, no browser), but with true GUI visuals - real font rendering, sub-pixel layout, a visible terminal cursor, resizable panels, native menus, and mouse-first interaction by construction.
**This is a presentation-layer replacement, not a backend or logic rewrite.**
The origin repo (`../lifeos-workbench`) keeps its ratatui `--tui` build; this fork is GUI-only (gpui cannot render to a remote terminal).

## Sibling repos (don't merge them)

- **Origin (ratatui build):** `../lifeos-workbench` - the cell-grid Workbench this was seeded from; keeps the headless `--tui` path.
- **Upstream backend:** [`life-os`](https://github.com/chayan-bit/life-os) - backend (`services/`, `modules/`, `migrations/`, `worker/`) is inherited and reused verbatim; only `frontend/` + the client<->API boundary change.
- **Debugger:** [`Mirrorscope`](https://github.com/chayan-bit/Mirrorscope) (local `../Mirrorscope`) - the time-travel debugger; the Workbench is its primary **DAP client** and host UI. Contract is DAP; keep it standalone.

## Mental model (don't violate)

- **Presentation + process-boundary fork, not a backend rewrite.** Every upstream invariant holds verbatim: one generic multi-tenant schema (no per-domain tables), `workspace_id` on every row, local-first libSQL replica (`offline:true`, last-push-wins + single-writer-per-row + `events` as reconciliation truth), codegen-only-on-Mac, owned-credentials-only (Nango, never claude.ai connectors), trading read-only for any agent (`broker-guard` fails closed), outward actions human-gated, `events` append-only (no UPDATE/DELETE), derived state in a separate un-synced `lifeos-derived.db`.
- **Frontend = a second renderer, not new modules.** Modules stay declarative manifests (`entityTypes`/`attrs`/`views:[{kind}]`, no DOM/DB code). The Workbench interprets the _identical_ manifests with **gpui-component** widgets (Table/List/panels/forms) instead of React. Never fork a manifest to make it GUI-specific; the view-model builders (`views.rs`) stay renderer-agnostic - they emit data, the gpui views paint it.
- **In-process, not bridged.** The Workbench binary is Rust and links `lifeos-api` as a crate - no HTTP `fetch`, no `localhost:8080` round-trip for the app itself. The `127.0.0.1` HTTP server is retained only for external consumers. "Life OS inside the app" means one address space, literally.
- **Three surfaces, one binary, one workspace.** Terminal / editor / agent / Life OS are panels of a `gpui-component` `DockArea` (resizable splits, dock tabs) wrapped in IDE chrome: `TitleBar` with menu + tab strip on top, a resizable file sidebar left, editor-first center, an integrated terminal dock along the bottom, statusline below - all mouse-driven natively (gpui elements own click/drag/scroll/cursor). Switching a panel's mode shares cwd + env + the in-process `lifeos-api` handle; the dock's shell session survives being hidden.
- **Reuse the surface, don't build it.** Editor = **both** Helix core (`helix-core`: rope, tree-sitter, our `lsp.rs`) rendered by a custom gpui element **and** gpui-component's built-in code editor, selectable via Lua `editor.engine = "helix" | "native"` - do NOT write an editor from scratch. Agent = an **ACP** client -> the existing harness - do NOT write a model. Terminal = `alacritty_terminal` VTE + `portable-pty`, painted by a custom gpui `Element` that draws a real block/beam cursor at `grid.cursor`. Debugger = a **DAP** client -> Mirrorscope - do NOT reimplement replay here. Window/menu/clipboard = `gpui` - do NOT hand-roll a window host.
- **Shared logic core + gpui renderer.** All pure logic (command registry + keymap + fuzzy in `palette.rs`, resource reconciliation in `pane_store.rs`, `layout.rs`, `manifest.rs`, `views.rs` builders, `file_tree.rs`, `lsp.rs`, `acp.rs`, `diff.rs`, `api.rs`, and the state-machine halves of the pane modules) is renderer-agnostic and unit-tested. The gpui views in `workbench/src/ui/` consume it. Keep the seam: never entangle gpui types into the logic core.

## What changes vs upstream (the entire fork surface)

| Concern     | Upstream                                   | Here                                                                     |
| ----------- | ------------------------------------------ | ------------------------------------------------------------------------ |
| UI          | React+Vite SPA (Refine/Cytoscape/Tailwind) | `gpui` + `gpui-component` native widgets over the same manifests         |
| Window      | browser tab                                | own `gpui` GPU/Metal window, `.app` bundle; fonts/ligatures/themes       |
| Editor      | none                                       | Helix core embed **and** gpui-component editor, Lua-selectable           |
| Terminal    | browser                                    | `alacritty_terminal` + `portable-pty`, custom gpui element with a cursor |
| Client<->API | HTTP fetch                                 | in-process crate link                                                     |
| Agent       | React AI Console                           | ACP client panel, edits as reviewable diffs                              |
| Config      | none                                       | Lua config (`mlua`) + first-launch import: VS Code + WezTerm             |
| Debug       | none                                       | DAP client -> Mirrorscope (task tree / scrubber / watchpoints)           |
| Mobile      | PWA                                        | dropped - Telegram lane covers it                                        |
| Backend     | Rust services + libSQL + Nango + Worker    | UNCHANGED                                                                 |

## GUI capability boundary (be honest, don't fake it)

Native GUI removes the cell-grid ceiling: proportional fonts, real inline images, sub-pixel layout, smooth scrolling, blended decorations, and animation are all first-class now.

- **Excellent (core of the app):** code editing + LSP, terminal with a real cursor, lists/tables/trees/Kanban/forms, command palette, styled markdown, charts, timeline scrubbers, async task trees, inline images/thumbnails/flamegraphs, diagnostic squiggles, ghost text, resizable/dockable panels.
- **Still route out (heavy specialist surfaces):** Cytoscape-class graph editing, Figma canvas editing, real maps -> on-demand external viewer or a bounded `gpui` canvas panel only if it proves daily-critical. Don't bloat the core to serve the tail.

## Mirrorscope integration (DAP)

- The Workbench is a DAP **client**; Mirrorscope is the DAP **server** (`reverseContinue`/`stepBack` + custom `listCheckpoints`/`taskTimeline`/`jumpToEvent`). Run it as a child process/thread behind the DAP interface; keep it usable from other clients.
- Mirrorscope's async **task tree / logical stack / timeline / watchpoint / waker-causality** outputs render as native gpui panels here - the "companion UI" its spec wanted, for free.
- **Flagship: agentic time-travel debugging.** Expose Mirrorscope's DAP ops to the ACP agent as tools so it can replay-to-fault -> set retroactive watchpoint -> scrub to the causing write -> read logical async task state -> propose a fix. Debug artifacts (sessions/checkpoints/root-causes) are written back as `events`/`entities` in the Coding module.

## The spine (why the 3 repos are one system)

Time-travel/replay is the universal primitive: Mirrorscope = replay over **execution**, `lifeos-vcs` = history over **files**, Life OS memory = event-sourced **knowledge**. One append-only `events` model, one semantic index (memvec/mgrep over code + execution + second brain), one agent across all three.

## Rust where it counts (includes the frontend)

Everything security-/throughput-critical stays Rust (`lifeos-api`, `lifeos-vcs`, `lifeos-ingest`, `lifeos-pipelines`, `lifeos-drain`, `broker-guard`, `bin/lifeos`) **plus** the whole `workbench/` app (shell, terminal, editor embed, gpui renderer, DAP/ACP clients). Stays JS: module manifests, Worker bot, `scaffold.js`. Stays Python: memvec.

## Async bridging (tokio <-> gpui)

`lifeos-api`, PTY readers, ACP/LSP children need tokio; gpui has its own executor.
Keep a tokio runtime owned by `ui/app.rs`; async results (search hits, entity fetches, agent stream, LSP diagnostics) are delivered into gpui via channels drained in a repaint tick / `cx.spawn` + `Entity::update`.
All `lifeos-api` and I/O calls stay off the render thread.

## Build order (this fork)

1. Fork repo + seed + gpui "hello workspace" window compiling with the kept crates. 2. Chrome: `TitleBar` + native menu + resizable sidebar/editor/dock + statusline + tab strip. 3. **Terminal element with a real cursor** (integrated terminal, fixes the "broken" feel). 4. File sidebar + command palette + tabs/splits/close over the kept `layout`/`palette` logic. 5. Editor: Helix-core custom element **and** native engine, config-switchable. 6. Life OS views (Table/List/board/detail) + agent + recall panels. 7. Lua config (`mlua`) + VS Code/WezTerm import; theming/fonts/ligatures polish. 8. `.app` bundle + full E2E verification. Then: Mirrorscope DAP client, agentic time-travel debugging, visual-tail polish.

## Conventions (inherited)

- Conventional commits, no co-author trailers. Many small files (200-400 lines, 800 max), functions <50 lines, immutable patterns. Tests >=80%, TDD. One commit per resolved issue.
- **Run `cargo clean` in `services/` (and `workbench/`) after a Rust build session** - `target/` is gitignored but regrows into tens of GB across the workspace.
- Backend behavior/spec questions -> upstream `docs/` is authoritative; don't duplicate it here, link it.
