/**
 * Life OS Core Module Registry
 * Allows declarative modules to register themselves and defines the manifest schema.
 */
(() => {
  "use strict";

  const registry = new Map();

  window.osRegisterModule = (manifest) => {
    if (!manifest.id) {
      console.error("[Registry] Module registration failed: Missing module id.");
      return;
    }

    if (registry.has(manifest.id)) {
      console.warn(`[Registry] Module '${manifest.id}' is already registered. Overwriting.`);
    }

    registry.set(manifest.id, manifest);
    console.log(`[Registry] Registered module: ${manifest.name} (id: ${manifest.id}, version: ${manifest.version || '1.0.0'})`);

    // Dispatch a custom event so the SPA can hot-reload or mount the module
    const event = new CustomEvent(`module-mounted:${manifest.id}`, { detail: { manifest } });
    window.dispatchEvent(event);
  };

  window.osGetModules = () => Array.from(registry.values());
  window.osGetModule = (id) => registry.get(id);

  console.log("[Registry] Core module registry active.");
})();
