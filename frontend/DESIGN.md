---
name: Life OS Workbench — Terminal Brutalism (cell grid)
target: native GPU window (primary — winit+wgpu glyph grid, truecolor always, ligatures, user fonts) · terminal via --tui (fallback — truecolor with 256-color + 16-color degradation)
supersedes: Life OS Neo-Brutal (web/React design system)
palette:
  # Truecolor hex kept from the Neo-Brutal identity so cloud/local/TUI read as one brand.
  # Each maps to a 256-color and a 16-color ANSI fallback for degraded terminals.
  bg:            { hex: '#12120a', ansi256: 233, ansi16: 'black' }          # near-black cream-tinted base
  bg-alt:        { hex: '#1c1c0f', ansi256: 234, ansi16: 'black' }          # alternate rows / inactive panes
  surface:       { hex: '#26261a', ansi256: 235, ansi16: 'brightblack' }    # pane fill
  fg:            { hex: '#f5f1dc', ansi256: 230, ansi16: 'white' }          # primary text
  fg-dim:        { hex: '#a8a68c', ansi256: 144, ansi16: 'brightblack' }    # secondary text, hints
  primary:       { hex: '#4b4cff', ansi256:  63, ansi16: 'brightblue' }     # active/selection/focus accent
  accent:        { hex: '#ffff00', ansi256: 226, ansi16: 'brightyellow' }   # focus card, current item, warnings
  success:       { hex: '#00ff9d', ansi256:  48, ansi16: 'brightgreen' }    # synced / validated / passing
  error:         { hex: '#ff4b4b', ansi256: 203, ansi16: 'brightred' }      # gated / blocked / errors
  gate:          { hex: '#ff4b4b', ansi256: 203, ansi16: 'brightred' }      # human-gate required (outward/irreversible)
  outline:       { hex: '#79785f', ansi256: 101, ansi16: 'brightblack' }    # inactive borders
  outline-focus: { hex: '#ffff00', ansi256: 226, ansi16: 'brightyellow' }   # focused pane border
emphasis:
  # Terminal has no fonts/shadows — weight is expressed with SGR attributes.
  title:      bold                              # replaces Montserrat 900 headlines
  active:     reverse                           # selected row/item (reverse video)
  focus:      'bold + outline-focus border'     # the focused pane
  muted:      dim                               # fg-dim hints, disabled
  link:       underline
  gated:      'error fg + bold'                 # anything requiring approval
borders:
  # Box-drawing replaces the Neo-Brutal hard-shadow depth model.
  pane-inactive: single   # │ ─ ┌ ┐ └ ┘
  pane-active:   thick     # ┃ ━ ┏ ┓ ┗ ┛   (in outline-focus color)
  emphasis-box:  double    # ║ ═ ╔ ╗ ╚ ╝   (gates, modals, destructive confirms)
  separator:     single    # inter-widget rules
spacing:
  cell: 1ch                # the atomic unit is one character cell
  pane-padding: 1          # 1 cell inside every pane border
  gutter: 1                # 1 blank column between side-by-side panes
  list-row: 1              # 1 line per row (dense); 2 lines for detail-rich rows
statusline: 1              # single reserved bottom line (mode · cwd · workspace · agent/debug state)
---

## Brand & style — "Terminal Brutalism"

The web app's **Neo-Brutalism** (bold blocks, hard shadows, stark outlines, high-contrast saturated color) translates cleanly to a terminal, because both are grid-native and unapologetically structural. The Workbench keeps the *identity* - the same truecolor palette, the same high-contrast, blocky, honest feel - and re-expresses its *mechanics* in terminal primitives:

| Neo-Brutal (web) | Terminal Brutalism (TUI) |
|---|---|
| Montserrat 900 headlines | `bold` + accent color (one monospace face, weight via SGR) |
| Hard drop shadows (`4px 4px 0`) | **box-drawing borders** (single → thick → double for depth) |
| Rounded corners | square box-drawing corners only (no radii in a grid) |
| Hover translate + shadow growth | pane border single → **thick** + `outline-focus` color on focus |
| Pressed translate | `reverse` video on the active row/item |
| Rotating globe `BrandMark` | a static 2-3 cell ASCII/Unicode mark in the statusline; no animation |
| Cream background `#fdfae4` | **dark** base (`#12120a`) — terminals are dark-first; the cream inverts to near-black, palette hues preserved |

