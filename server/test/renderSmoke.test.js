// Exercises validateRenderSmoke's orchestration (retry, timeout, error
// aggregation, teardown) against real Playwright + real HTTP servers, but
// fake `launchApi`/`launchFrontend` implementations - spinning up the real
// cargo binary + Vite dev server per test would make this suite slow and
// dependent on a local build being present. server/scripts/renderSmokeLive.js
// (manual, not run by vitest) exercises the real stack end-to-end instead.
import http from "node:http";
import { afterEach, describe, expect, it } from "vitest";
import { validateRenderSmoke } from "../validators/render.js";

let servers = [];

function listen(requestHandler) {
  return new Promise((resolve) => {
    const server = http.createServer(requestHandler);
    servers.push(server);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      resolve({ url: `http://127.0.0.1:${port}`, server });
    });
  });
}

afterEach(async () => {
  await Promise.all(servers.map((s) => new Promise((resolve) => s.close(resolve))));
  servers = [];
});

// A fake lifeos-api: /api/health always 200s; POST /api/event records the
// event; GET /api/event?type=module.installed replays them - mirroring the
// real route's contract closely enough for the fake frontend below to poll.
function startFakeApi() {
  const events = [];
  return listen((req, res) => {
    // The real lifeos-api serves cross-origin requests from the Vite dev
    // server's own port - this fake needs the same CORS header, or the
    // browser-side fetch() below fails silently and nothing ever mounts.
    res.setHeader("Access-Control-Allow-Origin", "*");
    if (req.url === "/api/health") {
      res.writeHead(200).end("ok");
      return;
    }
    if (req.method === "POST" && req.url === "/api/event") {
      let body = "";
      req.on("data", (chunk) => (body += chunk));
      req.on("end", () => {
        events.push(JSON.parse(body));
        res.writeHead(200, { "content-type": "application/json" }).end("{}");
      });
      return;
    }
    if (req.method === "GET" && req.url.startsWith("/api/event")) {
      res.writeHead(200, { "content-type": "application/json" }).end(JSON.stringify(events));
      return;
    }
    res.writeHead(404).end();
  });
}

// A fake frontend: polls the given api for module.installed events (mirroring
// useModuleStream.js's real poll fallback) and dispatches the same
// module-mounted:<id> CustomEvent the real moduleRegistry.js emits.
function startFakeFrontend(apiUrl, { throwOnLoad = false } = {}) {
  const html = `<!doctype html><html><body><script>
    ${throwOnLoad ? "throw new Error('simulated render crash');" : ""}
    setInterval(async () => {
      const res = await fetch(${JSON.stringify(apiUrl)} + "/api/event?type=module.installed");
      const events = await res.json();
      for (const ev of events) {
        window.dispatchEvent(new CustomEvent("module-mounted:" + ev.attrs.id));
      }
    }, 100);
  </script></body></html>`;
  return listen((req, res) => {
    res.writeHead(200, { "content-type": "text/html" }).end(html);
  });
}

describe("validateRenderSmoke - happy path", () => {
  it("passes when module-mounted fires with no console/page errors", async () => {
    const { url: apiUrl } = await startFakeApi();
    const { url: frontendUrl } = await startFakeFrontend(apiUrl);

    const result = await validateRenderSmoke(
      "widgets",
      { name: "Widgets" },
      {
        launchApi: async () => ({ url: apiUrl, stop: () => {} }),
        launchFrontend: async () => ({ url: frontendUrl, stop: () => {} }),
      },
    );

    expect(result).toEqual({ valid: true, errors: [] });
  }, 20000);
});

describe("validateRenderSmoke - failures", () => {
  it("fails after the bounded retry when the page throws on load", async () => {
    const { url: apiUrl } = await startFakeApi();
    const { url: frontendUrl } = await startFakeFrontend(apiUrl, { throwOnLoad: true });
    let launchCount = 0;

    const result = await validateRenderSmoke(
      "widgets",
      { name: "Widgets" },
      {
        launchApi: async () => ({ url: apiUrl, stop: () => {} }),
        launchFrontend: async () => {
          launchCount += 1;
          return { url: frontendUrl, stop: () => {} };
        },
      },
    );

    expect(result.valid).toBe(false);
    expect(result.errors[0]).toMatch(/console\/page errors/);
    expect(launchCount).toBe(2); // one bounded retry - exactly two attempts
  }, 20000);

  it("fails when module-mounted never fires (no arbitrary-timeout success)", async () => {
    // Two full MOUNT_TIMEOUT_MS attempts (the bounded retry) plus overhead
    // genuinely exceeds vitest's default 20s test timeout here.
    const { url: apiUrl } = await startFakeApi();
    // A frontend that never polls/dispatches anything - the event genuinely
    // never fires, distinct from the throw-on-load case above.
    const { url: frontendUrl } = await listen((req, res) => {
      res.writeHead(200, { "content-type": "text/html" }).end("<!doctype html><html><body>idle</body></html>");
    });

    const result = await validateRenderSmoke(
      "widgets",
      { name: "Widgets" },
      {
        launchApi: async () => ({ url: apiUrl, stop: () => {} }),
        launchFrontend: async () => ({ url: frontendUrl, stop: () => {} }),
      },
    );

    expect(result.valid).toBe(false);
    expect(result.errors[0]).toMatch(/did not fire within/);
  }, 30000);

  it("succeeds on the retry when the first attempt fails transiently", async () => {
    const { url: apiUrl } = await startFakeApi();
    const { url: frontendUrl } = await startFakeFrontend(apiUrl);
    let attempt = 0;

    const result = await validateRenderSmoke(
      "widgets",
      { name: "Widgets" },
      {
        launchApi: async () => {
          attempt += 1;
          if (attempt === 1) throw new Error("transient boot failure");
          return { url: apiUrl, stop: () => {} };
        },
        launchFrontend: async () => ({ url: frontendUrl, stop: () => {} }),
      },
    );

    expect(result).toEqual({ valid: true, errors: [] });
    expect(attempt).toBe(2);
  }, 20000);

  it("calls stop() on every launched process even after a failure", async () => {
    const { url: apiUrl } = await startFakeApi();
    const { url: frontendUrl } = await startFakeFrontend(apiUrl, { throwOnLoad: true });
    let apiStops = 0;
    let frontendStops = 0;

    await validateRenderSmoke(
      "widgets",
      { name: "Widgets" },
      {
        launchApi: async () => ({ url: apiUrl, stop: () => (apiStops += 1) }),
        launchFrontend: async () => ({ url: frontendUrl, stop: () => (frontendStops += 1) }),
      },
    );

    expect(apiStops).toBe(2);
    expect(frontendStops).toBe(2);
  }, 20000);
});
