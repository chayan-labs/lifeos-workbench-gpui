// Validator 2 - render smoke, headless Playwright (issue #75,
// docs/SELF-EXTENSION.md §4). Replaces the earlier fake stub (unconditional
// `return true`, a `// In a real environment we would...` comment) with a
// real boot of the app stack against a scratch DB on ephemeral ports.
//
// Scope note on "mount the new tile": the live frontend's hot-install path
// (frontend/src/lib/useModuleStream.js -> moduleRegistry.js) only ever
// carries a minimal {id, name, version, icon} manifest through the real
// `module.installed` SSE event - not the full entityTypes/views manifest
// from modules/<id>/module.js (InstalledModulePage.jsx renders hot-installed
// modules as a flat GenericList, not the multi-view ModuleManifestPage; only
// the 14 static day-1 modules get that treatment). So "mount the new tile,
// assert the module-mounted:<id> ready event fires, assert 0 console/page
// errors" is exercised end-to-end for real; per-view DOM assertions aren't
// (there's no live view system for hot-installed modules yet to assert
// against - that's frontend work beyond this issue's scope, matching #74's
// note that Validator 1 also doesn't assert everything §4's prose lists).
import { chromium } from "playwright";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { getEphemeralPort, launchApi as defaultLaunchApi, launchFrontend as defaultLaunchFrontend } from "../lib/appBoot.js";

const DEFAULT_REPO_ROOT = path.resolve(import.meta.dirname, "..", "..");
const MOUNT_TIMEOUT_MS = 10000;
const MAX_ATTEMPTS = 2; // one bounded retry, per #75's checklist

async function runOnce(moduleId, manifest, opts) {
  const repoRoot = opts.repoRoot ?? DEFAULT_REPO_ROOT;
  const launchApi = opts.launchApi ?? defaultLaunchApi;
  const launchFrontend = opts.launchFrontend ?? defaultLaunchFrontend;
  const openBrowser = opts.openBrowser ?? (() => chromium.launch());

  const dbDir = await fs.mkdtemp(path.join(os.tmpdir(), "lifeos-render-smoke-"));
  const apiPort = await getEphemeralPort();
  const frontendPort = await getEphemeralPort();

  let api;
  let frontend;
  let browser;
  let context;
  let page;
  const jsErrors = [];

  try {
    api = await launchApi({ repoRoot, dbDir, port: apiPort });
    frontend = await launchFrontend({ repoRoot, apiUrl: api.url, port: frontendPort });

    browser = await openBrowser();
    context = await browser.newContext();
    // Bypasses the SPA's client-side login gate (App.jsx checks
    // localStorage) - this validator is testing module rendering, not auth.
    await context.addInitScript(() => {
      window.localStorage.setItem("life_os_loggedin", "true");
    });
    // Resolves the instant the first console/page error is observed, so a
    // page that crashes on load fails fast with an accurate message instead
    // of waiting out the full mount timeout below.
    let onFirstError;
    const firstError = new Promise((resolve) => {
      onFirstError = resolve;
    });
    page = await context.newPage();
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        jsErrors.push(msg.text());
        onFirstError();
      }
    });
    page.on("pageerror", (err) => {
      jsErrors.push(err.message);
      onFirstError();
    });

    await page.goto(frontend.url, { waitUntil: "load" }).catch(() => {
      // A page that throws synchronously during initial script execution can
      // abort navigation in some engines - pageerror above still captured
      // the error, so let the firstError race below report it.
    });

    if (jsErrors.length > 0) {
      throw new Error(`console/page errors during render: ${jsErrors.join("; ")}`);
    }

    // Waits on the real CustomEvent moduleRegistry.js dispatches, not an
    // arbitrary timeout - the timeout below is only a safety net so a
    // never-firing event fails the build instead of hanging it.
    const mounted = page.evaluate(
      (id) =>
        new Promise((resolve) => {
          window.addEventListener(`module-mounted:${id}`, () => resolve(), { once: true });
        }),
      moduleId,
    );

    // Seeds the exact event the real self-extension install path emits
    // (docs/SELF-EXTENSION.md §1 step 5) - no auth needed, /api/event falls
    // back to the default workspace when no bearer token is presented
    // (src/auth.rs resolve_workspace), which is what the frontend does too.
    const eventRes = await fetch(`${api.url}/api/event`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ type: "module.installed", attrs: { id: moduleId, name: manifest?.name ?? moduleId } }),
    });
    if (!eventRes.ok) {
      throw new Error(`failed to seed module.installed event: HTTP ${eventRes.status}`);
    }

    const winner = await Promise.race([
      mounted.then(() => "mounted"),
      firstError.then(() => "error"),
      new Promise((resolve) => setTimeout(() => resolve("timeout"), MOUNT_TIMEOUT_MS)),
    ]);

    if (winner === "error" || jsErrors.length > 0) {
      throw new Error(`console/page errors during render: ${jsErrors.join("; ")}`);
    }
    if (winner === "timeout") {
      throw new Error(`module-mounted:${moduleId} did not fire within ${MOUNT_TIMEOUT_MS}ms`);
    }

    return { valid: true, errors: [] };
  } catch (error) {
    return { valid: false, errors: [error.message] };
  } finally {
    await page?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
    frontend?.stop();
    api?.stop();
    await fs.rm(dbDir, { recursive: true, force: true }).catch(() => {});
  }
}

export async function validateRenderSmoke(moduleId, manifest, opts = {}) {
  let result;
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    result = await runOnce(moduleId, manifest, opts);
    if (result.valid) return result;
  }
  return result;
}
