import React from 'react';
import { Wand2 } from 'lucide-react';

// Drop-in "Modify with AI" affordance. Opens the global AI Console prefilled
// with a layer-scoped request, so AI is reachable from every surface.
export default function AIEdit({ prefill, label = 'Modify with AI', className = '' }) {
  const open = () =>
    window.dispatchEvent(new CustomEvent('lifeos:ai', { detail: { prefill } }));
  return (
    <button
      onClick={open}
      className={`neo-btn bg-neo-surface text-neo-text py-1 px-2 text-[10px] flex items-center gap-1 ${className}`}
      title="Ask the AI Console to change this"
    >
      <Wand2 size={11} /> {label}
    </button>
  );
}
