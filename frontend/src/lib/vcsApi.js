// Thin wrapper over the real lifeos-vcs HTTP surface (issues #86/#87).
// TimeTravel.jsx is the only caller; kept separate from lib/vcs.js, which
// versions browser-only app settings via localStorage - a different concern
// from committed file content living in lifeos-vcs's CAS.

import { API_BASE, apiCall, authHeaders } from './api';

// Content-addressed blobs are immutable, so a fetched blob never goes
// stale - cache by blob_ref for the session (issue #109's "local content
// cache"). Values are Promises so concurrent callers share one request.
const blobCache = new Map();

// Fetch raw blob bytes by content hash via GET /api/vcs/blob (local CAS
// first, then the workspace's storage backends - issues #108/#109).
// Returns a Uint8Array.
export function fetchBlob(blobRef) {
  if (!blobCache.has(blobRef)) {
    const promise = (async () => {
      const res = await fetch(`${API_BASE}/api/vcs/blob?blob_ref=${encodeURIComponent(blobRef)}`, {
        headers: authHeaders(false),
      });
      if (!res.ok) throw new Error(`blob fetch failed (${res.status})`);
      return new Uint8Array(await res.arrayBuffer());
    })().catch((e) => {
      blobCache.delete(blobRef); // don't cache failures
      throw e;
    });
    blobCache.set(blobRef, promise);
  }
  return blobCache.get(blobRef);
}

export async function listFileEntities() {
  const { ok, data, error } = await apiCall('GET', '/api/entity?module=files&type=file');
  if (!ok) throw new Error(error || 'failed to list files');
  return data || [];
}

export async function commitFile({ entityId, name, contentBase64, message }) {
  const { ok, data, error } = await apiCall('POST', '/api/vcs/commit', {
    entity_id: entityId || undefined,
    name,
    content_base64: contentBase64,
    message,
  });
  if (!ok) throw new Error(error || 'commit failed');
  return data;
}

export async function getHistory(entityId) {
  const { ok, data, error } = await apiCall('GET', `/api/vcs/history?entity_id=${encodeURIComponent(entityId)}`);
  if (!ok) throw new Error(error || 'history failed');
  return data || [];
}

export async function getDiff({ entityId, oldRef, newRef }) {
  const q = new URLSearchParams({ entity_id: entityId, old: oldRef, ...(newRef ? { new: newRef } : {}) });
  const { ok, data, error } = await apiCall('GET', `/api/vcs/diff?${q.toString()}`);
  if (!ok) throw new Error(error || 'diff failed');
  return data;
}

export async function listRefs(kind) {
  const { ok, data, error } = await apiCall('GET', `/api/vcs/refs?kind=${kind}`);
  if (!ok) throw new Error(error || 'refs failed');
  return data || [];
}

export async function createBranch(name) {
  const { ok, data, error } = await apiCall('POST', '/api/vcs/branch', { name });
  if (!ok) throw new Error(error || 'branch failed');
  return data;
}

export async function createTag(name) {
  const { ok, data, error } = await apiCall('POST', '/api/vcs/tag', { name });
  if (!ok) throw new Error(error || 'tag failed');
  return data;
}

export async function readSnapshot(snapshotRef) {
  const { ok, data, error } = await apiCall('GET', `/api/vcs/snapshot?snapshot_ref=${encodeURIComponent(snapshotRef)}`);
  if (!ok) throw new Error(error || 'snapshot failed');
  return data;
}

// UTF-8 safe base64 encode for small text commits made from the browser.
export function textToBase64(text) {
  const bytes = new TextEncoder().encode(text);
  let binary = '';
  bytes.forEach((b) => { binary += String.fromCharCode(b); });
  return btoa(binary);
}
