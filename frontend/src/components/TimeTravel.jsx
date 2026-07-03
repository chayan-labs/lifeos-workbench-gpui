import React, { useState, useEffect, useCallback } from 'react';
import {
  GitCommit, History, RotateCcw, FileClock, Lock, Check, ShieldCheck, ChevronDown, ChevronRight, Anchor,
  Upload, GitBranch, Tag as TagIcon, Camera, AlertTriangle, Plus,
} from 'lucide-react';
import {
  listCommits, commit, restoreSnapshot, restoreFile, dirtyKeys, ensureBaseline, TRACKED_KEYS
} from '../lib/vcs';
import {
  listFileEntities, commitFile, getHistory, getDiff, listRefs, createBranch, createTag, readSnapshot, textToBase64,
} from '../lib/vcsApi';
import BlobView from './BlobView';

// VCS + time-travel surface, two independent layers:
// - Files (lifeos-vcs, issue #86/#87): real committed file content, CAS +
//   append-only version.created events on the server. AI never commits here
//   either - every write below is a direct, human-initiated form submit.
// - App settings checkpoints (lib/vcs.js): browser-only localStorage
//   snapshots of app preferences. Kept separate - different data, different
//   storage, same "AI is gated from history" rule.

const fileLabel = (k) =>
  k.replace(/^life_os_/, '').replace(/^KA_/, 'atlas:').replace(/_V1$/, '').replace(/_/g, ' ');

const shortRef = (r) => (r ? `${r.slice(0, 10)}…` : '');

