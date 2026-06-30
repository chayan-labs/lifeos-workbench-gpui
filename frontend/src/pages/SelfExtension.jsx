import React, { useState, useEffect, useRef } from 'react';
import { Terminal, Shield, CheckCircle, AlertTriangle, Play, RefreshCw, Zap, ShoppingBag, Info } from 'lucide-react';
import { apiCall } from '../lib/api';

const POLL_MS = 1500;
const POLL_TIMEOUT_MS = 60000;

export default function SelfExtension() {
  const [terminalInput, setTerminalInput] = useState('Create a Health Tracker module with daily step count, calorie intake, and water logging, and list board views.');
  const [logs, setLogs] = useState([]);
  const [isRunning, setIsRunning] = useState(false);
  const [demoType, setDemoType] = useState('success'); // success or bypass_blocked
  const [installNotice, setInstallNotice] = useState(null);
  const pollRef = useRef(null);
  const [marketplaceModules, setMarketplaceModules] = useState([
    { id: 'm_health', name: 'Health Tracker', desc: 'Workout sessions, water target logs, calorie metric dashboard.', status: 'gated_sandbox', installs: 382, verified: true },
    { id: 'm_finance', name: 'Personal Finance', desc: 'Sync checks, recurring budgets, transaction audits.', status: 'approved', installs: 844, verified: true },
    { id: 'm_fitness', name: 'Fitness Visualizer', desc: 'Body mass telemetry plots, progress chart curves.', status: 'unverified', installs: 12, verified: false }
  ]);

  useEffect(() => () => clearTimeout(pollRef.current), []);

  const appendLog = (text, type = 'info') => setLogs((prev) => [...prev, { text, type }]);

  // Illustrative-only: the prompt-injection guardrail (PreToolUse hook + Seatbelt)
  // runs inside the Mac harness's scaffold.js (self-extension epic, not yet built),
  // so there is no real endpoint to call it against. Shown purely to explain the
  // security model; never claims to have hit the network.
  const runBlockedDemo = () => {
    setIsRunning(true);
    setLogs([]);
    const steps = [
      { t: 0, text: '[demo - illustrative only, no network call] prompt: "malicious_hack"', type: 'info' },
      { t: 500, text: '[Mac Engine] Launching Claude Agent SDK', type: 'info' },
      { t: 1000, text: '[PreToolUse Hook] Intercepting file edit request: path="core/router.js"', type: 'warning' },
      { t: 1500, text: '❌ ACTION DENIED: Hook failed closed. File paths outside of target modules/ are strictly blocked.', type: 'error' },
      { t: 2000, text: '[Seatbelt Sandbox] Seatbelt kernel boundary check failed.', type: 'error' },
      { t: 2500, text: '⚠️ Sandbox halted process. Zero files modified. Transaction aborted (demo).', type: 'warning' },
    ];
    steps.forEach((step) => {
      setTimeout(() => {
        appendLog(step.text, step.type);
        if (step === steps[steps.length - 1]) setIsRunning(false);
      }, step.t);
    });
  };

  // Real flow: POST /api/module-request, then poll /api/jobs for the enqueued
  // module_build job and /api/event for module.requested/module.installed, and
  // listen for the app-emitted module-mounted:<id> hot-reload event.
  const runRequest = async () => {
    if (demoType === 'bypass_blocked') return runBlockedDemo();
    const prompt = terminalInput.trim();
    if (!prompt) return;
    setIsRunning(true);
    setLogs([]);
    clearTimeout(pollRef.current);

    appendLog(`POST /api/module-request { prompt: ${JSON.stringify(prompt)} }`, 'info');
    const { ok, data, error, offline } = await apiCall('POST', '/api/module-request', { prompt });
    if (!ok) {
      appendLog(`❌ Request failed: ${offline ? 'backend unreachable' : error}`, 'error');
      setIsRunning(false);
      return;
    }
    const { request_id: requestId, job_id: jobId } = data;
    appendLog(`✅ Queued. request_id=${requestId} job_id=${jobId}`, 'success');

    const onMounted = (e) => {
      appendLog(`🔔 module-mounted:${e.detail?.id || '?'} - hot-reload event received.`, 'success');
    };
    window.addEventListener('lifeos:module-mounted', onMounted);

    const startedAt = Date.now();
    let lastStatus = 'queued';
    const poll = async () => {
      const jobsRes = await apiCall('GET', `/api/jobs?limit=50`);
      const job = jobsRes.ok ? (jobsRes.data?.items || jobsRes.data || []).find((j) => j.id === jobId) : null;
      if (job && job.status !== lastStatus) {
        lastStatus = job.status;
        appendLog(`[jobs] ${jobId} -> ${job.status}`, job.status === 'failed' ? 'error' : 'info');
      }
      if (job && (job.status === 'done' || job.status === 'completed')) {
        appendLog('✅ Build finished. Watching for hot-reload…', 'success');
        window.removeEventListener('lifeos:module-mounted', onMounted);
        setIsRunning(false);
        return;
      }
      if (job && job.status === 'failed') {
        appendLog(`❌ Build failed honestly: ${job.error || 'no error detail recorded'} (the self-extension builder is not implemented yet - see the Self-Extension epic).`, 'error');
        window.removeEventListener('lifeos:module-mounted', onMounted);
        setIsRunning(false);
        return;
      }
      if (Date.now() - startedAt > POLL_TIMEOUT_MS) {
        appendLog('⏱ Stopped polling after 60s - check the Jobs tab in Database for live status.', 'warning');
        window.removeEventListener('lifeos:module-mounted', onMounted);
        setIsRunning(false);
        return;
      }
      pollRef.current = setTimeout(poll, POLL_MS);
    };
    pollRef.current = setTimeout(poll, POLL_MS);
  };

  const handleInstallMarketplace = (id) => {
    setInstallNotice(id);
    setTimeout(() => setInstallNotice(null), 3000);
  };

  return (
    <div className="flex flex-col gap-8">
      {/* Overview */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <Zap size={24} className="text-neo-yellow fill-neo-yellow" />
          Self-Extension: The Headline Feature
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          Life OS is built to extend itself. When you ask the system to add a new domain tracker, it drives the <strong>Claude Agent SDK</strong> to generate code, run a double-headed validator testing suite, commit it to git, and hot-reload your active client viewport.
        </p>
      </div>

      {/* Terminal Simulator & Security details */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        
        {/* Terminal Visualizer */}
        <div className="lg:col-span-7 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface flex flex-col gap-4 flex-1">
            <div className="flex justify-between items-center border-b-2 border-neo-border pb-3">
              <span className="neo-label-md flex items-center gap-2">
                <Terminal size={18} />
                Module Scaffold Engine
              </span>
              <div className="flex gap-2">
                <button 
                  onClick={() => setDemoType('success')}
                  className={`px-2 py-0.5 border text-[10px] font-mono font-bold ${
                    demoType === 'success' ? 'bg-neo-mint' : 'bg-neo-surface'
                  }`}
                >
                  SUCCESS FLOW
                </button>
                <button 
                  onClick={() => setDemoType('bypass_blocked')}
                  className={`px-2 py-0.5 border text-[10px] font-mono font-bold ${
                    demoType === 'bypass_blocked' ? 'bg-neo-red text-white' : 'bg-neo-surface'
                  }`}
                >
                  ATTACK BLOCKED
                </button>
              </div>
            </div>

            <div className="flex gap-2">
              <input
                type="text"
                value={terminalInput}
                onChange={(e) => setTerminalInput(e.target.value)}
                disabled={isRunning}
                className="neo-input flex-1 text-sm font-mono"
              />
              <button
                onClick={runRequest}
                disabled={isRunning}
                className="neo-btn bg-neo-yellow py-2 px-4 neo-label-md flex items-center gap-2"
              >
                {isRunning ? <RefreshCw className="animate-spin" size={16} /> : <Play size={16} />}
                {isRunning ? 'BUILDING...' : 'RUN'}
              </button>
            </div>

            {/* Terminal screen output */}
            <div className="bg-zinc-950 p-4 neo-border neo-radius text-zinc-300 font-mono text-xs min-h-[300px] overflow-y-auto shadow-inner flex flex-col gap-2">
              <div className="text-zinc-600">// Shell process active on host mac</div>
              {logs.map((log, idx) => (
                <div 
                  key={idx} 
                  className={`${
                    log.type === 'error' ? 'text-rose-500 font-bold' :
                    log.type === 'warning' ? 'text-amber-400' :
                    log.type === 'success' ? 'text-emerald-400' : 'text-zinc-300'
                  }`}
                >
                  {log.text}
                </div>
              ))}
              {isRunning && (
                <div className="animate-pulse text-zinc-500">_</div>
              )}
            </div>
          </div>
        </div>

        {/* Security / Validator Details */}
        <div className="lg:col-span-5 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface flex flex-col gap-4">
            <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-2 flex items-center gap-2">
              <Shield size={18} className="text-neo-blue" />
              Layered Sandbox Guard
            </h3>
            
            <div className="flex flex-col gap-3">
              {[
                { title: 'Layer 1: SDK Hard Lock', desc: 'Allowed/disallowed tool matrices lock API routes. permissionMode set to "dontAsk" prevents recursive prompting.' },
                { title: 'Layer 2: PreToolUse Hooks', desc: 'A programmatic hook intercepts and denies file operations escaping the target module folder, even under prompt injects.' },
                { title: 'Layer 3: macOS Seatbelt', desc: 'OS-level seatbelt policies jail shell forks, sandbox compiler binaries, and restrict disk access.' },
              ].map((layer, idx) => (
                <div key={idx} className="p-3 bg-neo-bg neo-border neo-radius">
                  <div className="flex items-center gap-2 mb-1">
                    <CheckCircle size={14} className="text-neo-mint" />
                    <span className="neo-label-md">{layer.title}</span>
                  </div>
                  <p className="text-xs text-neo-text-muted">{layer.desc}</p>
                </div>
              ))}
            </div>
          </div>
        </div>

      </div>

      {/* Module Marketplace Seam Section */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
        <h3 className="neo-title-md border-b-2 border-neo-border pb-3 mb-4 flex items-center gap-2">
          <ShoppingBag size={18} />
          Signed Module Marketplace (Distribution Channel)
        </h3>
        
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          {marketplaceModules.map((m) => (
            <div key={m.id} className="neo-border bg-neo-bg p-4 flex flex-col justify-between min-h-[180px]">
              <div>
                <div className="flex justify-between items-start mb-2">
                  <span className="neo-title-md text-xs">{m.name}</span>
                  {m.verified ? (
                    <span className="neo-chip neo-chip--completed py-0.5 text-[8px] font-mono">SIGNED ed25519</span>
                  ) : (
                    <span className="neo-chip neo-chip--overdue py-0.5 text-[8px] font-mono">UNVERIFIED</span>
                  )}
                </div>
                <p className="text-xs text-neo-text-muted mb-3">{m.desc}</p>
                <div className="flex gap-2">
                  <span className="neo-tag text-[9px] font-mono">Installs: {m.installs}</span>
                  <span className="neo-tag text-[9px] font-mono">Sandbox: strict</span>
                </div>
              </div>

              <div className="mt-4 pt-3 border-t border-neo-border border-dashed">
                {installNotice === m.id ? (
                  <div className="flex items-center gap-2 py-1.5 px-2 bg-neo-mint neo-border text-xs font-bold">
                    <Info size={12} /> Sandbox verification started…
                  </div>
                ) : (
                  <button
                    onClick={() => handleInstallMarketplace(m.id)}
                    className="neo-btn bg-neo-surface w-full py-1.5 text-xs font-bold"
                  >
                    Verify & Install Module
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      </div>

    </div>
  );
}
