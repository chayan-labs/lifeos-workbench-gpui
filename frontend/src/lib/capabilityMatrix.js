// The canonical, read-only {allowed|gated|forbidden} capability matrix
// (docs/AGENT-CONTROL.md §5) - merges the typed action-tool registry
// (agentActions.js, issue #33) with the legacy free-text guardrail layers
// (capabilities.js, AIConsole's routing model) into one view of "everything
// the agent can/cannot do," covering both granularities the app actually has.
//
// THIS FILE IS ITSELF READ-ONLY FROM THE AGENT'S PERSPECTIVE: it only reads
// from agentActions.js/capabilities.js and exposes a getter, never a setter.
// There is deliberately no UI anywhere that writes to either source file -
// the matrix cannot widen the agent's own reach from inside the app.
import { ACTION_TOOLS, PROTECTED_TOOLS } from './agentActions';
import { LAYERS } from './capabilities';

// A legacy layer's {aiCanRead, aiCanModify, gated, core} maps onto the
// canonical three-state model: `gated: true` layers are fully blocked
// (handled as `forbidden` here for displaying the AI's complete inability to
// touch them - they are read AND write blocked, not merely human-approval
// gated, but lumped here for the at-a-glance matrix), everything else the
// agent can already auto-apply maps to `allowed`. No legacy layer currently
// needs human approval mid-flight, so none maps to `gated` - that state is
// reserved for action tools (draft.create, pipeline.run).
function classifyLayer(layer) {
  if (layer.gated) return 'forbidden';
  return 'allowed';
}

export function getCapabilityMatrix() {
  const actionRows = Object.entries(ACTION_TOOLS).map(([tool, def]) => ({
    kind: 'action tool',
    id: tool,
    label: tool,
    classification: def.classification,
  }));

  const protectedRows = PROTECTED_TOOLS.map((tool) => ({
    kind: 'action tool',
    id: tool,
    label: tool,
    classification: 'forbidden',
  }));

  const layerRows = LAYERS.map((layer) => ({
    kind: 'app layer',
    id: layer.id,
    label: layer.label,
    classification: classifyLayer(layer),
  }));

  return [...actionRows, ...protectedRows, ...layerRows];
}
