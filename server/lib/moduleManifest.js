// Zod ModuleManifest - single source of truth for the structured-output
// schema the agent's build must satisfy (issue #73, docs/SELF-EXTENSION.md
// §3). This is the *summary* the agent emits alongside writing module.js
// (entityTypes/attrs/views/botCommands/agentTools ids), not a re-parse of
// the file - Validator 1 (#74) consumes this summary directly.
import { z } from "zod";

const AttrSchema = z.object({
  type: z.enum(["text", "number", "boolean", "date", "select", "json"]),
  required: z.boolean(),
});

const EntityTypeSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  plural: z.string().min(1),
  icon: z.string().min(1),
  attrs: z.record(z.string(), AttrSchema),
});

const ViewSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  kind: z.enum(["list", "board", "calendar", "table"]),
  type: z.string().min(1),
});

const BotCommandSchema = z.object({
  cmd: z.string().min(1),
  help: z.string().min(1),
});

const AgentToolSchema = z.object({
  name: z.string().min(1),
  gated: z.boolean(),
});

export const ModuleManifest = z.object({
  id: z.string().regex(/^[a-z][a-z0-9_]*$/, "id must be lower_snake_case"),
  name: z.string().min(1),
  icon: z.string().min(1),
  color: z.string().min(1),
  entityTypes: z.array(EntityTypeSchema).min(1),
  views: z.array(ViewSchema).min(1),
  botCommands: z.array(BotCommandSchema),
  agentTools: z.array(AgentToolSchema),
});

export const moduleManifestJsonSchema = z.toJSONSchema(ModuleManifest);
