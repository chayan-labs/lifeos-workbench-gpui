import React from 'react';
import { ImageOff } from 'lucide-react';
import { resolveDisplay, resolveField } from './displayHelpers';

// Generic gallery renderer: media entities (Design assets, ingest segments,
// ...) as a thumbnail grid driven by manifest display config. A blob_ref is
// resolved to a static asset URL via the API's planned /api/vcs/blob route
// (docs/STORAGE-BACKENDS.md); until that ships, anything without a directly
// fetchable URL renders an honest placeholder instead of a broken <img>.
export default function GenericGallery({ entities, display = {}, mediaField = 'blob_ref', emptyLabel = 'No media yet.' }) {
  if (!entities?.length) {
    return <p className="text-xs text-neo-text-muted">{emptyLabel}</p>;
  }

  const resolveSrc = (entity) => {
    const ref = resolveField(entity, mediaField);
    if (!ref) return null;
    // Direct http(s) URLs (e.g. an already-hosted asset) render as-is;
    // content-addressed blob_refs have no public route yet (lifeos-vcs is a
    // later epic), so they fall back to the placeholder.
    return /^https?:\/\//.test(ref) ? ref : null;
  };

  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
      {entities.map((entity) => {
        const { title, badge } = resolveDisplay(entity, display);
        const src = resolveSrc(entity);
        return (
          <div key={entity.id} className="neo-border bg-neo-bg flex flex-col overflow-hidden">
            <div className="aspect-square bg-neo-surface-muted flex items-center justify-center">
              {src ? (
                <img src={src} alt={title} className="w-full h-full object-cover" />
              ) : (
                <ImageOff size={28} className="text-neo-text-muted" />
              )}
            </div>
            <div className="p-2 flex flex-col gap-1">
              <span className="text-xs font-bold truncate">{title}</span>
              {badge && <span className="neo-tag text-[8px] font-mono self-start">{badge}</span>}
            </div>
          </div>
        );
      })}
    </div>
  );
}
