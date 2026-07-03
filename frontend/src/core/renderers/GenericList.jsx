import React from 'react';
import { resolveDisplay } from './displayHelpers';

// Generic list renderer: any module's entity type renders here driven purely
// by its `entityTypes.<type>.display` config (docs/MODULES.md §1). No module
// ever needs its own list component.
export default function GenericList({ entities, display = {}, onSelect, emptyLabel = 'Nothing here yet.' }) {
  if (!entities?.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }
  return (
    <ul className="flex flex-col gap-2">
      {entities.map((entity) => {
        const { title, subtitle, badge } = resolveDisplay(entity, display);
        return (
          <li
            key={entity.id}
            onClick={() => onSelect?.(entity)}
            className={`neo-border bg-neo-bg p-3 flex items-center justify-between gap-3 ${onSelect ? 'cursor-pointer hover:bg-neo-surface-muted' : ''}`}
          >
            <div className="min-w-0">
              <div className="text-sm font-bold truncate">{title}</div>
              {subtitle && <div className="text-xs text-neo-text-muted truncate">{subtitle}</div>}
            </div>
            {badge && <span className="neo-tag text-[9px] font-mono shrink-0">{badge}</span>}
          </li>
        );
      })}
    </ul>
  );
}