The one deliberate inversion: the web system is light-on-cream; the terminal is **dark-first** (default terminal expectation, less eye strain in a daily driver). The accent hues (blue/yellow/mint/red) are preserved exactly, so a screenshot of either surface is recognizably the same product.

## Color usage

- **fg / fg-dim** — primary vs secondary text. Never rely on color alone for meaning (accessibility + 16-color terminals): pair every color signal with an attribute or glyph.
- **primary (blue `#4b4cff`)** — focus/selection accent, active pane title, system controls.
- **accent (yellow `#ffff00`)** — the current item, focused card, non-blocking warnings.
- **success (mint `#00ff9d`)** — synced status, validated modules, passing tests/CI.
- **error / gate (red `#ff4b4b`)** — errors, block states, and - critically - **anything human-gated** (outward/irreversible actions: social/marketing publish, sends, browser actions, any trade action). Gated affordances are always `error fg + bold` and boxed in `double` border so a gate is never missed.

## Depth & focus (replaces elevation/shadows)

Terminals have no z-axis; depth is conveyed by **border weight + color + reverse video**, in three levels mirroring the web system's Level 1/2/3:

- **Level 1 (resting pane/widget):** `single` border in `outline`.
- **Level 2 (focused pane):** `thick` border in `outline-focus` (yellow). Exactly one pane is Level 2 at a time.
- **Level 3 (active/pressed item):** the selected row/cell in `reverse` video (+ `accent` fg). This is the "pressed" analogue.

Modals, destructive confirms, and gates escalate to a `double` border in `error`/`accent` to read as "stop and look".

## Layout & components (TUI widget vocabulary)

Rendering is **ratatui** widgets driven by the same Life OS module manifests (`view.kind`). Component ↔ manifest mapping:

1. **Workspace chrome** — Zed-model IDE frame around the pane tree: a one-line clickable **tab bar** on top, a persistent **file sidebar** (left, toggleable, keyboard `alt+f`), the editor-first **center pane tree**, an integrated **terminal dock** along the bottom (`alt+j`; its shell session survives hiding), and the statusline. Fully mouse-driven: click focuses any region or pane and places the editor cursor, the wheel scrolls whatever is under it (editor, terminal scrollback, lists, modals), modal rows activate on click. Center panes with nothing open render the **welcome surface** (keybinding hints). Panels are flat, Zed-style: tab bar/sidebar/statusline on `bg-alt`, each pane a one-row **header** (kind dot + title + clickable `×`) over borderless content; the focused header raises to `surface`. The native menu bar (File / View / Life OS) invokes the same command ids as the palette.
2. **Pane / tiling shell** — the Zellij-model container inside the chrome; every surface (terminal, editor, agent, any Life OS view) is a header-topped flat pane; side-by-side panes are separated by a dim seam column. Focused header = Level 2. A pane whose shell exits (`exit`) closes itself; the dock's session respawns on reopen.
3. **List / Table** (`view.kind: list|table`) — dense one-line rows; `reverse` on the cursor row; column headers in `bold`+`fg-dim`; sort/filter via the command palette.
4. **Board** (`view.kind: board`) — Kanban as side-by-side bordered column-panes; cards are one/two-line list items; lifecycle status = column.
5. **Calendar** (`view.kind: calendar`) — a cell-grid; day cells show count badges; today in `accent`.
6. **Detail** (`view.kind: detail`) — a stacked field pane (`label: value`, labels `fg-dim`) + a styled-markdown body region.
7. **Timeline / scrubber** (`view.kind: timeline`) — a horizontal bar with a movable cursor; also the Mirrorscope replay scrubber (arrows scrub, Enter = `jumpToEvent`).
8. **Tree** — async task tree (Mirrorscope logical stack), file tree, entity hierarchy; indent + box-drawing branches.
9. **Charts** — `Sparkline`/`BarChart`/`Chart` for equity curves, token/cost meters, poll-timing; functional, not decorative.
10. **Command palette** — a centered `double`-bordered modal, fuzzy-filtered list; the universal command bar (replaces the web CommandBar).
11. **Statusline** — the single reserved bottom line: `mode · cwd · workspace · agent state · debug/replay state`.
12. **Gallery** (`view.kind: gallery`) — thumbnail grid rendered via **terminal graphics protocol** (Kitty/Sixel/iTerm2); falls back to a filename+metadata list where the protocol is unavailable.
13. **Graph** (`view.kind: graph`) — small graphs as box-drawing node-link/indented trees; large graphs route to the **external-viewer escape hatch** (not forced into the grid).
14. **Diff / apply layer** — agent edits (ACP) and VCS diffs shown inline: added `success`, removed `error`, hunks accept/reject with the cursor.

