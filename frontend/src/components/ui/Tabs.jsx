import React from 'react';

// Neo-brutalist tab strip. `tabs` = [{ id, label, icon? }]. Controlled.
export default function Tabs({ tabs, active, onChange, className = '' }) {
  return (
    <div className={`flex flex-wrap gap-1.5 ${className}`}>
      {tabs.map((t) => {
        const isActive = active === t.id;
        return (
          <button
            key={t.id}
            onClick={() => onChange(t.id)}
            className={`neo-btn py-1.5 px-3 text-xs flex items-center gap-1.5 ${
              isActive ? 'bg-neo-blue text-white' : 'bg-neo-surface text-neo-text'
            }`}
          >
            {t.icon && <t.icon size={13} />}
            {t.label}
          </button>
        );
      })}
    </div>
  );
}
