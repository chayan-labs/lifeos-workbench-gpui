// Boots the real app stack (lifeos-api + the Vite frontend) as disposable
// child processes on ephemeral ports, for issue #75's render-smoke validator
// (docs/SELF-EXTENSION.md §4, Validator 2). Never touches the canonical
// lifeos.db - LIFEOS_DB_PATH/LIFEOS_DERIVED_DB_PATH point at a fresh scratch
// directory the caller owns, and TURSO_URL/TURSO_TOKEN are cleared so the API
// never syncs against the real remote even if the host environment has them set.
import { spawn } from "node:child_process";
import net from "node:net";
import path from "node:path";

export async function getEphemeralPort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.on("error", reject);
    srv.listen(0, "127.0.0.1", () => {
      const { port } = srv.address();
      srv.close(() => resolve(port));
    });
  });
}

async function waitForHttp(url, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
      lastError = new Error(`HTTP ${res.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error(`timed out waiting for ${url}: ${lastError?.message ?? "no response"}`);
}

// The prebuilt debug binary is required rather than triggering `cargo build`
// here - a multi-minute compile hidden inside a "smoke test" retry loop would
// make failures slow and confusing. `cargo build --bin lifeos-api` is a
// documented manual prerequisite, same pattern as this repo's other
// "works once you've run the real setup step" gates (docs/MANUAL-SETUP.md).
export async function launchApi({ repoRoot, dbDir, port }) {
  const binary = path.join(repoRoot, "services", "target", "debug", "lifeos-api");
  const child = spawn(binary, [], {
    cwd: path.join(repoRoot, "services"),
    env: {
      ...process.env,
      LIFEOS_DB_PATH: path.join(dbDir, "scratch.db"),
      LIFEOS_DERIVED_DB_PATH: path.join(dbDir, "scratch-derived.db"),
      LIFEOS_BIND_ADDR: `127.0.0.1:${port}`,
      TURSO_URL: "",
      TURSO_TOKEN: "",
    },
    stdio: "ignore",
  });

  const url = `http://127.0.0.1:${port}`;
  try {
    await waitForHttp(`${url}/api/health`, 15000);
  } catch (error) {
    child.kill();
    throw new Error(`lifeos-api did not become healthy (is it built? \`cargo build --bin lifeos-api\`): ${error.message}`);
  }

  return { url, stop: () => child.kill() };
}

export async function launchFrontend({ repoRoot, apiUrl, port }) {
  // --host 127.0.0.1 pins the bind address explicitly - Vite's default
  // "localhost" can resolve to the IPv6 loopback (::1) first on some hosts,
  // which then refuses the IPv4 fetch() this file uses to poll readiness.
  const child = spawn(
    "npx",
    ["vite", "--port", String(port), "--strictPort", "--host", "127.0.0.1"],
    {
      cwd: path.join(repoRoot, "frontend"),
      env: { ...process.env, VITE_API_URL: apiUrl },
      stdio: "ignore",
    },
  );

  const url = `http://127.0.0.1:${port}`;
  try {
    await waitForHttp(url, 20000);
  } catch (error) {
    child.kill();
    throw new Error(`frontend dev server did not come up (is \`npm install\` run in frontend/?): ${error.message}`);
  }

  return { url, stop: () => child.kill() };
}
