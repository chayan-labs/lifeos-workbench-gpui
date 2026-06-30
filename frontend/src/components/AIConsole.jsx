import React, { useState, useEffect, useRef } from 'react';
import { Sparkles, X, ShieldAlert, GitCommit, Wand2, Lock, CornerDownLeft } from 'lucide-react';
import MarkdownRenderer from './MarkdownRenderer';
import { routeIntent, canAI } from '../lib/capabilities';
import { apiCall } from '../lib/api';
import { commit as vcsCommit } from '../lib/vcs';
import { selectedAgent } from '../lib/ai';

// The app-wide AI surface. Mounted once in Layout; openable from anywhere via:
//   window.dispatchEvent(new CustomEvent('lifeos:ai', { detail: { prefill, layer } }))
// It routes a natural-language change request to the layers it touches, enforces
// the guardrail registry (gated layers + no-delete-core), proposes a change, and
// lets the HUMAN commit it to VCS (AI can never commit).

const EXAMPLES = [
  'Make the theme warmer and increase contrast',
  'Add a "Spanish" knowledge domain with a starter roadmap',
  'Recommend 3 projects for my Trading domain',
  'Delete the version history', // demonstrates a gated refusal
];

export default function AIConsole() {
  const [open, setOpen] = useState(false);
  const [input, setInput] = useState('');
  const [log, setLog] = useState([]);
  const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState(null); // proposed change awaiting human commit
  const endRef = useRef(null);

  useEffect(() => {
    const onOpen = (e) => {
      setOpen(true);
      if (e.detail?.prefill) setInput(e.detail.prefill);
    };
    window.addEventListener('lifeos:ai', onOpen);
    return () => window.removeEventListener('lifeos:ai', onOpen);
  }, []);

  useEffect(() => { endRef.current?.scrollIntoView({ behavior: 'smooth' }); }, [log, busy, pending]);

  const isDelete = (t) => /\b(delete|remove|drop|wipe|erase|destroy)\b/i.test(t);

  const run = async () => {
    const text = input.trim();
    if (!text || busy) return;
    setInput('');
    setPending(null);
    setLog((l) => [...l, { role: 'user', text }]);
    setBusy(true);

    const layers = routeIntent(text);
    const action = isDelete(text) ? 'delete' : 'modify';
    const verdicts = layers.map((layer) => ({ layer, ...canAI(action, layer.id) }));
    const blocked = verdicts.filter((v) => !v.allowed);

    if (blocked.length) {
      // Guardrail stop - explain why, propose nothing.
      const reasons = blocked.map((b) => `- **${b.layer.label}** - ${b.reason}`).join('\n');
      setLog((l) => [...l, {
        role: 'ai',
        blocked: true,
        text: `I can't do that - it hits a guardrail:\n\n${reasons}\n\n_These are protected so the app can't be broken. You can make this change yourself._`,
      }]);
      setBusy(false);
      return;
    }

    // Allowed: ask the model lane for a plan; fall back to a deterministic one.
    const scope = layers.map((l) => l.label).join(', ');
    const { ok, data } = await apiCall('POST', '/api/llm', {
      system: 'You are the in-app builder for Life OS. Describe the concrete change you would make, briefly.',
      prompt: `User request: ${text}\nLayers in scope: ${scope}`,
      agent: selectedAgent(),
    });
    const plan = (ok && (data?.text || data)) ||
      `**Proposed change** (local plan - \`/api/llm\` not connected):\n\n- Target: ${scope}\n- ${text}\n\nThis is reversible: once applied it's committed to VCS, so you can time-travel back anytime.`;

    setLog((l) => [...l, { role: 'ai', text: plan }]);
    setPending({ text, scope });
    setBusy(false);
  };

  const applyAndCommit = () => {
    // The change itself is applied by the relevant surface in a full build; here
    // the human seals it into version history (AI is gated from committing).
    const c = vcsCommit(`AI-assisted: ${pending.text}`, 'user');
    setLog((l) => [...l, { role: 'system', text: `Committed to VCS as \`${c.id}\` - "${c.message}". You can jump back to this point anytime.` }]);
    setPending(null);
  };

  return (
    <>
      {/* Floating launcher - reachable from every page */}
      {!open && (
        <button
          onClick={() => setOpen(true)}
          className="fixed bottom-6 right-6 z-[120] neo-btn bg-neo-blue text-white py-3 px-4 flex items-center gap-2 neo-shadow-lg"
          title="Ask AI to change anything"
        >
          <Sparkles size={18} /> <span className="hidden sm:inline font-bold">AI Console</span>
        </button>
      )}

      {open && (
        <aside className="fixed right-0 top-0 bottom-0 w-full sm:w-[420px] bg-[var(--neo-surface)] border-l-4 border-neo-border neo-shadow-xl z-[130] flex flex-col">
          <div className="p-4 border-b-4 border-neo-border flex justify-between items-center bg-neo-blue text-white">
            <h3 className="neo-title-md text-base flex items-center gap-2"><Wand2 size={18} /> AI Console</h3>
            <button onClick={() => setOpen(false)} className="neo-icon-btn p-1.5 text-neo-text"><X size={16} /></button>
          </div>

          <div className="px-4 py-2 border-b-2 border-neo-border bg-neo-surface-muted text-[11px] text-neo-text-muted flex items-center gap-1.5">
            <Lock size={11} /> AI can reshape any non-gated layer. It can never touch VCS, secrets, guardrails or billing - or delete core.
          </div>

          <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-3">
            {log.length === 0 && (
              <div className="flex flex-col gap-2">
                <p className="text-xs text-neo-text-muted">Ask me to change anything in the app. Examples:</p>
                {EXAMPLES.map((ex) => (
                  <button key={ex} onClick={() => setInput(ex)} className="neo-btn bg-neo-surface text-neo-text py-1.5 px-2 text-[11px] text-left">{ex}</button>
                ))}
              </div>
            )}
            {log.map((m, i) => (
              <div
                key={i}
                className={`p-2.5 text-xs neo-border ${
                  m.role === 'user' ? 'bg-neo-blue text-white self-end max-w-[85%]'
                  : m.role === 'system' ? 'bg-neo-mint text-neo-text'
                  : m.blocked ? 'bg-neo-red/10 border-neo-red text-neo-text'
                  : 'bg-neo-surface-muted text-neo-text'
                }`}
              >
                {m.blocked && <div className="flex items-center gap-1 font-bold text-neo-red mb-1"><ShieldAlert size={13} /> Guardrail</div>}
                <MarkdownRenderer content={m.text} className={m.role === 'user' ? 'text-white' : ''} />
              </div>
            ))}
            {busy && <div className="p-2.5 text-xs neo-border bg-neo-surface-muted text-neo-text animate-pulse">Planning…</div>}
            {pending && (
              <button onClick={applyAndCommit} className="neo-btn bg-neo-mint text-neo-text py-2 px-3 text-xs flex items-center justify-center gap-2 self-start">
                <GitCommit size={14} /> Apply & commit to VCS
              </button>
            )}
            <div ref={endRef} />
          </div>

          <div className="p-3 border-t-2 border-neo-border flex flex-col gap-2">
            <textarea
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); run(); } }}
              placeholder="Change anything… (Enter to send)"
              className="neo-input text-sm min-h-[60px] w-full"
            />
            <button onClick={run} disabled={busy} className="neo-btn bg-neo-blue text-white py-2 text-xs font-bold flex items-center justify-center gap-2 disabled:opacity-50">
              <CornerDownLeft size={14} /> Send to AI
            </button>
          </div>
        </aside>
      )}
    </>
  );
}
