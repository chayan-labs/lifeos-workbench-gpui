import React, { useState } from 'react';
import {
  FolderGit2, Folder, FolderOpen, File, FileCode, FileText, FileImage,
  HardDrive, Cloud, Server, Database, Check, ChevronRight, ChevronDown, GitBranch, Lock, Hammer
} from 'lucide-react';
import { getRepoProjects } from '../lib/atlasStore';

/*
 * GitHub-style repository browser (frontend showcase).
 * Files are visible & navigable inside the platform, but the actual bytes live
 * in the storage backend the user chooses. The tree is metadata only; the
 * "stored in" badge shows where each blob physically resides.
 */

const STORE_KEY = 'life_os_storage_backend';

const BACKENDS = [
  { id: 'local', name: 'Local machine', icon: HardDrive, hint: '~/LifeOS/vault', detail: 'Bytes never leave your Mac.' },
  { id: 'gdrive', name: 'Google Drive', icon: Cloud, hint: 'drive.google.com', detail: 'OAuth via owned app; agent holds only a connectionId.' },
  { id: 's3', name: 'S3 / R2', icon: Server, hint: 's3://lifeos-vault', detail: 'S3-compatible object store (AWS S3 or Cloudflare R2).' },
  { id: 'turso', name: 'Turso / libSQL', icon: Database, hint: 'lifeos.db blobs', detail: 'Stored in the canonical DB (project default).' },
];

const iconFor = (name) => {
  if (/\.(png|jpg|jpeg|svg|gif|webp)$/i.test(name)) return FileImage;
  if (/\.(md|txt)$/i.test(name)) return FileText;
  if (/\.(js|jsx|ts|tsx|rs|py|json|toml|css)$/i.test(name)) return FileCode;
  return File;
};

// Mock repo. `store` overrides the default backend for a specific file/folder.
const TREE = [
  { type: 'dir', name: 'modules', children: [
    { type: 'dir', name: 'learning', children: [
      { type: 'file', name: 'module.js', size: '4.2 KB' },
      { type: 'file', name: 'atlas_data.json', size: '1.1 MB', store: 'turso' },
    ] },
    { type: 'file', name: 'tasks.module.js', size: '3.8 KB' },
    { type: 'file', name: 'trading.module.js', size: '5.1 KB' },
  ] },
  { type: 'dir', name: 'media', children: [
    { type: 'file', name: 'demo-reel.mp4', size: '184 MB', store: 's3' },
    { type: 'file', name: 'cover.png', size: '2.4 MB', store: 'gdrive' },
    { type: 'file', name: 'voiceover.wav', size: '38 MB', store: 's3' },
  ] },
  { type: 'dir', name: 'docs', children: [
    { type: 'file', name: 'ARCHITECTURE.md', size: '22 KB' },
    { type: 'file', name: 'DATA-MODEL.md', size: '14 KB' },
  ] },
  { type: 'file', name: 'README.md', size: '8.3 KB' },
  { type: 'file', name: 'lifeos.db', size: '512 MB', store: 'turso' },
];

function StoreBadge({ backendId }) {
  const b = BACKENDS.find((x) => x.id === backendId) || BACKENDS[0];
  return (
    <span className="neo-tag text-[9px] px-1.5 py-0.5 text-neo-text-muted shrink-0" title={`Stored in ${b.name}`}>
      <b.icon size={9} /> {b.name}
    </span>
  );
}

function TreeNode({ node, depth, defaultBackend, onSelect, selected }) {
  const [open, setOpen] = useState(depth === 0);
  const pad = { paddingLeft: `${depth * 16 + 8}px` };

  if (node.type === 'dir') {
    const Caret = open ? ChevronDown : ChevronRight;
    const FolderIcon = open ? FolderOpen : Folder;
    return (
      <div>
        <button
          onClick={() => setOpen(!open)}
          className="w-full flex items-center gap-1.5 py-1.5 px-2 hover:bg-neo-surface-high text-left text-sm text-neo-text"
          style={pad}
        >
          <Caret size={13} className="text-neo-text-muted shrink-0" />
          <FolderIcon size={15} className="text-neo-blue shrink-0" />
          <span className="font-bold truncate">{node.name}</span>
        </button>
        {open && node.children.map((c) => (
          <TreeNode key={c.name} node={c} depth={depth + 1} defaultBackend={defaultBackend} onSelect={onSelect} selected={selected} />
        ))}
      </div>
    );
  }

  const Icon = iconFor(node.name);
  const backendId = node.store || defaultBackend;
  const isSel = selected === node;
  return (
    <button
      onClick={() => onSelect(node)}
      className={`w-full flex items-center gap-1.5 py-1.5 px-2 text-left text-sm transition-colors ${isSel ? 'bg-neo-yellow text-neo-text' : 'hover:bg-neo-surface-high text-neo-text'}`}
      style={pad}
    >
      <span className="w-[13px] shrink-0" />
      <Icon size={15} className="text-neo-text-muted shrink-0" />
      <span className="truncate flex-1">{node.name}</span>
      <span className="text-[10px] text-neo-text-muted font-mono shrink-0">{node.size}</span>
      <StoreBadge backendId={backendId} />
    </button>
  );
}

