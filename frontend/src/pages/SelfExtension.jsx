import React, { useState } from 'react';
import { Terminal, Shield, CheckCircle, AlertTriangle, Play, RefreshCw, Zap, ShoppingBag } from 'lucide-react';

export default function SelfExtension() {
  const [terminalInput, setTerminalInput] = useState('Create a Health Tracker module with daily step count, calorie intake, and water logging, and list board views.');
  const [logs, setLogs] = useState([]);
  const [isRunning, setIsRunning] = useState(false);
  const [demoType, setDemoType] = useState('success'); // success or bypass_blocked
  const [marketplaceModules, setMarketplaceModules] = useState([
    { id: 'm_health', name: 'Health Tracker', desc: 'Workout sessions, water target logs, calorie metric dashboard.', status: 'gated_sandbox', installs: 382, verified: true },
    { id: 'm_finance', name: 'Personal Finance', desc: 'Sync checks, recurring budgets, transaction audits.', status: 'approved', installs: 844, verified: true },
    { id: 'm_fitness', name: 'Fitness Visualizer', desc: 'Body mass telemetry plots, progress chart curves.', status: 'unverified', installs: 12, verified: false }
  ]);

  const runSimulation = () => {
    setIsRunning(true);
    setLogs([]);
    
    const steps = demoType === 'success' ? [
      { t: 0, text: 'POST /api/module-request { prompt: "health", workspace_id: "personal_workspace" }', type: 'info' },
      { t: 600, text: '[Mac Engine] Launching Claude Agent SDK (Model: Claude 3.5 Sonnet)', type: 'info' },
      { t: 1200, text: '[Seatbelt Sandbox] Gating file write access. Base directory restricted strictly to: modules/health/', type: 'success' },
      { t: 1800, text: '[Scaffolder] Copying modules/_template/ -> modules/health/', type: 'info' },
      { t: 2400, text: '[Scaffolder] Populating manifest fields and parsing schema using Zod schema structure...', type: 'info' },
      { t: 3000, text: '[Validator 1] Running structural validator: checking types, views and commands...', type: 'info' },
      { t: 3400, text: '>> STRUCTURAL VALIDATION PASSED. Found 3 views: list, board, calendar.', type: 'success' },
      { t: 4000, text: '[Validator 2] Launching Headless Playwright instance against scratch mock replica DB...', type: 'info' },
      { t: 4600, text: '>> HEADLESS TEST PASSED: Captured custom "module-mounted:health" event with 0 console errors.', type: 'success' },
      { t: 5200, text: '[Git VCS] Creating atomic branch commit: feat: add health tracker module', type: 'success' },
      { t: 5800, text: '[SSE Controller] Dispatching module.installed event to active app clients.', type: 'success' },
      { t: 6400, text: '✅ Health Tracker Module successfully hot-loaded. Hot-reload complete!', type: 'success' }
    ] : [
      { t: 0, text: 'POST /api/module-request { prompt: "malicious_hack", workspace_id: "personal_workspace" }', type: 'info' },
      { t: 600, text: '[Mac Engine] Launching Claude Agent SDK', type: 'info' },
      { t: 1200, text: '[PreToolUse Hook] Intercepting file edit request: path="core/router.js"', type: 'warning' },
      { t: 1800, text: '❌ ACTION DENIED: Hook failed closed. File paths outside of target modules/ are strictly blocked.', type: 'error' },
      { t: 2400, text: '[Seatbelt Sandbox] Seatbelt kernel boundary check failed.', type: 'error' },
      { t: 3000, text: '⚠️ Sandbox halted process. Zero files modified. Transaction aborted.', type: 'warning' }
    ];

    steps.forEach((step) => {
      setTimeout(() => {
        setLogs((prev) => [...prev, step]);
        if (step.text.includes('complete') || step.text.includes('aborted')) {
          setIsRunning(false);
          if (step.text.includes('complete') && demoType === 'success') {
            localStorage.setItem('life_os_module_health', 'true');
          }
        }
      }, step.t);
    });
  };

  const handleInstallMarketplace = (id) => {
    alert(`Starting local sandbox verification sequence for module: ${id}...`);
  };

  return (
    <div className="flex flex-col gap-8">
      {/* Overview */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-white">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <Zap size={24} className="text-[var(--neo-yellow)] fill-[var(--neo-yellow)]" />
          Self-Extension: The Headline Feature
        </h2>
        <p className="neo-body-md text-[var(--neo-text-muted)]">
          Life OS is built to extend itself. When you ask the system to add a new domain tracker, it drives the <strong>Claude Agent SDK</strong> to generate code, run a double-headed validator testing suite, commit it to git, and hot-reload your active client viewport.
        </p>
      </div>

      {/* Terminal Simulator & Security details */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">
        
        {/* Terminal Visualizer */}
        <div className="lg:col-span-7 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white flex flex-col gap-4 flex-1">
            <div className="flex justify-between items-center border-b-2 border-[var(--neo-border)] pb-3">
              <span className="neo-label-md flex items-center gap-2">
                <Terminal size={18} />
                Module Scaffold Engine
              </span>
              <div className="flex gap-2">
                <button 
                  onClick={() => setDemoType('success')}
                  className={`px-2 py-0.5 border text-[10px] font-mono font-bold ${
                    demoType === 'success' ? 'bg-[var(--neo-mint)]' : 'bg-white'
                  }`}
                >
                  SUCCESS FLOW
                </button>
                <button 
                  onClick={() => setDemoType('bypass_blocked')}
                  className={`px-2 py-0.5 border text-[10px] font-mono font-bold ${
                    demoType === 'bypass_blocked' ? 'bg-[var(--neo-red)] text-white' : 'bg-white'
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
                onClick={runSimulation}
                disabled={isRunning}
                className="neo-btn bg-[var(--neo-yellow)] py-2 px-4 neo-label-md flex items-center gap-2"
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
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white flex flex-col gap-4">
            <h3 className="neo-title-md border-b-2 border-[var(--neo-border)] pb-3 mb-2 flex items-center gap-2">
              <Shield size={18} className="text-[var(--neo-blue)]" />
              Layered Sandbox Guard
            </h3>
            
            <div className="flex flex-col gap-3">
              {[
                { title: 'Layer 1: SDK Hard Lock', desc: 'Allowed/disallowed tool matrices lock API routes. permissionMode set to "dontAsk" prevents recursive prompting.' },
                { title: 'Layer 2: PreToolUse Hooks', desc: 'A programmatic hook intercepts and denies file operations escaping the target module folder, even under prompt injects.' },
                { title: 'Layer 3: macOS Seatbelt', desc: 'OS-level seatbelt policies jail shell forks, sandbox compiler binaries, and restrict disk access.' },
              ].map((layer, idx) => (
                <div key={idx} className="p-3 bg-[var(--neo-bg)] neo-border neo-radius">
                  <div className="flex items-center gap-2 mb-1">
                    <CheckCircle size={14} className="text-[var(--neo-mint)]" />
                    <span className="neo-label-md">{layer.title}</span>
                  </div>
                  <p className="text-xs text-[var(--neo-text-muted)]">{layer.desc}</p>
                </div>
              ))}
            </div>
          </div>
        </div>

      </div>

      {/* Module Marketplace Seam Section */}
      <div className="neo-surface neo-border-thick neo-shadow p-5 bg-white">
        <h3 className="neo-title-md border-b-2 border-black pb-3 mb-4 flex items-center gap-2">
          <ShoppingBag size={18} />
          Signed Module Marketplace (Distribution Channel)
        </h3>
        
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          {marketplaceModules.map((m) => (
            <div key={m.id} className="neo-border bg-[var(--neo-bg)] p-4 flex flex-col justify-between min-h-[180px]">
              <div>
                <div className="flex justify-between items-start mb-2">
                  <span className="neo-title-md text-xs">{m.name}</span>
                  {m.verified ? (
                    <span className="neo-chip neo-chip--completed py-0.5 text-[8px] font-mono">SIGNED ed25519</span>
                  ) : (
                    <span className="neo-chip neo-chip--overdue py-0.5 text-[8px] font-mono">UNVERIFIED</span>
                  )}
                </div>
                <p className="text-xs text-[var(--neo-text-muted)] mb-3">{m.desc}</p>
                <div className="flex gap-2">
                  <span className="neo-tag text-[9px] font-mono">Installs: {m.installs}</span>
                  <span className="neo-tag text-[9px] font-mono">Sandbox: strict</span>
                </div>
              </div>

              <div className="mt-4 pt-3 border-t border-[var(--neo-border)] border-dashed">
                <button 
                  onClick={() => handleInstallMarketplace(m.id)}
                  className="neo-btn bg-white w-full py-1.5 text-xs font-bold"
                >
                  Verify & Install Module
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

    </div>
  );
}
