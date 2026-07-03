// Loads a module.js file's osRegisterModule({...}) argument by running the
// file in a fresh vm context (issue #74, docs/SELF-EXTENSION.md §4) rather
// than regex/string-matching its source - a real module.js is plain script
// that calls the global osRegisterModule(manifest) function once, so vm.Script
// + a context that captures that call is sufficient without a full ESM loader.
import fs from "node:fs/promises";
import vm from "node:vm";

export async function loadManifestFromFile(modulePath) {
  const source = await fs.readFile(modulePath, "utf8");
  return loadManifestFromSource(source, modulePath);
}

export function loadManifestFromSource(source, filename = "module.js") {
  let captured = null;
  const context = vm.createContext({
    osRegisterModule: (manifest) => {
      captured = manifest;
    },
    console,
  });

  new vm.Script(source, { filename }).runInContext(context, { timeout: 5000 });

  return captured;
}
