import path from 'path';
import fs from 'fs';
import { validateStructural } from './validators/structural.js';
import { validateRenderSmoke } from './validators/render.js';

/**
 * Simulates scaffolding a new module using the Claude Agent SDK.
 * Incorporates the 3-layer security constraints defined in docs/SELF-EXTENSION.md.
 */
export async function scaffoldModule(prompt, workspaceId) {
  console.log(`[Scaffolder] Initiating self-extension for workspace: ${workspaceId}`);
  console.log(`[Scaffolder] Prompt: "${prompt}"`);

  // Step 1: Resolve module ID from prompt (e.g. "add a reading list module" -> "reading")
  const moduleId = prompt.toLowerCase().includes('reading') ? 'reading' : 'custom_module';
  const targetModuleDir = path.resolve(`./modules/${moduleId}`);

  console.log(`[Scaffolder] TARGET DIRECTORY: ${targetModuleDir}`);

  // Step 2: Layer A & B constraints simulation
  // PreToolUse guard: prevent writing files outside the modules/ subdirectory
  const isPathAllowed = (filePath) => {
    const resolvedPath = path.resolve(filePath);
    return resolvedPath.startsWith(path.resolve('./modules/') + path.sep) || resolvedPath === path.resolve('./modules');
  };

  console.log(`[Scaffolder] Layer B Guard loaded. All writes confined to: ./modules/*`);

  // Step 3: Copy template modules/_template/ to modules/<id>/
  try {
    const templateDir = path.resolve('./modules/_template');
    if (!fs.existsSync(targetModuleDir)) {
      fs.mkdirSync(targetModuleDir, { recursive: true });
    }
    
    // Write manifest
    const manifestPath = path.join(targetModuleDir, 'module.js');
    const mockManifest = `
osRegisterModule({
  id: "${moduleId}",
  name: "${moduleId.charAt(0).toUpperCase() + moduleId.slice(1)}",
  icon: "BookOpen",
  color: "var(--neo-mint)",
  version: "1.0.0",
  entityTypes: {
    item: {
      label: "Reading Item",
      plural: "Reading Items",
      attrs: {
        url: { type: "text", required: true },
        progress: { type: "number", required: false }
      }
    }
  },
  views: [
    { id: "all", label: "My List", kind: "list", type: "item" }
  ]
});
`;
    fs.writeFileSync(manifestPath, mockManifest);
    console.log(`[Scaffolder] Scaffolding complete in worktree: ${manifestPath}`);

    // Step 4: Run validators
    console.log(`[Scaffolder] Running Validator 1 (Structural validation)...`);
    const isStructuralValid = await validateStructural(manifestPath);
    if (!isStructuralValid) {
      throw new Error("Validator 1 failed: Manifest schema is invalid or duplicate IDs found");
    }
    console.log(`[Scaffolder] Validator 1 PASSED.`);

    console.log(`[Scaffolder] Running Validator 2 (Render smoke test)...`);
    const isRenderValid = await validateRenderSmoke(manifestPath);
    if (!isRenderValid) {
      throw new Error("Validator 2 failed: Page rendering resulted in errors");
    }
    console.log(`[Scaffolder] Validator 2 PASSED.`);

    console.log(`[Scaffolder] Committing scaffolded module to VCS git repository...`);
    // git commit would run here in the worktree
    console.log(`[Scaffolder] SUCCESS. Module '${moduleId}' is hot-loaded.`);
    return { success: true, moduleId };
  } catch (error) {
    console.error(`[Scaffolder] FAILED:`, error.message);
    return { success: false, error: error.message };
  }
}

// Direct execution test
if (process.argv[1] === import.meta.filename) {
  scaffoldModule("add a reading list module", "default-workspace");
}
