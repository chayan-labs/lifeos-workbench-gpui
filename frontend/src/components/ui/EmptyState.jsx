import React from 'react';

// Friendly empty placeholder that explains what a surface will hold and how to
// fill it - used so no panel ever renders as a blank/undefined box.
export default function EmptyState({ icon: Icon, title, hint, action }) {
  return (
    <div className="neo-border border-dashed p-8 flex flex-col items-center justify-center text-center gap-2 bg-neo-surface-muted">
      {Icon && <Icon size={28} className="text-neo-text-muted" />}
      <p className="neo-label-sm text-neo-text">{title}</p>
      {hint && <p className="text-xs text-neo-text-muted max-w-sm">{hint}</p>}
      {action && <div className="mt-2">{action}</div>}
    </div>
  );
}
