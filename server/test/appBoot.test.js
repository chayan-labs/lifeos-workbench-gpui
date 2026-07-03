import net from "node:net";
import { describe, expect, it } from "vitest";
import { getEphemeralPort } from "../lib/appBoot.js";

describe("getEphemeralPort", () => {
  it("returns a free, bindable port", async () => {
    const port = await getEphemeralPort();
    expect(port).toBeGreaterThan(0);

    // Prove it's actually free by binding it ourselves.
    await new Promise((resolve, reject) => {
      const srv = net.createServer();
      srv.on("error", reject);
      srv.listen(port, "127.0.0.1", () => srv.close(resolve));
    });
  });

  it("returns different ports across calls", async () => {
    const a = await getEphemeralPort();
    const b = await getEphemeralPort();
    expect(a).not.toBe(b);
  });
});
