// Naive module-id derivation from a self-extension prompt (issue #72). The
// PreToolUse hook (preToolUseHook.js) needs a concrete target directory
// BEFORE the agent runs, so something outside the LLM call has to pick the
// id first. #73 added the structured-output manifest, but its `id` field
// is only known *after* the agent runs - so this pre-agent slug remains the
// hook's target directory; scaffold.js now asserts the two agree.
const MAX_LEN = 40;
const FALLBACK = "custom_module";

export function slugify(text) {
  const slug = text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, MAX_LEN)
    .replace(/_+$/g, "");

  return slug || FALLBACK;
}
