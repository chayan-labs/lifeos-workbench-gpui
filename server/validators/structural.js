import fs from 'fs';

/**
 * Pure JS structural validator.
 * Asserts the manifest contains the correct properties and matches schema constraints.
 */
export async function validateStructural(manifestPath) {
  if (!fs.existsSync(manifestPath)) {
    console.error(`[Validator 1] Manifest file not found at: ${manifestPath}`);
    return false;
  }

  const content = fs.readFileSync(manifestPath, 'utf8');
  
  // A simple structural regex/parse check for registration signature
  if (!content.includes('osRegisterModule')) {
    console.error('[Validator 1] Manifest must call osRegisterModule(...)');
    return false;
  }

  // Validate presence of required fields
  const requiredFields = ['id:', 'name:', 'entityTypes:'];
  for (const field of requiredFields) {
    if (!content.includes(field)) {
      console.error(`[Validator 1] Missing required manifest field: ${field}`);
      return false;
    }
  }

  return true;
}
