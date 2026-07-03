import React, { useEffect, useState } from 'react';
import { Store, UploadCloud, DownloadCloud, ShieldCheck, RefreshCw } from 'lucide-react';
import { apiCall } from '../lib/api';

// Module marketplace browse/publish/install (issues #101/#102,
// docs/PLATFORM-SYSTEMS.md). Publish/install call the real
// /api/marketplace/* routes; the render/structural validators and the
// git-commit-as-install step live in the Node scaffold layer
// (server/scaffold.js), not here - this UI covers the marketplace half.
export default function Marketplace() {
  const [packages, setPackages] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [moduleId, setModuleId] = useState('');
  const [version, setVersion] = useState('1.0.0');
  const [manifestText, setManifestText] = useState('{\n  "id": "",\n  "version": "1.0.0"\n}');
  const [busyId, setBusyId] = useState(null);

  const load = async () => {
    setLoading(true);
    const { ok, data, error: err } = await apiCall('GET', '/api/marketplace/packages');
    if (ok) setPackages(data.packages || []);
    else setError(err || 'Failed to load marketplace packages.');
    setLoading(false);
  };

  useEffect(() => { load(); }, []);

  const handlePublish = async (e) => {
    e.preventDefault();
    setError('');
    let manifest;
    try {
      manifest = JSON.parse(manifestText);
    } catch {
      setError('Manifest must be valid JSON.');
      return;
    }
    const { ok, error: err } = await apiCall('POST', '/api/marketplace/publish', {
      module_id: moduleId,
      version,
      manifest,
    });
    if (!ok) {
      setError(err || 'Publish failed.');
      return;
    }
    setModuleId('');
    load();
  };

  const handleInstall = async (packageId) => {
    setBusyId(packageId);
    const { ok, error: err } = await apiCall('POST', '/api/marketplace/install', { package_id: packageId });
    if (!ok) setError(err || 'Install failed - signature verification did not pass.');
    setBusyId(null);
  };

  return (
    <div className="p-6 flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <Store size={22} />
          <h1 className="neo-title-lg">Module Marketplace</h1>
        </div>
        <button onClick={load} className="neo-btn px-3 py-2 flex items-center gap-1.5 text-xs font-bold uppercase">
          <RefreshCw size={14} /> Refresh
        </button>
      </div>

      {error && (
        <div className="p-3 bg-neo-red text-white border-2 border-neo-border neo-label-sm">{error}</div>
      )}

      <form onSubmit={handlePublish} className="neo-surface neo-border neo-shadow-sm p-4 flex flex-col gap-3">
        <div className="flex items-center gap-2 neo-label-md">
          <UploadCloud size={16} /> Publish a package
        </div>
        <div className="flex gap-3">
          <input
            className="p-2 neo-border bg-neo-bg text-xs font-mono flex-1"
            placeholder="module_id"
            value={moduleId}
            onChange={(e) => setModuleId(e.target.value)}
            required
          />
          <input
            className="p-2 neo-border bg-neo-bg text-xs font-mono w-32"
            placeholder="version"
            value={version}
            onChange={(e) => setVersion(e.target.value)}
            required
          />
        </div>
        <textarea
          className="p-2 neo-border bg-neo-bg text-xs font-mono h-28"
          value={manifestText}
          onChange={(e) => setManifestText(e.target.value)}
        />
        <button type="submit" className="neo-btn self-start px-4 py-2 bg-neo-mint text-black text-xs font-bold uppercase">
          Sign &amp; Publish
        </button>
      </form>

      <div className="flex flex-col gap-3">
        {loading && <span className="neo-label-sm text-neo-text-muted">Loading...</span>}
        {!loading && packages.length === 0 && (
          <span className="neo-label-sm text-neo-text-muted">No packages published yet.</span>
        )}
        {packages.map((pkg) => (
          <div key={pkg.id} className="neo-surface neo-border neo-shadow-sm p-4 flex items-center justify-between">
            <div className="flex flex-col gap-1">
              <span className="font-bold text-sm">{pkg.module_id}@{pkg.version}</span>
              <span className="neo-label-sm text-neo-text-muted flex items-center gap-1">
                <ShieldCheck size={12} /> {pkg.publisher_pubkey.slice(0, 16)}...
              </span>
            </div>
            <button
              onClick={() => handleInstall(pkg.id)}
              disabled={busyId === pkg.id}
              className="neo-btn px-3 py-2 flex items-center gap-1.5 text-xs font-bold uppercase bg-neo-yellow text-black"
            >
              <DownloadCloud size={14} /> {busyId === pkg.id ? 'Installing...' : 'Install'}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
