import { describe, expect, it } from "vitest";
import { loadManifestFromSource } from "../lib/loadManifest.js";

describe("loadManifestFromSource", () => {
  it("captures the osRegisterModule({...}) argument", () => {
    const source = `osRegisterModule({ id: "foo", entityTypes: {} });`;
    expect(loadManifestFromSource(source)).toEqual({ id: "foo", entityTypes: {} });
  });

  it("returns null when osRegisterModule is never called", () => {
    const source = `const x = 1;`;
    expect(loadManifestFromSource(source)).toBeNull();
  });

  it("does not leak into the host's globals", () => {
    const source = `globalThis.pwned = true; osRegisterModule({ id: "foo" });`;
    loadManifestFromSource(source);
    expect(globalThis.pwned).toBeUndefined();
  });

  it("propagates a syntax error from the source", () => {
    expect(() => loadManifestFromSource("this is not valid js {{{")).toThrow();
  });
});
