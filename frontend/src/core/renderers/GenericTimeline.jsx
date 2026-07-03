import React, { useMemo } from 'react';
import { resolveField, resolveDisplay } from './displayHelpers';

// Generic timeline renderer: a manifest-declared date field (Travel legs'
// `start`, media segments' `ts`) orders entities on a vertical timeline. No
// per-module timeline component needed.
export default function GenericTimeline({ entities, display = {}, dateField = 'start', emptyLabel = 'Nothing on the timeline yet.' }) {
  const ordered = useMemo(
    () => [...(entities || [])].sort((a, b) =>
      String(resolveField(a, dateField) || '').localeCompare(String(resolveField(b, dateField) || ''))
    ),
    [entities, dateField]
  );

  if (!ordered.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }

  return (
    <div className="flex flex-col">
      {ordered.map((entity, i) => {
        const { title, subtitle, badge } = resolveDisplay(entity, display);
        const when = resolveField(entity, dateField);
        return (
          <div key={entity.id} className="flex gap-3">
            <div className="flex flex-col items-center">
              <div className="w-2.5 h-2.5 rounded-full bg-neo-blue mt-1.5" />
              {i < ordered.length - 1 && <div className="w-px flex-1 bg-neo-border" />}
            </div>
            <div className="pb-5 flex-1">
              <div className="text-[10px] font-mono text-neo-text-muted">{when}</div>
              <div className="text-sm font-bold">{title}</div>
              {subtitle && <div className="text-xs text-neo-text-muted">{subtitle}</div>}
              {badge && <span className="neo-tag text-[9px] font-mono mt-1 inline-block">{badge}</span>}
            </div>
          </div>
        );
      })}
    </div>
  );
}
