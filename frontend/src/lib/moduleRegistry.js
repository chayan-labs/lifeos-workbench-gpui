// React-side module manifest registry, mirroring core/registry.js's
// osRegisterModule/osGetModules contract (docs/MODULES.md §1) so the same
// manifest shape works whether the SPA shell is the legacy vanilla-JS core
// or this React app. Registering a manifest dispatches a `module-mounted:<id>`
// window CustomEvent - the same event name self-extension's headless
// Playwright validator asserts on (docs/SELF-EXTENSION.md).

const registry = new Map();
const STORAGE_KEY = 'life_os_installed_modules';

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...registry.values()]));
  } catch {
    // localStorage can throw in private-browsing/quota-exceeded states;
    // the in-memory registry still works for the current session.
  }
}

export function registerModule(manifest) {
  if (!manifest?.id) {
    console.error('[ModuleRegistry] registration failed: missing module id', manifest);
    return;
  }
  registry.set(manifest.id, manifest);
  persist();
  window.dispatchEvent(new CustomEvent(`module-mounted:${manifest.id}`, { detail: { manifest } }));
  window.dispatchEvent(new CustomEvent('lifeos:module-mounted', { detail: { id: manifest.id, manifest } }));
}

export function getModules() {
  return [...registry.values()];
}

export function getModule(id) {
  return registry.get(id);
}

// Restore modules installed in a prior session so their tabs survive reload
// without waiting for a fresh SSE/poll event.
export function hydrateFromStorage() {
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) || '[]');
    saved.forEach((m) => registry.set(m.id, m));
  } catch {
    // Corrupt/missing storage - start with an empty registry.
  }
  return getModules();
}