## Degradation & escape hatches (honest limits)

Terminal-weight is the product constraint, so visual richness degrades on purpose, in a fixed order:

- **Images** (galleries, Figma thumbnails, generated media, rendered flamegraphs) → **window mode:** cell-aligned textured quads drawn by our renderer (first-class, no protocol); **`--tui`:** terminal graphics protocol (Kitty/Sixel/iTerm2), else a metadata list.
- **True-graphical tail** (large zoomable graphs, Figma canvas editing, video/audio playback, real maps) → **on-demand external viewer**, opened only when asked, so the app stays terminal-weight.
- **If design/graph work becomes daily-critical** → bounded **wgpu canvas-overlay panes** inside the same window; explicitly an exception, never the default, never a GUI toolkit.

Never fork a manifest or bloat the whole app to serve the tail; degrade the *view*, keep the data model and the weight intact.

## Window mode (primary surface)

The standalone app owns its renderer, so the same cell-grid system gains a polish tier no host terminal offers. Everything below lives **under** the ratatui `Backend` seam - widgets never branch on the mode.

- **Typography:** user-chosen monospace face (default: JetBrains Mono), ligatures on, emoji + CJK fallback via cosmic-text, crisp HiDPI glyph atlas, subtle cell padding (x: 2px, y: 2px) and window padding (8px) so the grid breathes like Zed rather than abutting the window edge.
- **Sub-cell decorations** (renderer-level modifiers panes can request): wavy diagnostic underlines, faded *ghost text* for edit prediction, soft-blended selection tint, scrollbar strip with search/diagnostic/git marks, cursor styles (block/bar/underline) with optional smooth trailing animation (≤120ms; off by default - Brutalism prefers snap).
- **Window chrome:** native macOS traffic lights on a titlebar-merged top row (title = `workspace · cwd · mode`), dock icon + menu bar (muda), native clipboard/IME/drag-and-drop.
- **Theme import:** VS Code theme JSON and WezTerm color schemes map onto the palette roles above (bg/surface/fg/primary/accent/success/error/outline); Terminal Brutalism stays the default identity.
- **Weight budget:** cold start < 150ms to first frame, < 150MB resident with editor + terminal + agent panes open, 60fps scroll/resize. If a feature can't fit the budget it belongs in an escape hatch, not the core.

## Terminal support & fallbacks (`--tui`)

- **Truecolor** preferred (all hexes above); **256-color** and **16-color ANSI** fallbacks are defined per palette entry so the system stays legible on degraded/SSH terminals.
- **No reliance on color alone** — every state also carries an SGR attribute (`bold`/`reverse`/`dim`/`underline`) and/or a glyph, so meaning survives monochrome.
- **Graphics protocols are optional** — every image view has a text fallback; the app is fully usable on a plain VT100-class terminal, just without inline media.
- **Sub-cell decorations degrade:** squiggles → `underline` + color, ghost text → `dim`, scrollbar marks → gutter glyphs.
