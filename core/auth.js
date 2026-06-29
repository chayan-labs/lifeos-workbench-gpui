/**
 * Life OS Authentication & Tenant Management
 * Implements SaaS-ready tenant-switching and session management.
 */
(() => {
  "use strict";

  const auth = {
    currentUser: {
      id: "chayan",
      email: "chayan@example.com",
      name: "Chayan Aggarwal"
    },
    activeWorkspace: {
      id: "default-personal-workspace",
      name: "Personal Life OS",
      plan: "personal"
    },

    switchWorkspace(workspaceId) {
      console.log(`[Auth] Switching workspace to: ${workspaceId}`);
      this.activeWorkspace.id = workspaceId;
      if (window.osDb) {
        window.osDb.workspaceId = workspaceId;
      }
    }
  };

  window.osAuth = auth;
  console.log("[Auth] Tenant-aware session state loaded.");
})();
