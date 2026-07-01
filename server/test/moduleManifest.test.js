import { describe, expect, it } from "vitest";
import { ModuleManifest, moduleManifestJsonSchema } from "../lib/moduleManifest.js";

const VALID_MANIFEST = {
  id: "reading_list",
  name: "Reading List",
  icon: "BookOpen",
  color: "var(--neo-yellow)",
  entityTypes: [
    {
      id: "item",
      label: "Item",
      plural: "Items",
      icon: "FileText",
      attrs: { name: { type: "text", required: true } },
    },
  ],
  views: [{ id: "all", label: "All Items", kind: "list", type: "item" }],
  botCommands: [{ cmd: "add", help: "Add a reading list item" }],
  agentTools: [{ name: "reading_list.add", gated: false }],
};

describe("ModuleManifest", () => {
  it("accepts a well-formed manifest", () => {
    const result = ModuleManifest.safeParse(VALID_MANIFEST);
    expect(result.success).toBe(true);
  });

  it("rejects an id that isn't lower_snake_case", () => {
    const result = ModuleManifest.safeParse({ ...VALID_MANIFEST, id: "Reading-List" });
    expect(result.success).toBe(false);
  });

  it("rejects a manifest with no entityTypes", () => {
    const result = ModuleManifest.safeParse({ ...VALID_MANIFEST, entityTypes: [] });
    expect(result.success).toBe(false);
  });

  it("rejects a manifest with no views", () => {
    const result = ModuleManifest.safeParse({ ...VALID_MANIFEST, views: [] });
    expect(result.success).toBe(false);
  });

  it("rejects an attr with an unknown type", () => {
    const bad = {
      ...VALID_MANIFEST,
      entityTypes: [{ ...VALID_MANIFEST.entityTypes[0], attrs: { name: { type: "wat", required: true } } }],
    };
    expect(ModuleManifest.safeParse(bad).success).toBe(false);
  });

  it("rejects a missing top-level field", () => {
    const { color, ...withoutColor } = VALID_MANIFEST;
    expect(ModuleManifest.safeParse(withoutColor).success).toBe(false);
  });
});

describe("moduleManifestJsonSchema", () => {
  it("is a JSON Schema object matching Zod's own output for this shape", () => {
    expect(moduleManifestJsonSchema.type).toBe("object");
    expect(moduleManifestJsonSchema.required).toEqual(
      expect.arrayContaining(["id", "name", "icon", "color", "entityTypes", "views", "botCommands", "agentTools"]),
    );
  });
});