function FileVersioning() {
  const [files, setFiles] = useState([]);
  const [selected, setSelected] = useState(null);
  const [history, setHistory] = useState([]);
  const [name, setName] = useState('');
  const [content, setContent] = useState('');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');
  const [flash, setFlash] = useState('');
  const [diffPair, setDiffPair] = useState(null); // { old, new }
  const [diff, setDiff] = useState(null);
  const [viewing, setViewing] = useState(null); // a VersionEntry to render inline

  const refreshFiles = useCallback(async () => {
    try {
      setFiles(await listFileEntities());
    } catch (e) {
      setError(e.message);
    }
  }, []);

  useEffect(() => { refreshFiles(); }, [refreshFiles]);

  const selectFile = async (entity) => {
    setSelected(entity);
    setDiff(null);
    setDiffPair(null);
    setViewing(null);
    try {
      const h = await getHistory(entity.id);
      setHistory(h); // oldest first
      if (h.length >= 2) {
        const newest = h[h.length - 1];
        const prev = h[h.length - 2];
        loadDiff(entity.id, prev.blob_ref, newest.blob_ref);
      }
    } catch (e) {
      setError(e.message);
    }
  };

  const loadDiff = async (entityId, oldRef, newRef) => {
    setDiffPair({ old: oldRef, new: newRef });
    setDiff(null);
    try {
      setDiff(await getDiff({ entityId, oldRef, newRef }));
    } catch (e) {
      setError(e.message);
    }
  };

  const doCommit = async (e) => {
    e.preventDefault();
    if (!name.trim()) { setError('filename is required'); return; }
    setError('');
    try {
      const entity = await commitFile({
        entityId: selected?.attrs?.name === name.trim() ? selected.id : undefined,
        name: name.trim(),
        contentBase64: textToBase64(content),
        message: message || 'Update',
      });
      setFlash(`Committed ${entity.attrs?.name || name}`);
      setMessage('');
      await refreshFiles();
      await selectFile(entity);
      setTimeout(() => setFlash(''), 1500);
    } catch (e2) {
      setError(e2.message);
    }
  };

  return (
    <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h3 className="neo-title-md flex items-center gap-2"><FileClock size={18} /> Files (lifeos-vcs)</h3>
        {flash && <span className="neo-tag bg-neo-mint text-neo-text"><Check size={11} /> {flash}</span>}
      </div>
      <p className="text-xs text-neo-text-muted">
        Real content-addressed version history via <code>lifeos-vcs</code> - every commit here persists bytes through the CAS store and survives reload.
      </p>
      {error && (
        <div className="neo-tag bg-neo-red text-white self-start"><AlertTriangle size={11} /> {error}</div>
      )}

      <form onSubmit={doCommit} className="flex flex-col gap-2 p-3 neo-border bg-neo-surface-high">
        <div className="flex gap-2">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="filename, e.g. notes.txt"
            className="neo-input text-sm flex-1"
          />
          <input
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            placeholder="commit message"
            className="neo-input text-sm flex-1"
          />
        </div>
        <textarea
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="file content (text)"
          rows={3}
          className="neo-input text-xs font-mono"
        />
        <button type="submit" className="neo-btn bg-neo-mint text-neo-text py-2 px-4 text-xs flex items-center gap-2 self-start">
          <Upload size={14} /> Commit version
        </button>
      </form>

      <div className="grid grid-cols-1 lg:grid-cols-12 gap-4">
        <div className="lg:col-span-4 flex flex-col gap-1">
          <span className="neo-label-sm text-neo-text-muted">Committed files ({files.length})</span>
          {files.map((f) => (
            <button
              key={f.id}
              onClick={() => selectFile(f)}
              className={`text-left text-xs font-mono p-2 neo-border ${selected?.id === f.id ? 'bg-neo-yellow text-neo-text' : 'bg-neo-surface hover:bg-neo-surface-high'}`}
            >
              {f.attrs?.name || f.title || f.id}
            </button>
          ))}
          {files.length === 0 && <span className="text-[11px] text-neo-text-muted">No files committed yet.</span>}
        </div>

        <div className="lg:col-span-8 flex flex-col gap-3">
          {!selected ? (
            <p className="text-sm text-neo-text-muted">Select a file to see its real version timeline.</p>
          ) : (
            <>
              <span className="neo-label-sm text-neo-text-muted">
                Timeline for {selected.attrs?.name || selected.title} ({history.length})
              </span>
              <div className="flex flex-col gap-1">
                {[...history].reverse().map((v, idx, reversed) => {
                  const older = reversed[idx + 1]; // next in newest-first order = chronologically previous
                  return (
                    <div key={v.blob_ref + v.ts} className="flex items-center justify-between text-[11px] p-2 neo-border bg-neo-surface">
                      <div className="flex flex-col min-w-0">
                        <span className="font-bold text-neo-text truncate">{v.message || '(no message)'}</span>
                        <span className="font-mono text-neo-text-muted">{shortRef(v.blob_ref)} · {v.author} · {new Date(v.ts * 1000).toLocaleString()}</span>
                      </div>
                      <div className="flex items-center gap-1 shrink-0">
                        <button
                          onClick={() => setViewing(viewing?.blob_ref === v.blob_ref ? null : v)}
                          className={`neo-btn py-1 px-2 text-[10px] ${viewing?.blob_ref === v.blob_ref ? 'bg-neo-yellow text-neo-text' : 'bg-neo-surface-high text-neo-text'}`}
                        >
                          {viewing?.blob_ref === v.blob_ref ? 'hide' : 'view'}
                        </button>
                        {older && (
                          <button
                            onClick={() => loadDiff(selected.id, older.blob_ref, v.blob_ref)}
                            className="neo-btn bg-neo-surface-high text-neo-text py-1 px-2 text-[10px]"
                          >
                            diff vs previous
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>

              {viewing && (
                <div className="flex flex-col gap-2 p-3 neo-border bg-neo-surface-high">
                  <span className="neo-label-sm text-neo-text-muted">
                    Content at {shortRef(viewing.blob_ref)} - fetched by hash from whichever backend holds it
                  </span>
                  <BlobView
                    blobRef={viewing.blob_ref}
                    name={selected.attrs?.name || selected.title}
                    mime={selected.attrs?.mime}
                    size={selected.attrs?.size}
                    version={viewing.message}
                  />
                </div>
              )}

              {diffPair && (
                <div className="flex flex-col gap-2 p-3 neo-border bg-neo-surface-high">
                  <span className="neo-label-sm text-neo-text-muted">
                    Diff {shortRef(diffPair.old)} → {shortRef(diffPair.new)}
                  </span>
                  {!diff ? (
                    <span className="text-[11px] text-neo-text-muted">Loading…</span>
                  ) : diff.supported ? (
                    <>
                      <span className="text-xs font-bold text-neo-text">{diff.summary}</span>
                      <div className="font-mono text-[11px] leading-relaxed overflow-x-auto">
                        {diff.lines.map((l, i) => (
                          <div
                            key={i}
                            className={
                              l.tag === 'insert' ? 'bg-neo-mint/30 text-neo-text' :
                              l.tag === 'delete' ? 'bg-neo-red/20 text-neo-text line-through decoration-1' :
                              'text-neo-text-muted'
                            }
                          >
                            {l.tag === 'insert' ? '+ ' : l.tag === 'delete' ? '- ' : '  '}{l.text}
                          </div>
                        ))}
                      </div>
                    </>
                  ) : (
                    <div className="neo-tag bg-neo-yellow text-neo-text self-start">
                      <AlertTriangle size={11} /> No diff pipeline for "{diff.kind}" yet - blocked by {diff.blocked_by}
                    </div>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function Snapshots() {
  const [branches, setBranches] = useState([]);
  const [tags, setTags] = useState([]);
  const [branchName, setBranchName] = useState('');
  const [tagName, setTagName] = useState('');
  const [error, setError] = useState('');
  const [manifest, setManifest] = useState(null);
  const [manifestRef, setManifestRef] = useState('');

  const refresh = useCallback(async () => {
    try {
      const [b, t] = await Promise.all([listRefs('branch'), listRefs('tag')]);
      setBranches(b);
      setTags(t);
    } catch (e) {
      setError(e.message);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const doCreateBranch = async (e) => {
    e.preventDefault();
    if (!branchName.trim()) return;
    setError('');
    try {
      await createBranch(branchName.trim());
      setBranchName('');
      await refresh();
    } catch (e2) { setError(e2.message); }
  };

  const doCreateTag = async (e) => {
    e.preventDefault();
    if (!tagName.trim()) return;
    setError('');
    try {
      await createTag(tagName.trim());
      setTagName('');
      await refresh();
    } catch (e2) { setError(e2.message); }
  };

  const inspect = async (snapshotRef) => {
    setManifestRef(snapshotRef);
    setManifest(null);
    try {
      setManifest(await readSnapshot(snapshotRef));
    } catch (e) { setError(e.message); }
  };

  return (
    <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-4">
      <h3 className="neo-title-md flex items-center gap-2"><Camera size={18} /> Snapshots, Branches &amp; Tags</h3>
      <p className="text-xs text-neo-text-muted">
        "Show me everything as it was" - a branch/tag captures every committed file's current version into one snapshot. Branches move forward; tags refuse to move once set. There is no route anywhere to force a branch backward or rewrite a tag.
      </p>
      {error && <div className="neo-tag bg-neo-red text-white self-start"><AlertTriangle size={11} /> {error}</div>}

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="flex flex-col gap-2">
          <span className="neo-label-sm text-neo-text-muted flex items-center gap-1"><GitBranch size={12} /> Branches</span>
          <form onSubmit={doCreateBranch} className="flex gap-2">
            <input value={branchName} onChange={(e) => setBranchName(e.target.value)} placeholder="branch name" className="neo-input text-xs flex-1" />
            <button type="submit" className="neo-btn bg-neo-blue text-white px-2 text-xs flex items-center gap-1"><Plus size={12} /> Create</button>
          </form>
          {branches.map((b) => (
            <button key={b.name} onClick={() => inspect(b.snapshot_ref)} className="text-left text-[11px] font-mono p-2 neo-border bg-neo-surface hover:bg-neo-surface-high flex justify-between">
              <span>{b.name}</span>
              <span className="text-neo-text-muted">{shortRef(b.snapshot_ref)}</span>
            </button>
          ))}
          {branches.length === 0 && <span className="text-[11px] text-neo-text-muted">No branches yet.</span>}
        </div>

        <div className="flex flex-col gap-2">
          <span className="neo-label-sm text-neo-text-muted flex items-center gap-1"><TagIcon size={12} /> Tags</span>
          <form onSubmit={doCreateTag} className="flex gap-2">
            <input value={tagName} onChange={(e) => setTagName(e.target.value)} placeholder="tag name" className="neo-input text-xs flex-1" />
            <button type="submit" className="neo-btn bg-neo-blue text-white px-2 text-xs flex items-center gap-1"><Plus size={12} /> Create</button>
          </form>
          {tags.map((t) => (
            <button key={t.name} onClick={() => inspect(t.snapshot_ref)} className="text-left text-[11px] font-mono p-2 neo-border bg-neo-surface hover:bg-neo-surface-high flex justify-between">
              <span>{t.name}</span>
              <span className="text-neo-text-muted">{shortRef(t.snapshot_ref)}</span>
            </button>
          ))}
          {tags.length === 0 && <span className="text-[11px] text-neo-text-muted">No tags yet.</span>}
        </div>
      </div>

      {manifestRef && (
        <div className="flex flex-col gap-2 p-3 neo-border bg-neo-surface-high">
          <span className="neo-label-sm text-neo-text-muted">Snapshot {shortRef(manifestRef)}</span>
          {!manifest ? (
            <span className="text-[11px] text-neo-text-muted">Loading…</span>
          ) : Object.keys(manifest.entries).length === 0 ? (
            <span className="text-[11px] text-neo-text-muted">empty</span>
          ) : (
            <div className="flex flex-col gap-1 font-mono text-[11px]">
              {Object.entries(manifest.entries).map(([entityId, blobRef]) => (
                <div key={entityId} className="flex justify-between">
                  <span className="text-neo-text truncate">{entityId}</span>
                  <span className="text-neo-text-muted">{shortRef(blobRef)}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function TimeTravel() {
  const [commits, setCommits] = useState([]);
  const [dirty, setDirty] = useState([]);
  const [message, setMessage] = useState('');
  const [expanded, setExpanded] = useState(null);
  const [flash, setFlash] = useState('');

  const refresh = () => {
    ensureBaseline();
    setCommits([...listCommits()].reverse()); // newest first
    setDirty(dirtyKeys());
  };

  useEffect(() => { refresh(); }, []);

  const doCommit = () => {
    commit(message || 'Manual checkpoint', 'user');
    setMessage('');
    setFlash('Committed');
    refresh();
    setTimeout(() => setFlash(''), 1500);
  };

  const doRestore = (id) => {
    if (!window.confirm('Jump the whole app back to this point? A new restore commit will be appended (history is never erased).')) return;
    restoreSnapshot(id);
    setFlash('Restored snapshot');
    refresh();
    setTimeout(() => setFlash(''), 1500);
  };

  const doRestoreFile = (id, key) => {
    restoreFile(id, key);
    setFlash(`Restored ${fileLabel(key)}`);
    refresh();
    setTimeout(() => setFlash(''), 1500);
  };

  return (
    <div className="flex flex-col gap-6">
      {/* AI-gated banner */}
      <div className="neo-surface neo-border-thick neo-shadow p-4 flex items-start gap-3 bg-neo-surface">
        <ShieldCheck size={22} className="text-neo-mint shrink-0 mt-0.5" />
        <div>
          <h3 className="neo-label-md text-neo-text flex items-center gap-2">Version Control <span className="neo-tag bg-neo-red text-white"><Lock size={10} /> AI-GATED</span></h3>
          <p className="text-xs text-neo-text-muted mt-1">
            Every change you or the AI make can be committed here. History is append-only - restoring a past point appends a new commit, so you can always move forward again. <strong>AI can never commit, rewrite, or delete history.</strong> Only you drive time-travel.
          </p>
        </div>
      </div>

      <FileVersioning />
      <Snapshots />

      {/* App settings checkpoints (local, browser-only) */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <h3 className="neo-title-md flex items-center gap-2"><GitCommit size={18} /> App settings checkpoints (local)</h3>
          {flash && <span className="neo-tag bg-neo-mint text-neo-text"><Check size={11} /> {flash}</span>}
        </div>
        <p className="text-xs text-neo-text-muted">
          Browser-only snapshots of app preferences (theme, plan, sidebar state, …) - separate from real file content above, which lives server-side in lifeos-vcs.
        </p>
        <p className="text-xs text-neo-text-muted">
          {dirty.length === 0
            ? 'Working tree clean - nothing changed since the last commit.'
            : `${dirty.length} change${dirty.length > 1 ? 's' : ''} since last commit: ${dirty.map(fileLabel).join(', ')}`}
        </p>
        <div className="flex gap-2">
          <input
            value={message}
            onChange={(e) => setMessage(e.target.value)}
            placeholder="Commit message (e.g. 'added Spanish domain')"
            className="neo-input text-sm flex-1"
          />
          <button onClick={doCommit} className="neo-btn bg-neo-mint text-neo-text py-2 px-4 text-xs flex items-center gap-2">
            <GitCommit size={14} /> Commit
          </button>
        </div>
      </div>

      {/* Timeline */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 flex flex-col gap-3">
        <h3 className="neo-title-md flex items-center gap-2"><History size={18} /> Timeline ({commits.length})</h3>
        <div className="flex flex-col gap-2">
          {commits.map((c) => {
            const isOpen = expanded === c.id;
            const keys = Object.keys(c.snapshot || {});
            return (
              <div key={c.id} className={`neo-border ${c.baseline ? 'bg-neo-yellow/15 border-neo-blue' : 'bg-neo-surface'}`}>
                <div className="flex items-center gap-2 p-3">
                  <button onClick={() => setExpanded(isOpen ? null : c.id)} className="text-neo-text-muted shrink-0">
                    {isOpen ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                  </button>
                  {c.baseline ? <Anchor size={14} className="text-neo-blue shrink-0" /> : <GitCommit size={14} className="text-neo-text-muted shrink-0" />}
                  <div className="flex flex-col min-w-0 flex-1">
                    <span className="text-sm font-bold text-neo-text truncate">{c.message}</span>
                    <span className="text-[10px] font-mono text-neo-text-muted">
                      {c.hash} · {c.author}{c.baseline ? ' · protected baseline' : ''} · {new Date(c.createdAt).toLocaleString()}
                    </span>
                  </div>
                  <button
                    onClick={() => doRestore(c.id)}
                    className="neo-btn bg-neo-surface-high text-neo-text py-1 px-2 text-[10px] flex items-center gap-1 shrink-0"
                    title="Jump the whole app to this point"
                  >
                    <RotateCcw size={11} /> Jump here
                  </button>
                </div>
                {isOpen && (
                  <div className="px-3 pb-3 pt-0 border-t border-neo-border flex flex-col gap-1">
                    <span className="neo-label-sm text-neo-text-muted text-[10px] mt-2 flex items-center gap-1"><FileClock size={11} /> Files in this commit</span>
                    {keys.length === 0 && <span className="text-[11px] text-neo-text-muted">empty</span>}
                    {keys.map((k) => (
                      <div key={k} className="flex items-center justify-between text-[11px] py-0.5">
                        <span className="font-mono text-neo-text truncate">{fileLabel(k)}</span>
                        <button onClick={() => doRestoreFile(c.id, k)} className="text-neo-blue hover:underline shrink-0 ml-2">restore this file</button>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
