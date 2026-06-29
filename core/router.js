/**
 * Life OS SPA router
 * Maps hash URLs to core views and dynamically renders registered module manifests.
 */
(() => {
  "use strict";

  const router = {
    routes: {},

    init() {
      window.addEventListener("hashchange", () => this.handleRoute());
      this.handleRoute();
    },

    handleRoute() {
      const hash = window.location.hash || "#/dashboard";
      console.log(`[Router] Navigating to hash: ${hash}`);
      
      const contentArea = document.getElementById("main-content");
      if (!contentArea) return;

      // Handle custom module rendering
      if (hash.startsWith("#/m/")) {
        const parts = hash.split("/");
        const moduleId = parts[2];
        const viewId = parts[3];
        this.renderModuleView(moduleId, viewId, contentArea);
      } else {
        // Fallback or static dashboard routes
        contentArea.innerHTML = `<h3>Life OS View</h3><p>Renders view for ${hash}</p>`;
      }
    },

    renderModuleView(moduleId, viewId, container) {
      if (!window.osGetModule) return;
      const m = window.osGetModule(moduleId);
      if (!m) {
        container.innerHTML = `<div class="error">Module '${moduleId}' not found.</div>`;
        return;
      }

      const view = m.views.find(v => v.id === viewId) || m.views[0];
      container.innerHTML = `
        <div class="neo-surface p-6 border-4 border-black mb-4" style="background-color: ${m.color || 'white'}">
          <h2 class="text-2xl font-bold mb-2">${m.name} - ${view.label}</h2>
          <p class="text-sm">Rendering entity type <strong>${view.type}</strong> inside a generic <strong>${view.kind}</strong> view.</p>
        </div>
      `;
    }
  };

  window.osRouter = router;
  console.log("[Router] SPA clientside router initialized.");
})();
