import React from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { resolveDisplay } from './displayHelpers';
import { apiCall } from '../../lib/api';

// Generic Kanban renderer: columns come from a view's `groupBy` lifecycle
// (docs/MODULES.md §1 `lifecycle: [...]`), cards from `entityTypes.display`.
// Moving a card optimistically PATCHes /api/entity/:id with rollback on
// failure - the same pattern as Modules.jsx's hand-written task board, now
// reusable by any module with zero bespoke code.
export default function GenericBoard({
  entities,
  setEntities,
  display = {},
  columns,
  groupByField = 'status',
  emptyLabel = 'Nothing here yet.',
  // Override the default PATCH-to-API behavior, e.g. when entities are a
  // local-only mock with no backing /api/entity row.
  onMove,
}) {
  const byColumn = (col) => entities.filter((e) => (e[groupByField] ?? e.attrs?.[groupByField]) === col);

  const move = async (entity, nextCol) => {
    if (onMove) return onMove(entity, nextCol);
    const prev = entities;
    setEntities(entities.map((e) => (e.id === entity.id ? { ...e, [groupByField]: nextCol } : e)));
    const { ok } = await apiCall('PATCH', `/api/entity/${entity.id}`, { [groupByField]: nextCol });
    if (!ok) setEntities(prev); // rollback - the move never reached the canonical DB
  };

  if (!entities?.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
      {columns.map((col, colIdx) => (
        <div key={col} className="flex flex-col gap-3 neo-border bg-neo-surface-muted p-3 min-h-[160px]">
          <div className="flex items-center justify-between neo-label-sm">
            <span>{col}</span>
            <span className="neo-tag text-[9px]">{byColumn(col).length}</span>
          </div>
          {byColumn(col).map((entity) => {
            const { title, subtitle, badge } = resolveDisplay(entity, display);
            return (
              <div key={entity.id} className="neo-border bg-neo-bg p-2.5 flex flex-col gap-1.5">
                <div className="text-xs font-bold">{title}</div>
                {subtitle && <div className="text-[10px] text-neo-text-muted">{subtitle}</div>}
                {badge && <span className="neo-tag text-[9px] font-mono self-start">{badge}</span>}
                <div className="flex justify-between mt-1">
                  <button
                    disabled={colIdx === 0}
                    onClick={() => move(entity, columns[colIdx - 1])}
                    className="neo-icon-btn p-1 disabled:opacity-20"
                    title={`Move to ${columns[colIdx - 1]}`}
                  >
                    <ChevronLeft size={12} />
                  </button>
                  <button
                    disabled={colIdx === columns.length - 1}
                    onClick={() => move(entity, columns[colIdx + 1])}
                    className="neo-icon-btn p-1 disabled:opacity-20"
                    title={`Move to ${columns[colIdx + 1]}`}
                  >
                    <ChevronRight size={12} />
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}
