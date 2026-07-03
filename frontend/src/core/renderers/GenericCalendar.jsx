import React, { useMemo } from 'react';
import { resolveField, resolveDisplay } from './displayHelpers';

// Generic calendar renderer: groups entities by day from a manifest-declared
// date field (e.g. schedule_block.start, calendar_event.start) - no
// per-module calendar component needed. Renders as a simple agenda grouped
// by date rather than a full month grid, since the generic data has no fixed
// notion of duration/recurrence yet.
export default function GenericCalendar({ entities, display = {}, dateField = 'start', emptyLabel = 'Nothing scheduled.' }) {
  const groups = useMemo(() => {
    const byDay = new Map();
    for (const entity of entities || []) {
      const raw = resolveField(entity, dateField);
      if (!raw) continue;
      const day = String(raw).slice(0, 10); // ISO date prefix
      if (!byDay.has(day)) byDay.set(day, []);
      byDay.get(day).push(entity);
    }
    return [...byDay.entries()].sort(([a], [b]) => a.localeCompare(b));
  }, [entities, dateField]);

  if (!groups.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }

  return (
    <div className="flex flex-col gap-4">
      {groups.map(([day, items]) => (
        <div key={day} className="neo-border bg-neo-bg p-3">
          <div className="neo-label-sm mb-2">{day}</div>
          <div className="flex flex-col gap-1.5">
            {items.map((entity) => {
              const { title, subtitle, badge } = resolveDisplay(entity, display);
              return (
                <div key={entity.id} className="flex items-center justify-between text-xs">
                  <div>
                    <span className="font-bold">{title}</span>
                    {subtitle && <span className="text-neo-text-muted ml-2">{subtitle}</span>}
                  </div>
                  {badge && <span className="neo-tag text-[9px] font-mono">{badge}</span>}
                </div>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}