export default function Repository() {
  const [backend, setBackend] = useState(localStorage.getItem(STORE_KEY) || 'local');
  const [selected, setSelected] = useState(null);
  const acceptedProjects = getRepoProjects();

  const chooseBackend = (id) => {
    setBackend(id);
    localStorage.setItem(STORE_KEY, id);
  };

  const activeBackend = BACKENDS.find((b) => b.id === backend);
  const selBackend = selected ? (selected.store || backend) : null;
  const selBackendObj = BACKENDS.find((b) => b.id === selBackend);

  return (
    <div className="flex flex-col gap-8 max-w-6xl">
      {/* Intro */}
      <div className="neo-surface neo-border-thick neo-shadow p-6">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <FolderGit2 size={24} className="text-neo-blue" /> Repository
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          A Git-style view of every file in your workspace. Files are browsable here, but the actual bytes live in the <strong>storage backend you choose</strong> - your own machine, Google Drive, S3/R2, or the canonical DB. Life OS stores only metadata + version history.
        </p>
      </div>

      {/* Storage picker */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
        <h3 className="neo-title-md flex items-center gap-2"><HardDrive size={18} /> Default Storage Backend</h3>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-3">
          {BACKENDS.map((b) => {
            const active = backend === b.id;
            return (
              <button
                key={b.id}
                onClick={() => chooseBackend(b.id)}
                className={`p-4 neo-border-thick text-left flex flex-col gap-2 transition-all ${
                  active ? 'bg-neo-yellow text-neo-text neo-shadow' : 'bg-neo-surface hover:bg-neo-surface-high'
                }`}
              >
                <div className="flex items-center justify-between">
                  <b.icon size={20} className="text-neo-blue" />
                  {active && <Check size={16} className="text-neo-text" />}
                </div>
                <span className="neo-label-sm text-neo-text">{b.name}</span>
                <code className="text-[10px] font-mono text-neo-text-muted">{b.hint}</code>
                <p className="text-[10px] text-neo-text-muted leading-tight">{b.detail}</p>
              </button>
            );
          })}
        </div>
        <div className="neo-tag bg-neo-mint text-neo-text self-start"><Lock size={11} /> Tokens held in Nango vault - never in agent context</div>
      </div>

      {/* Auto-added projects from Knowledge (accept -> repo loop) */}
      {acceptedProjects.length > 0 && (
        <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
          <h3 className="neo-title-md flex items-center gap-2"><Hammer size={18} className="text-neo-blue" /> Projects auto-added from Knowledge</h3>
          <p className="text-xs text-neo-text-muted">Projects you accepted in a Knowledge domain are scaffolded into the repo here.</p>
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {acceptedProjects.map((p) => (
              <div key={p.id} className="p-3 neo-border bg-neo-surface flex flex-col gap-1">
                <div className="flex items-center gap-1.5">
                  <FolderGit2 size={14} className="text-neo-blue shrink-0" />
                  <span className="font-bold text-sm text-neo-text truncate">{p.name}</span>
                </div>
                <span className="text-[11px] text-neo-text-muted">{p.pitch}</span>
                <div className="flex items-center gap-1.5 mt-1">
                  <span className="neo-tag text-[9px]">{p.domain}</span>
                  {p.difficulty && <span className="neo-tag text-[9px] bg-neo-yellow text-neo-text">{p.difficulty}</span>}
                </div>
                <code className="text-[9px] font-mono text-neo-text-muted mt-1">repo/projects/{p.id}</code>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Repo browser */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        <div className="lg:col-span-7 neo-surface neo-border-thick neo-shadow flex flex-col">
          <div className="flex items-center justify-between px-4 py-3 border-b-2 border-neo-border">
            <span className="neo-label-sm flex items-center gap-2 text-neo-text"><GitBranch size={14} /> main</span>
            <span className="text-[11px] text-neo-text-muted font-mono">default → {activeBackend?.name}</span>
          </div>
          <div className="py-1">
            {TREE.map((n) => (
              <TreeNode key={n.name} node={n} depth={0} defaultBackend={backend} onSelect={setSelected} selected={selected} />
            ))}
          </div>
        </div>

        {/* File detail */}
        <div className="lg:col-span-5 neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
          <h3 className="neo-title-md flex items-center gap-2"><File size={18} /> File Detail</h3>
          {!selected ? (
            <p className="text-sm text-neo-text-muted">Select a file to inspect where its bytes are stored.</p>
          ) : (
            <div className="flex flex-col gap-3">
              <div className="flex items-center gap-2">
                {React.createElement(iconFor(selected.name), { size: 18, className: 'text-neo-blue' })}
                <span className="font-bold text-neo-text break-all">{selected.name}</span>
              </div>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="p-2 neo-border bg-neo-surface-high">
                  <div className="text-neo-text-muted">Size</div>
                  <div className="font-mono text-neo-text">{selected.size}</div>
                </div>
                <div className="p-2 neo-border bg-neo-surface-high">
                  <div className="text-neo-text-muted">Stored in</div>
                  <div className="font-mono text-neo-text flex items-center gap-1">
                    {selBackendObj && <selBackendObj.icon size={12} />} {selBackendObj?.name}
                  </div>
                </div>
              </div>
              <div className="p-3 neo-border bg-neo-surface text-[11px] text-neo-text-muted leading-relaxed">
                The platform holds this file's path, hash and version history. On open, Life OS streams the bytes from <strong>{selBackendObj?.name}</strong> ({selBackendObj?.hint}) via the storage proxy - the blob never passes through libSQL.
              </div>
              <div className="flex gap-2">
                <button className="neo-btn bg-neo-surface-high text-neo-text py-1.5 px-3 text-xs flex-1">View history</button>
                <button className="neo-btn bg-neo-blue text-white py-1.5 px-3 text-xs flex-1">Open</button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
