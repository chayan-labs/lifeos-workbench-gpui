/**
 * Life OS Local Database Wrapper
 * Coordinates with the Turso local embedded replica (offline: true)
 * and isolates transactions to the active workspace.
 */
(() => {
  "use strict";

  const db = {
    workspaceId: "default-personal-workspace",

    async query(endpoint, payload = {}) {
      const response = await fetch(`http://127.0.0.1:8080/api/${endpoint}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ...payload, workspace_id: this.workspaceId })
      });
      return await response.json();
    },

    async createEntity(module, type, title, attrs) {
      console.log(`[Database] Creating entity in '${module}'`);
      return this.query("entity", { module, type, title, attrs });
    }
  };

  window.osDb = db;
  console.log("[Database] Multi-tenant DB interface active.");
})();
