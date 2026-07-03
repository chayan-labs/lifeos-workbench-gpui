/**
 * Files Module
 * Google Drive via owned Google OAuth (self-hosted Nango), plus local
 * content-addressed version history (lifeos-vcs, docs/VERSIONING.md). Reads
 * and local commits are free; upload/share are HUMAN-GATED (docs/SECURITY.md §2).
 */
osRegisterModule({
  id: "files",
  name: "Files",
  icon: "FolderOpen",
  color: "var(--neo-orange)",
  num: 8,
  version: "1.0.0",

  entityTypes: {
    file: {
      label: "File",
      plural: "Files",
      icon: "File",
      attrs: {
        name: { type: "text", required: true },
        mime: { type: "text", required: false },
        size: { type: "text", required: false },
        blob_ref: { type: "text", required: false },
        drive_id: { type: "text", required: false },
        version_no: { type: "number", required: false },
        parent_folder: { type: "text", required: false }
      },
      display: {
        title: "name",
        subtitle: "mime"
      }
    },
    folder: {
      label: "Folder",
      plural: "Folders",
      icon: "Folder",
      attrs: {},
      display: {
        title: "title"
      }
    }
  },

  views: [
    { id: "browse", label: "Browse", kind: "table", type: "file" }
  ],

  events: ["file.imported", "version.created", "file.shared"],

  agentTools: [
    { name: "drive.sync", schema: {}, impl: "sync", gated: false },
    { name: "drive.read", schema: {}, impl: "list", gated: false },
    { name: "file.commit", schema: {}, impl: "commit", gated: false },
    { name: "drive.upload", schema: {}, impl: "upload", gated: true }, // GATED
    { name: "drive.share", schema: {}, impl: "share", gated: true } // GATED
  ],

  integrations: ["google-drive"]
});
