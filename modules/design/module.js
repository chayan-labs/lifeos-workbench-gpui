/**
 * Design Module
 * Inspects Figma design systems, triggers Higgsfield media generation, and logs design system assets.
 */
osRegisterModule({
  id: "design",
  name: "Design & Figma",
  icon: "Figma",
  color: "var(--neo-blue)",
  num: 7,
  version: "1.0.0",

  entityTypes: {
    design_file: {
      label: "Design File",
      plural: "Design Files",
      icon: "Image",
      attrs: {
        figma_url: { type: "text", required: true },
        last_synced: { type: "date", required: false }
      },
      display: {
        title: "title",
        subtitle: "figma_url"
      }
    }
  },

  views: [
    { id: "assets_gallery", label: "Media Assets", kind: "gallery", type: "design_file" }
  ],

  events: ["asset.generated", "design.synced", "version.created"],

  botCommands: [
    { cmd: "design", help: "Analyze current design assets", handler: "design_status" }
  ],

  agentTools: [
    { name: "design.inspect", schema: {}, impl: "figma_inspect", gated: false },
    { name: "design.generate", schema: {}, impl: "higgsfield_gen", gated: false }
  ]
});
