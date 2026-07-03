import React, { useMemo, useState } from 'react';
import { BookOpen, Search, ChevronRight, FileText } from 'lucide-react';
import MarkdownRenderer from '../components/MarkdownRenderer';

// Real docs/*.md, loaded at build time straight from the repo's authoritative
// docs/ tree (no hardcoded content) - stays in sync automatically since it's
// a glob over the actual files, not a snapshot (issue #37).
const DOC_MODULES = import.meta.glob('../../../docs/*.md', { query: '?raw', import: 'default', eager: true });

function titleFromFilename(path) {
  const base = path.split('/').pop().replace(/\.md$/, '');
  return base.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase());
}

function firstHeading(content) {
  const m = content.match(/^#\s+(.+)$/m);
  return m ? m[1].trim() : null;
}

function extractHeadings(content) {
  const lines = content.split('\n');
  const headings = [];
  for (const line of lines) {
    const m = /^(#{1,3})\s+(.+)$/.exec(line);
    if (m) headings.push({ level: m[1].length, text: m[2].trim() });
  }
  return headings;
}

function slugify(text) {
  return text.toLowerCase().trim().replace(/[^a-z0-9]+/g, '-').replace(/(^-|-$)/g, '');
}

const DOCS = Object.entries(DOC_MODULES)
  .map(([path, content]) => {
    const id = path.split('/').pop().replace(/\.md$/, '');
    return {
      id,
      path: `docs/${path.split('/').pop()}`,
      title: firstHeading(content) || titleFromFilename(path),
      content,
      headings: extractHeadings(content),
    };
  })
  .sort((a, b) => a.title.localeCompare(b.title));

export default function DocsHub() {
  const [selectedDocId, setSelectedDocId] = useState(DOCS.find((d) => d.id === 'ARCHITECTURE')?.id || DOCS[0]?.id);
  const [searchQuery, setSearchQuery] = useState('');

  const filteredDocs = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return DOCS;
    return DOCS.filter((d) => d.title.toLowerCase().includes(q) || d.content.toLowerCase().includes(q));
  }, [searchQuery]);

  const selectedDoc = DOCS.find((d) => d.id === selectedDocId) || filteredDocs[0];

  if (!DOCS.length) {
    return (
      <div className="neo-surface neo-border-thick neo-shadow p-6">
        <p className="text-sm text-neo-text-muted">No markdown files found under <code>docs/</code>.</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-yellow">
        <h2 className="neo-title-lg text-neo-text mb-1.5 flex items-center gap-2">
          <BookOpen size={28} />
          Docs
        </h2>
        <p className="neo-body-md text-neo-text font-semibold">
          The live specification, read straight from <code>docs/*.md</code> in this repo - {DOCS.length} documents.
        </p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 items-start">
        {/* Document list */}
        <div className="lg:col-span-3 flex flex-col gap-2">
          <label className="relative">
            <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-neo-text-muted" />
            <input
              className="neo-input text-xs pl-7 w-full"
              placeholder="Search docs…"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
            />
          </label>
          <span className="neo-label-sm text-neo-text-muted text-[10px] px-1">DOCS/ TREE</span>
          <div className="flex flex-col gap-1.5 max-h-[600px] overflow-y-auto pr-1">
            {filteredDocs.map((doc) => (
              <button
                key={doc.id}
                onClick={() => setSelectedDocId(doc.id)}
                className={`neo-btn text-left p-3 flex items-center justify-between transition-all ${
                  selectedDoc?.id === doc.id ? 'bg-neo-yellow neo-shadow' : 'bg-neo-surface'
                }`}
              >
                <div className="flex items-center gap-2 min-w-0">
                  <FileText size={14} className="shrink-0 text-neo-text" />
                  <span className="neo-label-md text-xs truncate block">{doc.title}</span>
                </div>
                <ChevronRight size={12} className="shrink-0 text-neo-text" />
              </button>
            ))}
            {!filteredDocs.length && <p className="text-xs text-neo-text-muted px-1">No docs match "{searchQuery}".</p>}
          </div>
        </div>

        {/* In-doc heading index */}
        <div className="lg:col-span-3 flex flex-col gap-2">
          <span className="neo-label-sm text-neo-text-muted text-[10px] px-1">SECTIONS IN THIS DOC</span>
          <div className="flex flex-col gap-1 bg-neo-surface p-3 neo-border neo-radius min-h-[160px] max-h-[600px] overflow-y-auto">
            {selectedDoc?.headings.map((h, i) => (
              <a
                key={i}
                href={`#${slugify(h.text)}`}
                className="text-left py-1 px-2 text-xs font-mono text-neo-text-muted hover:text-neo-blue truncate"
                style={{ paddingLeft: `${(h.level - 1) * 10 + 8}px` }}
              >
                {h.text}
              </a>
            ))}
            {!selectedDoc?.headings.length && <p className="text-xs text-neo-text-muted">No headings.</p>}
          </div>
        </div>

        {/* Rendered markdown */}
        <div className="lg:col-span-6">
          <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface min-h-[400px]">
            {selectedDoc && (
              <>
                <MarkdownRenderer content={selectedDoc.content} />
                <div className="mt-8 pt-3 border-t border-neo-border text-[9px] font-mono text-neo-text-muted">
                  Source: {selectedDoc.path}
                </div>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
