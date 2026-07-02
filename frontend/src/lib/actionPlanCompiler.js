// Compiles a natural-language instruction into a structured ActionPlan: an
// ordered list of typed actions drawn only from the closed agentActions.js
// registry (docs/AGENT-CONTROL.md §3). The model is constrained to emit JSON
// naming only real tools; anything it invents is dropped (not executed) so a
// hallucinated tool can never slip through as if it were real.
import { apiCall } from './api';
import { ACTION_TOOLS, classifyAction } from './agentActions';
import { llmSelection } from './ai';

const TOOL_NAMES = Object.keys(ACTION_TOOLS);

const SYSTEM_PROMPT = `You compile a user's instruction into a JSON action plan for an app called Life OS.
Output ONLY a JSON array (no prose, no markdown fences) of objects: { "tool": "<name>", "args": {...}, "reason": "<short why>" }.
The "tool" MUST be one of exactly these: ${TOOL_NAMES.join(', ')}.
For entity.update, args = { "id": "<entity id>", "patch": { ...fields to change } }.
For entity.create, args = { "module": "...", "type": "...", "title": "...", "attrs": {...} }.
If you don't have enough information (e.g. no entity ids were given), output an empty array [].
Never invent a tool name outside the list above.`;

function tryParseJsonArray(text) {
  if (!text) return null;
  // Models sometimes wrap JSON in a fenced block despite instructions; strip it.
  const cleaned = text.replace(/^```(json)?/i, '').replace(/```$/, '').trim();
  try {
    const parsed = JSON.parse(cleaned);
    return Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

// Returns { plan: [{tool, args, reason, classification}], rejected: [...], raw }
export async function compileActionPlan(instruction, context = '') {
  const { ok, data } = await apiCall('POST', '/api/llm', {
    system: SYSTEM_PROMPT,
    prompt: context ? `Context: ${context}\nInstruction: ${instruction}` : instruction,
    ...llmSelection(),
  });

  const raw = ok ? (data?.text || data) : null;
  const parsed = tryParseJsonArray(typeof raw === 'string' ? raw : null) || [];

  const plan = [];
  const rejected = [];
  for (const step of parsed) {
    if (!step || typeof step.tool !== 'string') continue;
    const classification = classifyAction(step.tool);
    if (classification === 'forbidden' && !ACTION_TOOLS[step.tool]) {
      // Genuinely not a real tool (hallucinated or protected-domain name) -
      // never silently coerced into something runnable.
      rejected.push({ ...step, reason: `'${step.tool}' is not a real action tool.` });
      continue;
    }
    plan.push({ ...step, classification });
  }

  return { plan, rejected, raw };
}
