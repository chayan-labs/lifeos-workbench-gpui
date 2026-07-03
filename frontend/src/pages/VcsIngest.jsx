import React, { useState, useEffect, useCallback } from 'react';
import { FileCode, Play, FileAudio, Search, GitCommit, ArrowRight, Eye, Diff, UploadCloud, RefreshCw } from 'lucide-react';
import { apiCall } from '../lib/api';
import { listFileEntities } from '../lib/vcsApi';

// Same poll-only-while-active pattern as pages/Database.jsx's JOBS_POLL_MS.
const INGEST_POLL_MS = 3000;

// Real ingest status panel (issue #91): file.imported/version.created now
// auto-enqueue an ingest job (services/lifeos-api routes/files.rs,
// routes/drive.rs); this panel is the manual trigger + status/segment-count/
// re-index UI docs/MEDIA-INTELLIGENCE.md §4 and frontend/FRONTEND.md §2 call
// for, backed by real POST /api/ingest + GET /api/entity, not mock data.
function IngestStatusPanel() {
  const [files, setFiles] = useState([]);
  const [selectedId, setSelectedId] = useState('');
  const [statusById, setStatusById] = useState({});
  const [busyId, setBusyId] = useState(null);
  const [error, setError] = useState('');

  const loadFiles = useCallback(() => {
    listFileEntities()
      .then((data) => {
        setFiles(data);
        if (!selectedId && data.length) setSelectedId(data[0].id);
      })
      .catch((e) => setError(e.message));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => { loadFiles(); }, [loadFiles]);

  const refreshStatus = useCallback(async (entityId) => {
    const [entityRes, segmentsRes] = await Promise.all([
      apiCall('GET', `/api/entity/${encodeURIComponent(entityId)}`),
      apiCall('GET', `/api/entity?type=segment&parent_id=${encodeURIComponent(entityId)}`),
    ]);
    const attrs = entityRes.ok ? entityRes.data?.attrs || {} : {};
    const segmentCount = segmentsRes.ok && Array.isArray(segmentsRes.data) ? segmentsRes.data.length : 0;
    setStatusById((prev) => ({
      ...prev,
      [entityId]: {
        ingestStatus: attrs.ingest_status || (segmentCount > 0 || attrs.transcript_ref ? 'completed' : 'not_ingested'),
        blockedBy: attrs.ingest_blocked_by || null,
        segmentCount,
        hasTranscript: Boolean(attrs.transcript_ref),
      },
    }));
  }, []);

  useEffect(() => {
    if (!selectedId) return undefined;
    refreshStatus(selectedId);
    const status = statusById[selectedId];
    if (busyId !== selectedId && status?.ingestStatus !== 'not_ingested') return undefined;
    const interval = setInterval(() => refreshStatus(selectedId), INGEST_POLL_MS);
    return () => clearInterval(interval);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, busyId]);

  const triggerIngest = async (entityId) => {
    setBusyId(entityId);
    setError('');
    const { ok, error: err } = await apiCall('POST', '/api/ingest', { entity_id: entityId });
    if (!ok) setError(err || 'ingest enqueue failed');
    await refreshStatus(entityId);
    setBusyId(null);
  };

  const current = selectedId ? statusById[selectedId] : null;

  return (
    <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
      <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4 flex items-center gap-2">
        <UploadCloud size={18} />
        Ingest Status
      </h3>

      {error && <div className="text-xs text-neo-red mb-3">{error}</div>}

      <div className="flex gap-2 mb-4">
        <select
          className="neo-input flex-1 text-xs"
          value={selectedId}
          onChange={(e) => setSelectedId(e.target.value)}
        >
          {files.length === 0 && <option value="">No file entities yet - commit one first</option>}
          {files.map((f) => (
            <option key={f.id} value={f.id}>{f.attrs?.name || f.id}</option>
          ))}
        </select>
        <button
          onClick={() => selectedId && triggerIngest(selectedId)}
          disabled={!selectedId || busyId === selectedId}
          className="neo-btn py-1.5 px-3 bg-neo-yellow text-[10px] font-bold flex items-center gap-1 disabled:opacity-50"
        >
          <RefreshCw size={12} className={busyId === selectedId ? 'animate-spin' : ''} />
          {current?.ingestStatus === 'not_ingested' ? 'Ingest' : 'Re-index'}
        </button>
      </div>

      {current && (
        <div className="p-3 bg-neo-bg neo-border text-xs flex flex-col gap-1.5">
          <div className="flex justify-between items-center">
            <span className="font-bold">Status</span>
            <span className={`neo-chip py-0.5 text-[9px] ${current.ingestStatus === 'unsupported' ? 'neo-chip--review' : 'neo-chip--completed'}`}>
              {(busyId === selectedId ? 'queued' : current.ingestStatus).toUpperCase()}
            </span>
          </div>
          <div className="flex justify-between items-center">
            <span className="text-neo-text-muted">Segments</span>
            <span className="font-mono">{current.segmentCount}</span>
          </div>
          {current.blockedBy && (
            <div className="pt-1.5 border-t border-neo-border border-dashed text-[10px] italic text-neo-text-muted">
              Blocked: {current.blockedBy}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function VcsIngest() {
  const [transcriptionQuery, setTranscriptionQuery] = useState('Nango credentials proxy');
  const allClips = [
    { file: 'session_clip_382.mp3', text: '...the credentials actually reside inside self-hosted Nango proxy, meaning the agent context does not leak access tokens...', timestamp: '04:12 - 04:30', confidence: '98%' },
    { file: 'meeting_notes_june.mp3', text: '...we should configure the proxy callback URL on the Cloudflare Worker to point to Nango...', timestamp: '12:05 - 12:20', confidence: '91%' },
    { file: 'trading_playbook_audio.wav', text: '...double bottom bounds are checked on chart screenshots using perceptual hash difference functions...', timestamp: '01:45 - 02:10', confidence: '89%' },
    { file: 'vcs_overview.mp3', text: '...content-defined chunking or CDC uses blake3 hashes to deduplicate versions of large media files like video and assets...', timestamp: '08:50 - 09:30', confidence: '95%' }
  ];

  // Interactive query-based filtering
  const foundClips = allClips.filter(c => 
    c.text.toLowerCase().includes(transcriptionQuery.toLowerCase()) ||
    c.file.toLowerCase().includes(transcriptionQuery.toLowerCase())
  );

  const [selectedDiffFile, setSelectedDiffFile] = useState('image');
  const [diffResult, setDiffResult] = useState({
    image: {
      filename: 'AAPL_Daily_Chart.png',
      change: 'Perceptual Hash Diff: 14.8% delta',
      details: 'Visual bounds adjusted on RSI index line. Pixels shifted by +12px on Y axis.',
      visual: 'Side-by-side overlay representation: Blue/red outline overlay on RSI channel.'
    },
    godot: {
      filename: 'combat_scene.tscn',
      change: 'Text Diff: 3 lines added, 2 removed',
      details: 'Modified node properties under [node name="Player" type="CharacterBody2D"]:\n- speed = 400.0\n+ speed = 480.0',
      visual: 'Plain-text config serialization matching standard git diff'
    },
    figma: {
      filename: 'Dashboard Design System',
      change: 'Node Tree Diff: 2 nodes added, 1 modified',
      details: 'Added FrameNode "GlobeLogoContainer"\nAdded VectorNode "LatitudeOrbitLine"\nModified TextStyleNode font-weight from Bold to Heavy',
      visual: 'JSON structure comparison of Figma document tree via mcp-figma'
    }
  });

  const versionedFiles = [
    { name: 'logo_animation_hero.mp4', type: 'VIDEO', size: '14.2 MB', versions: 3, lastCommit: 'Refine spin speed offset' },
    { name: 'dashboard_mockup.fig', type: 'DESIGN', size: '4.8 MB', versions: 5, lastCommit: 'Brutalist box outline fix' },
    { name: 'audio_dictation_notes.wav', type: 'AUDIO', size: '32.1 MB', versions: 2, lastCommit: 'Transcript generated' },
  ];

  // Interactive slider for FastCDC deduplication simulator
  const [chunkSizeKb, setChunkSizeKb] = useState(64);
  const totalBaseSizeMb = 120.4;
  // Compute mock deduplication ratio based on chunk size
  const dedupRatio = Math.max(12, Math.min(84, Math.round(92 - (chunkSizeKb / 4)))).toFixed(1);
  const finalSizeMb = (totalBaseSizeMb * (1 - (parseFloat(dedupRatio) / 100))).toFixed(1);

  return (
    <div className="flex flex-col gap-8">
      {/* Overview */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <FileCode size={24} className="text-neo-blue" />
          `lifeos-vcs` & Media Intelligence
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          Life OS extends version control to all files (images, designs, videos, audio) using content-addressed BLAKE3 + FastCDC chunking. At the same time, the Rust-based <strong>media intelligence pipeline</strong> parses audio files using whisper-rs, mapping voice queries to timestamped database segments.
        </p>
      </div>

      {/* Ingest Status - real, backed by POST /api/ingest + GET /api/entity (issue #91) */}
      <IngestStatusPanel />

      {/* Main Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">

        {/* Version Control list */}
        <div className="lg:col-span-6 neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
          <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4 flex items-center gap-2">
            <GitCommit size={18} />
            Universal Version History
          </h3>
          <div className="flex flex-col gap-4">
            {versionedFiles.map((file, idx) => (
              <div key={idx} className="p-4 bg-neo-bg neo-border flex flex-col gap-2 relative">
                <div className="flex justify-between items-start">
                  <div>
                    <span className="neo-label-md block font-bold text-neo-blue">{file.name}</span>
                    <span className="text-[10px] text-neo-text-muted font-mono">{file.size} • {file.type}</span>
                  </div>
                  <span className="neo-chip neo-chip--completed py-0.5 text-[9px]">{file.versions} VERSIONS</span>
                </div>
                
                <div className="pt-2 border-t border-neo-border border-dashed text-xs flex justify-between items-center">
                  <span className="text-[10px] italic text-neo-text-muted">Latest: "{file.lastCommit}"</span>
                  <button onClick={() => setSelectedDiffFile(file.type === 'VIDEO' ? 'godot' : file.type === 'DESIGN' ? 'figma' : 'image')} className="neo-btn py-1 px-2.5 bg-neo-surface text-[10px] font-bold">
                    Diff History
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Media Intelligence Clip Finder */}
        <div className="lg:col-span-6 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface flex-1">
            <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4 flex items-center gap-2">
              <FileAudio size={18} />
              Semantic Voice Search
            </h3>
            
            <div className="flex gap-2 mb-6">
              <div className="relative flex-1">
                <Search size={16} className="absolute left-3 top-3 text-neo-text-muted" />
                <input
                  type="text"
                  value={transcriptionQuery}
                  onChange={(e) => setTranscriptionQuery(e.target.value)}
                  placeholder="Type e.g. Nango, proxy, chart, blake3..."
                  className="neo-input w-full pl-10"
                />
              </div>
            </div>

            <div className="flex flex-col gap-3 max-h-[300px] overflow-y-auto">
              {foundClips.length > 0 ? foundClips.map((clip, idx) => (
                <div key={idx} className="p-3 bg-neo-surface neo-border text-xs flex flex-col gap-2 relative">
                  <div className="flex justify-between items-center border-b border-neo-border pb-1.5">
                    <span className="font-bold flex items-center gap-1">
                      <FileAudio size={12} className="text-neo-blue" />
                      {clip.file}
                    </span>
                    <span className="text-[9px] neo-chip neo-chip--completed py-0.5">{clip.confidence} match</span>
                  </div>
                  <p className="italic text-neo-text-muted text-[11px]">"{clip.text}"</p>
                  <div className="flex justify-between items-center pt-2">
                    <span className="text-[10px] font-mono bg-neo-bg px-1.5 py-0.5 border">
                      Timestamp: {clip.timestamp}
                    </span>
                    <span className="text-[10px] text-neo-blue font-bold flex items-center gap-0.5">
                      <Play size={10} className="fill-neo-blue" /> {clip.timestamp.split(' - ')[0]}
                    </span>
                  </div>
                </div>
              )) : (
                <div className="text-center py-6 text-neo-text-muted italic text-xs">
                  No matching transcript chunks found.
                </div>
              )}
            </div>

          </div>
        </div>

      </div>

      {/* FastCDC Deduplication Simulator */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4">
          FastCDC Deduplication Simulator
        </h3>
        <p className="text-xs text-neo-text-muted mb-4">
          Content-Defined Chunking splits file revisions dynamically to maximize block reuse. Move the slider to inspect the impact of block-size boundaries.
        </p>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-6 items-center">
          <div className="p-4 bg-neo-bg neo-border flex flex-col gap-2">
            <label className="neo-label-sm font-bold block">TARGET CHUNK BOUNDARY: {chunkSizeKb} KB</label>
            <input 
              type="range" 
              min={16} 
              max={256} 
              step={16}
              value={chunkSizeKb} 
              onChange={(e) => setChunkSizeKb(parseInt(e.target.value))}
              className="w-full cursor-pointer h-2 bg-neo-surface rounded-none border-2 border-neo-border accent-black"
            />
            <div className="flex justify-between text-[10px] font-mono text-neo-text-muted">
              <span>16 KB</span>
              <span>256 KB</span>
            </div>
          </div>

          <div className="p-4 bg-neo-surface border-2 border-neo-border text-center">
            <span className="neo-label-sm block text-neo-text-muted">DEDUPLICATION RATIO</span>
            <span className="neo-title-md text-3xl text-neo-mint font-black block my-1">{dedupRatio}%</span>
            <span className="text-[10px] block">Block reuse optimized</span>
          </div>

          <div className="p-4 bg-neo-surface border-2 border-neo-border text-center">
            <span className="neo-label-sm block text-neo-text-muted">STORED SIZE VS BASE</span>
            <span className="neo-title-md text-3xl text-neo-blue font-black block my-1">{finalSizeMb} MB</span>
            <span className="text-[10px] block">Down from {totalBaseSizeMb} MB</span>
          </div>
        </div>
      </div>

      {/* Semantic Diff Explorer Section */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4 flex items-center gap-2">
          <Diff size={18} />
          Per-Type Semantic Diff Explorer (`diff(a, b)` function)
        </h3>
        
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
          {[
            { id: 'image', label: 'Image (PNG/JPG)', desc: 'Perceptual overlay delta' },
            { id: 'godot', label: 'Godot scene (.tscn)', desc: 'Declarative text block comparison' },
            { id: 'figma', label: 'Figma mockups', desc: 'Node-tree vector diff' }
          ].map((item) => (
            <button
              key={item.id}
              onClick={() => setSelectedDiffFile(item.id)}
              className={`neo-btn text-left p-3 flex flex-col gap-1 ${
                selectedDiffFile === item.id ? 'bg-neo-yellow' : 'bg-neo-surface'
              }`}
            >
              <span className="neo-label-md text-xs">{item.label}</span>
              <span className="text-[10px] text-neo-text-muted">{item.desc}</span>
            </button>
          ))}
        </div>

        <div className="neo-border p-4 bg-neo-bg flex flex-col gap-3">
          <div className="flex justify-between items-center border-b border-neo-border pb-2 mb-1">
            <span className="neo-label-sm font-bold text-xs">File: {diffResult[selectedDiffFile].filename}</span>
            <span className="neo-chip neo-chip--review text-[10px]">{diffResult[selectedDiffFile].change}</span>
          </div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div className="p-3 bg-neo-surface neo-border">
              <span className="neo-label-sm block text-[10px] text-neo-text-muted mb-1">DIFF OUTPUT LOGS:</span>
              <pre className="font-mono text-xs whitespace-pre-wrap leading-tight text-neo-text-muted">
                {diffResult[selectedDiffFile].details}
              </pre>
            </div>
            <div className="p-3 bg-neo-surface neo-border flex flex-col justify-center items-center text-center">
              <span className="neo-label-sm block text-[10px] text-neo-text-muted mb-2">VISUAL COMPARISON PREVIEW:</span>
              <div className="p-4 bg-zinc-950 text-white font-mono text-[10px] neo-radius w-full border">
                {diffResult[selectedDiffFile].visual}
              </div>
            </div>
          </div>
        </div>

      </div>

    </div>
  );
}
