# `_template` - the module scaffold skeleton

This is the directory `server/scaffold.js` clones when the self-extension builder creates a new module (`docs/SELF-EXTENSION.md`).
It is also the canonical, runnable example of the manifest contract documented in `docs/MODULES.md` §1.

A module is a single file, `module.js`, that calls `osRegisterModule({...})` once.
There is no DOM code, no router code, and no DB code in a module - rendering is entirely generic, driven by what the manifest declares (see `views.md`).
Adding a module never requires a migration: every field lives in `entities.attrs` (a JSON blob), keyed by `workspace_id` + `module` + `type`.

## What scaffolding does

1. `server/scaffold.js` copies this directory to `modules/<new_id>/`.
2. It replaces the placeholder values (`template_id`, `Template Module`, `item`, ...) with the new module's real id/name/entity types, generated from the user's natural-language request.
3. Two validators run against the result before it is ever mounted:
   - `server/validators/structural.js` - pure JS: confirms the file calls `osRegisterModule(...)` and declares `id`, `name`, and `entityTypes`.
   - `server/validators/render.js` - boots the app and asserts the new module mounts with zero console/JS errors (a Playwright smoke check).
4. Only a module that passes both is committed (`git commit`, one commit per install - every install is revertable).

This template itself passes both validators unmodified - that is the acceptance bar for the scaffold, not just for what it produces.

## Manifest fields (see `views.md` for the rendering-relevant ones in depth)

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Unique slug. Used as `module` on every entity this module owns, and as the route segment (`/m/<id>`). |
| `name` | yes | Display name in navigation and headers. |
| `icon` | no | Lucide icon name (frontend resolves it) or an emoji. |
| `color` | no | Accent color, usually a CSS variable from the design system. |
| `num` | no | Display ordering hint. |
| `version` | no | Semver string; defaults to `1.0.0`. |
| `entityTypes` | yes | One entry per entity `type` this module creates - see `views.md`. |
| `views` | no | How `entityTypes` render - see `views.md`. A module with no views still works; its entities just have no dedicated UI yet. |
| `events` | no | Event `type` strings this module emits to the append-only `events` log (for telemetry/automation, not storage). |
| `botCommands` | no | Telegram surface: `{ cmd, help, handler }`. |
| `agentTools` | no | Harness-callable actions: `{ name, schema, impl, gated? }`. `gated: true` means the tool only ever produces a draft - never auto-executes an outward/irreversible action (`docs/SECURITY.md`). |

## Minimal working example

`module.js` in this directory is a complete, valid module: one entity type (`item`), one list view, two declared events, one bot command, and one ungated agent tool.
Copy it, rename `template_id`/`item`/etc., and you have a new module.
