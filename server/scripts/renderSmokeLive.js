// Manual live run of Validator 2 against the real app stack (cargo binary +
// Vite dev server + headless Chromium) - not run by the vitest suite
// (renderSmoke.test.js exercises the orchestration against fast HTTP
// fixtures instead). Requires `cargo build --bin lifeos-api` and
// `npm install` in frontend/ to have been run already.
//
// Usage: node server/scripts/renderSmokeLive.js
import { validateRenderSmoke } from "../validators/render.js";

const result = await validateRenderSmoke("reading", { name: "Reading" });
console.log(JSON.stringify(result, null, 2));
process.exit(result.valid ? 0 : 1);
