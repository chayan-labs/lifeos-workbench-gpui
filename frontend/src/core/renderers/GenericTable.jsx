import React, { useState } from 'react';
import { apiCall } from '../../lib/api';

// Generic table renderer: columns come from a view's `columns` config
// (docs/MODULES.md §1, `{ key, label, editable? }`); inline edits PATCH
// /api/entity/:id on blur with optimistic update + rollback. Reusable by any
// module with zero bespoke table code.
export default function GenericTable({ entities, setEntities, columns, onRowClick, emptyLabel = 'Nothing here yet.' }) {
  const cellValue = (entity, col) => (col.key in entity ? entity[col.key] : entity.attrs?.[col.key]);

  const commitEdit = async (entity, col, value) => {
    const prev = entities;
    setEntities(entities.map((e) => (e.id === entity.id ? { ...e, [col.key]: value } : e)));
    const { ok } = await apiCall('PATCH', `/api/entity/${entity.id}`, { [col.key]: value });
    if (!ok) setEntities(prev);
  };

  if (!entities?.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }

  return (
    <table className="w-full text-xs font-mono">
      <thead>
        <tr className="text-left text-neo-text-muted border-b-2 border-neo-border">
          {columns.map((col) => (
            <th key={col.key} className="py-1.5 pr-3">{col.label}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {entities.map((entity) => (
          <tr key={entity.id} className="border-b border-neo-border/40 hover:bg-neo-surface-muted">
            {columns.map((col) => (
              <td
                key={col.key}
                className={`py-1.5 pr-3 ${onRowClick && !col.editable ? 'cursor-pointer' : ''} ${col.className || ''}`}
                onClick={col.editable ? (e) => e.stopPropagation() : () => onRowClick?.(entity)}
              >
                {col.editable ? (
                  <input
                    defaultValue={cellValue(entity, col) ?? ''}
                    onBlur={(e) => {
                      if (e.target.value !== (cellValue(entity, col) ?? '')) commitEdit(entity, col, e.target.value);
                    }}
                    className="bg-transparent border-b border-dashed border-neo-border focus:outline-none focus:border-neo-blue w-full"
                  />
                ) : (
                  <span className={col.truncate ? 'block truncate max-w-xs' : ''}>{String(cellValue(entity, col) ?? '')}</span>
                )}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
