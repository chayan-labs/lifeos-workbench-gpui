import React, { useEffect, useMemo, useState } from 'react';
import { AlertTriangle, Download, FileQuestion } from 'lucide-react';
import MarkdownRenderer from './MarkdownRenderer';
import { fetchBlob } from '../lib/vcsApi';

// Blob content viewer (issue #109, docs/STORAGE-BACKENDS.md §5): fetches any
// blob_ref through the API (which falls back across the workspace's storage
// backends by hash) and renders it. The deliberate, safe default:
// - markdown/text -> inline via MarkdownRenderer (marked GFM + KaTeX)
// - anything else -> a typed placeholder card (name/mime/size/version +
//   download); richer viewers are user-built custom views via the Agent
//   Control Plane, never baked in here.

const MARKDOWN_MIMES = ['text/markdown', 'text/x-markdown'];
const MD_EXTENSIONS = /\.(md|markdown|txt)$/i;

export function isMarkdownLike(mime, name) {
  if (mime && (MARKDOWN_MIMES.includes(mime) || mime.startsWith('text/'))) return true;
  return Boolean(name && MD_EXTENSIONS.test(name));
}

const formatSize = (bytes) => {
  if (bytes == null) return null;
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

function PlaceholderCard({ blobRef, name, mime, size, version, bytes }) {
  const download = () => {
    const blob = new Blob([bytes ?? new Uint8Array()], { type: mime || 'application/octet-stream' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = name || blobRef.slice(0, 12);
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="neo-border bg-neo-surface p-4 flex items-center gap-4">
      <FileQuestion size={28} className="text-neo-text-muted shrink-0" />
      <div className="flex flex-col min-w-0 flex-1">
        <span className="text-sm font-bold text-neo-text truncate">{name || '(unnamed blob)'}</span>
        <span className="text-[11px] font-mono text-neo-text-muted truncate">
          {[mime || 'unknown type', formatSize(size), version, `${blobRef.slice(0, 10)}…`]
            .filter(Boolean)
            .join(' · ')}
        </span>
        <span className="text-[11px] text-neo-text-muted mt-1">
          No inline viewer for this type - build a custom view via Agent Control, or download it.
        </span>
      </div>
      {bytes && (
        <button onClick={download} className="neo-btn bg-neo-surface-high text-neo-text py-1.5 px-3 text-xs flex items-center gap-1 shrink-0">
          <Download size={13} /> Download
        </button>
      )}
    </div>
  );
}

export default function BlobView({ blobRef, name, mime, size, version }) {
  const [bytes, setBytes] = useState(null);
  const [error, setError] = useState('');
  const markdown = isMarkdownLike(mime, name);

  useEffect(() => {
    let cancelled = false;
    setBytes(null);
    setError('');
    if (!blobRef) return undefined;
    fetchBlob(blobRef)
      .then((b) => { if (!cancelled) setBytes(b); })
      .catch((e) => { if (!cancelled) setError(e.message); });
    return () => { cancelled = true; };
  }, [blobRef]);

  const text = useMemo(
    () => (markdown && bytes ? new TextDecoder().decode(bytes) : ''),
    [markdown, bytes],
  );

  if (!blobRef) return null;
  if (error) {
    return (
      <div className="neo-tag bg-neo-red text-white self-start">
        <AlertTriangle size={11} /> {error}
      </div>
    );
  }
  if (!bytes) {
    return <span className="text-[11px] text-neo-text-muted">Fetching {blobRef.slice(0, 10)}…</span>;
  }
  if (markdown) {
    return <MarkdownRenderer content={text} className="prose prose-sm max-w-none" />;
  }
  return <PlaceholderCard blobRef={blobRef} name={name} mime={mime} size={size} version={version} bytes={bytes} />;
}
