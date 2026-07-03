# Life OS Workbench — Claude working notes

Terminal-weight, agent-native **fork of Life OS**, shipping as a **standalone desktop app**. Same Rust backend, no browser: every Life OS module renders as a cell-grid UI over the _same_ declarative manifests, `lifeos-api` is linked **in-process**, and the app is simultaneously a terminal, a modal editor, an AI agent, and a Mirrorscope DAP client. The primary face is its **own GPU-rendered window** (`winit` + `wgpu` glyph grid implementing ratatui's `Backend`); running inside a host terminal survives only as the secondary `--tui` mode (SSH/headless). Release bar: Cursor-class features, Zed-adjacent visuals, terminal-class weight (no Electron/webview), and first-launch config import from VS Code + WezTerm. Read `README.md` for the narrative spec and `docs/ARCHITECTURE.md` (+ the upstream `docs/` tree) for the authoritative **backend** spec before non-trivial work. Read `frontend/DESIGN.md` for the cell-grid design system.

## Sibling repos (don't merge them)

- **Upstream backend:** [`life-os`](https://github.com/chayan-bit/life-os) — this is a fork of it; backend (`services/`, `modules/`, `migrations/`, `worker/`) is inherited and reused, only `frontend/` + the client↔API boundary change.
- **Debugger:** [`Mirrorscope`](https://github.com/chayan-bit/Mirrorscope) (local `../Mirrorscope`) — the time-travel debugger; the Workbench is its primary **DAP client** and host UI. Contract is DAP; keep it standalone.

## Mental model (don't violate)

- **This is a presentation + process-boundary fork, not a backend rewrite.** Every upstream invariant holds verbatim: one generic multi-tenant schema (no per-domain tables), `workspace_id` on every row, local-first libSQL replica (`offline:true`, last-push-wins + single-writer-per-row + `events` as reconciliation truth), codegen-only-on-Mac, owned-credentials-only (Nango, never claude.ai connectors), trading read-only for any agent (`broker-guard` fails closed), outward actions human-gated, `events` append-only (no UPDATE/DELETE), derived state in a separate un-synced `lifeos-derived.db`.
- **Frontend = a second renderer backend, not new modules.** Modules stay declarative manifests (`entityTypes`/`attrs`/`views:[{kind}]`, no DOM/DB code). The Workbench interprets the _identical_ manifests with **ratatui** widgets instead of React. Never fork a manifest to make it TUI-specific; if a `view.kind` can't render in a grid, degrade per `frontend/DESIGN.md`, don't change the manifest.
- **In-process, not bridged.** The Workbench binary is Rust and links `lifeos-api` as a crate — no HTTP `fetch`, no `localhost:8080` round-trip for the app itself. The `127.0.0.1` HTTP server is retained only for external consumers. "Life OS inside the app" means one address space, literally.
- **Three surfaces, one binary, one pane manager.** Terminal / editor / agent / Life OS are panes of a Zellij-model tiling layout wrapped in Zed-model IDE chrome: tab bar on top, persistent file sidebar left, editor-first center panes, an integrated terminal dock along the bottom, statusline below — all fully mouse-driven (click focuses panes/places the editor cursor, wheel scrolls anything under it, modal rows click-activate). Switching a pane's mode is instant and shares cwd + env + the in-process `lifeos-api` handle; the dock's shell session survives being hidden.
- **Reuse the surface, don't build it.** Editor = embed Helix core (`helix-core`/`helix-view`): rope, tree-sitter, LSP — do NOT write an editor. Agent = an **ACP** client → the existing harness — do NOT write a model. Terminal = `alacritty_terminal` VTE + `portable-pty`. Debugger = a **DAP** client → Mirrorscope — do NOT reimplement replay here. Window host = thin `winit`+`wgpu` glyph-grid renderer behind ratatui's `Backend` trait — do NOT adopt a retained-mode GUI toolkit or fork the UI into a second widget tree.
- **One UI tree, two backends.** All panes/views/keymaps are written once against ratatui; the GPU window and `--tui` are interchangeable `Backend` impls. Never write window-only UI logic above the backend seam (window-only _renderer_ capabilities — inline images, sub-cell decorations, ghost text — live below it and degrade gracefully in `--tui`).

## What changes vs upstream (the entire fork surface)

| Concern    | Upstream                                   | Here                                                                            |
| ---------- | ------------------------------------------ | ------------------------------------------------------------------------------- |
| UI         | React+Vite SPA (Refine/Cytoscape/Tailwind) | ratatui renderers over the same manifests → GPU window (primary) / ANSI `--tui` |
| Window     | browser tab                                | own `winit`+`wgpu` glyph-grid window, `.app` bundle; ligatures/themes/padding   |
| Editor     | none                                       | Helix core embed                                                                |
| Terminal   | browser                                    | `alacritty_terminal` + `portable-pty` panes                                     |
| Client↔API | HTTP fetch                                 | in-process crate link                                                           |
| Agent      | React AI Console                           | ACP client pane, edits as reviewable diffs                                      |
| Config     | none                                       | first-launch import: VS Code settings/keybindings/themes + WezTerm Lua (`mlua`) |
| Debug      | none                                       | DAP client → Mirrorscope (task tree / scrubber / watchpoints)                   |
| Mobile     | PWA                                        | dropped — Telegram lane covers it                                               |
| Backend    | Rust services + libSQL + Nango + Worker    | UNCHANGED                                                                       |

## Cell-grid capability boundary (be honest, don't fake it)

- **Excellent (≈80% of use):** code editing + LSP, terminal, lists/tables/trees/Kanban/forms, command palette, styled markdown, sparkline/bar charts, timeline scrubbers, async task trees. The editor + debug surface and structured Life OS live here.
- **Native-window extras (we own the renderer):** real inline images as cell-aligned textured quads (galleries, thumbnails, flamegraphs), sub-cell decorations (diagnostic squiggles, ghost text for edit prediction, blended scrollbar marks), full font/ligature/theme/padding control. In `--tui` these degrade to terminal graphics protocols (Kitty/Sixel/iTerm2) or metadata lists.
- **Visual tail → escape hatches, in order:** (1) on-demand external viewer for the true-graphical tail (big graphs, Figma canvas, video); (2) bounded wgpu canvas-overlay panes ONLY if design/graph work proves daily-critical. Never adopt a GUI toolkit or make the whole app heavy to serve the tail.
- **Out of scope for the grid:** Cytoscape-class graph editing, Figma canvas editing, inline video/audio playback, proportional-font/animation, real maps → route to the hatches.

## Mirrorscope integration (DAP)

- The Workbench is a DAP **client**; Mirrorscope is the DAP **server** (`reverseContinue`/`stepBack` + custom `listCheckpoints`/`taskTimeline`/`jumpToEvent`). Run it as a child process/thread behind the DAP interface; keep it usable from other clients.
- Mirrorscope's async **task tree / logical stack / timeline / watchpoint / waker-causality** outputs render as native TUI panes here — this is the "companion UI" its spec wanted, for free.
- **Flagship: agentic time-travel debugging.** Expose Mirrorscope's DAP ops to the ACP agent as tools so it can replay-to-fault → set retroactive watchpoint → scrub to the causing write → read logical async task state → propose a fix. Debug artifacts (sessions/checkpoints/root-causes) are written back as `events`/`entities` in the Coding module.

## The spine (why the 3 repos are one system)

Time-travel/replay is the universal primitive: Mirrorscope = replay over **execution**, `lifeos-vcs` = history over **files**, Life OS memory = event-sourced **knowledge**. One append-only `events` model, one semantic index (memvec/mgrep over code + execution + second brain), one agent across all three.

## Rust where it counts (now includes the frontend)

Everything security-/throughput-critical stays Rust (`lifeos-api`, `lifeos-vcs`, `lifeos-ingest`, `lifeos-pipelines`, `lifeos-drain`, `broker-guard`, `bin/lifeos`) **plus** the whole `workbench/` app (shell, terminal, editor embed, renderer backend, DAP/ACP clients). Stays JS: module manifests, Worker bot, `scaffold.js`. Stays Python: memvec.

## Build order (de-risk the light path first)

1. Shell: tiling pane manager + embedded terminal + command palette. ✅ 2. Editor pane: Helix core embed. ✅ 3. Life OS TUI renderer: link `lifeos-api`, `list`/`board`/`table`/`detail` over existing manifests. ✅ 4. Agent pane (ACP) with reviewable diffs + toolbelt. ✅ 5. Standalone window host: `winit`+`wgpu` glyph grid as a ratatui `Backend`, `--tui` retained. 6. Zed-adjacent polish: fonts/ligatures/themes/padding, sub-cell decorations, `.app` bundle. 7. Cursor-parity feature wave (README §12; tracked as `parity`/`agent-native` issues). 8. Config importers: VS Code + WezTerm Lua, run on first launch. 9. Mirrorscope DAP client (only after Mirrorscope Phases 1-2). 10. Agentic time-travel debugging. 11. Visual-tail polish (inline images, external viewer). Phases 1-8 = shippable standalone lightweight agentic IDE with Life OS built in.

## Conventions (inherited)

- Conventional commits, no co-author trailers. Many small files (200-400 lines, 800 max), functions <50 lines, immutable patterns. Tests ≥80%, TDD. One commit per resolved issue.
- **Run `cargo clean` in `services/` (and `workbench/`) after a Rust build session** — `target/` is gitignored but regrows into tens of GB across the workspace.
- Backend behavior/spec questions → upstream `docs/` is authoritative; don't duplicate it here, link it.
