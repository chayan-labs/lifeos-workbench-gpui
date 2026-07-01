import React, { useCallback, useEffect, useState } from 'react';
import { ShieldCheck, Key, Layers, ArrowRight, Plus, X, Unlink, Loader2 } from 'lucide-react';
import Nango from '@nangohq/frontend';
import APIExplorer from '../components/APIExplorer';
import { apiCall, NANGO_CONNECT_UI_URL } from '../lib/api';

// Nango `provider_config_key`s already wired server-side (docs/INTEGRATIONS.md
// §2). Native connectors (Kite, WhatsApp, browser sessions) have their own
// bespoke connect flows (daily login-url, QR scan, headed-browser capture)
// and are listed read-only here alongside the generic Nango connections.
const NANGO_PROVIDERS = [
  { key: 'google-mail', label: 'Gmail' },
  { key: 'google-calendar', label: 'Google Calendar' },
  { key: 'google-drive', label: 'Google Drive' },
  { key: 'notion', label: 'Notion' },
  { key: 'slack', label: 'Slack' },
  { key: 'github', label: 'GitHub' },
];

function statusChip(status) {
  const cls = status === 'active' ? 'neo-chip--completed' : '';
  return <span className={`neo-chip ${cls} py-0.5 text-[9px]`}>{(status || 'unknown').toUpperCase()}</span>;
}

export default function Integrations() {
  const [connections, setConnections] = useState([]);
  const [loading, setLoading] = useState(true);
  const [offline, setOffline] = useState(false);
  const [busyId, setBusyId] = useState(null);
  const [disconnectError, setDisconnectError] = useState(null);
  const [showAddModal, setShowAddModal] = useState(false);
  const [selectedProvider, setSelectedProvider] = useState(NANGO_PROVIDERS[0].key);
  const [connectError, setConnectError] = useState(null);
  const [connecting, setConnecting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    const res = await apiCall('GET', '/api/connections');
    setOffline(res.offline);
    setConnections(res.ok ? res.data : []);
    setLoading(false);
  }, []);

  useEffect(() => { load(); }, [load]);

  const handleDisconnect = async (conn) => {
    setBusyId(conn.id);
    setDisconnectError(null);
    const res = await apiCall('DELETE', `/api/connections/${conn.id}`);
    if (!res.ok) {
      setDisconnectError(
        res.status === 501
          ? `Could not disconnect ${conn.provider} - Nango is not configured on the API yet.`
          : res.error || `Could not disconnect ${conn.provider}.`
      );
    }
    await load();
    setBusyId(null);
  };

  const openConnectModal = () => {
    setConnectError(null);
    setShowAddModal(true);
  };

  const handleAuthorize = async () => {
    setConnectError(null);
    setConnecting(true);

    const sessionRes = await apiCall('POST', '/api/connections/session', { provider: selectedProvider });
    if (!sessionRes.ok) {
      setConnecting(false);
      setConnectError(
        sessionRes.status === 501
          ? 'Nango is not configured on the API yet - see docs/MANUAL-SETUP.md #47.'
          : sessionRes.error || 'Could not start the connect session.'
      );
      return;
    }

    const nango = new Nango();
    const connectUI = nango.openConnectUI({
      baseURL: NANGO_CONNECT_UI_URL,
      onEvent: async (event) => {
        if (event.type === 'connect') {
          const { connectionId, providerConfigKey } = event.payload;
          const completeRes = await apiCall('POST', '/api/connections/complete', {
            connection_id: connectionId,
            provider: providerConfigKey,
          });
          setConnecting(false);
          if (completeRes.ok) {
            setShowAddModal(false);
            await load();
          } else {
            setConnectError(completeRes.error || 'Connected with Nango, but recording the connection failed.');
          }
        } else if (event.type === 'close') {
          setConnecting(false);
        }
      },
    });
    connectUI.setSessionToken(sessionRes.data.session_token);
  };

  return (
    <div className="flex flex-col gap-8">
      {/* Introduction Card */}
      <div className="neo-surface neo-border-thick neo-shadow p-6 bg-neo-surface">
        <h2 className="neo-title-md mb-2 flex items-center gap-2">
          <ShieldCheck size={24} className="text-neo-mint" />
          Secure Integration Architecture: Nango Proxy Vault
        </h2>
        <p className="neo-body-md text-neo-text-muted">
          <strong>Hard Rule: No OAuth tokens are ever injected into the agent context.</strong> Integrations are handled by a self-hosted <strong>Nango</strong> vault. The agent only reads and writes a public <code className="bg-neo-surface-high px-1 py-0.5 neo-border text-neo-blue font-mono">connectionId</code>, while the local API server proxies network requests, injecting OAuth keys automatically at call-time.
        </p>
      </div>

      {/* Main Layout */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-8">

        {/* Connection List */}
        <div className="lg:col-span-7 neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface">
          <div className="flex justify-between items-center border-b-2 border-neo-border pb-3 mb-4">
            <h3 className="neo-title-md flex items-center gap-2">
              <Key size={18} />
              Connections
            </h3>
            <button
              onClick={openConnectModal}
              className="neo-btn py-1 px-2.5 bg-neo-yellow text-xs font-bold flex items-center gap-1"
            >
              <Plus size={12} /> Add Connection
            </button>
          </div>

          {offline && (
            <p className="text-xs text-neo-red font-mono mb-3">Offline - could not reach the local API.</p>
          )}
          {disconnectError && (
            <p className="text-xs text-neo-red font-mono mb-3">{disconnectError}</p>
          )}
          {loading && <p className="text-xs text-neo-text-muted">Loading…</p>}
          {!loading && !offline && connections.length === 0 && (
            <p className="text-xs text-neo-text-muted">No connections yet. Add one to get started.</p>
          )}

          <div className="flex flex-col gap-4">
            {connections.map((conn) => (
              <div key={conn.id} className="p-4 bg-neo-bg neo-border flex flex-col gap-3">
                <div className="flex justify-between items-start">
                  <div>
                    <span className="neo-title-md text-sm">{conn.provider}</span>
                    <span className="text-xs font-mono text-neo-text-muted block mt-1">
                      {conn.account_handle || '—'}
                    </span>
                  </div>
                  {statusChip(conn.status)}
                </div>

                <div className="pt-2 border-t border-neo-border border-dashed text-xs flex justify-between items-center text-neo-text-muted font-mono">
                  <span>
                    Connected: {new Date(conn.created_at * 1000).toLocaleString()}
                    {conn.expires_at ? ` · expires ${new Date(conn.expires_at * 1000).toLocaleString()}` : ''}
                  </span>
                  {conn.status === 'active' && (
                    <button
                      onClick={() => handleDisconnect(conn)}
                      disabled={busyId === conn.id}
                      className="neo-btn py-1 px-2.5 bg-neo-surface text-[10px] font-bold text-neo-text flex items-center gap-1 disabled:opacity-50"
                    >
                      {busyId === conn.id ? <Loader2 size={10} className="animate-spin" /> : <Unlink size={10} />}
                      Disconnect
                    </button>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Nango Security Flow Diagram */}
        <div className="lg:col-span-5 flex flex-col gap-6">
          <div className="neo-surface neo-border-thick neo-shadow p-5 bg-neo-surface flex-1 flex flex-col gap-4">
            <h3 className="neo-title-md border-b-2 border-neo-border pb-3 flex items-center gap-2">
              <Layers size={18} className="text-neo-blue" />
              API Proxy Flow
            </h3>

            <div className="flex flex-col gap-4 text-xs font-mono">
              <div className="p-3 bg-neo-surface-muted neo-border">
                <span className="font-bold text-neo-blue">Step 1: Agent Action</span>
                <p className="mt-1 font-sans text-xs">Agent wants to read a Gmail inbox. Calls the local API with a connectionId only.</p>
                <code className="text-[10px] text-neo-text-muted block mt-2">GET /api/gmail/list?workspace_id=…</code>
              </div>

              <div className="flex justify-center text-[var(--neo-border)]">
                <ArrowRight className="rotate-90" size={18} />
              </div>

              <div className="p-3 bg-neo-surface-muted neo-border">
                <span className="font-bold text-neo-red">Step 2: Nango Proxy Decryption</span>
                <p className="mt-1 font-sans text-xs">The API resolves the connectionId, calls Nango's proxy, which injects the live token server-side.</p>
                <code className="text-[10px] text-neo-text-muted block mt-2">nango.proxy(connectionId, "google-mail", ...)</code>
              </div>

              <div className="flex justify-center text-[var(--neo-border)]">
                <ArrowRight className="rotate-90" size={18} />
              </div>

              <div className="p-3 bg-neo-surface-muted neo-border">
                <span className="font-bold text-neo-mint">Step 3: Provider Response</span>
                <p className="mt-1 font-sans text-xs">The provider API responds and only the data payload flows back - the token never leaves Nango.</p>
              </div>
            </div>

            <div className="mt-auto p-3 bg-neo-surface-high border border-[var(--neo-yellow)] neo-radius text-xs">
              <span className="font-bold block mb-1">Custom API Connectors:</span>
              <p className="text-[11px] text-neo-text-muted">
                Kite Trading API (Zerodha), WhatsApp (self-hosted GOWA), and the browser actuator use envelope-encrypted DB rows instead of Nango's OAuth vault - they appear in this same connection list with a <code>kite</code>, <code>whatsapp</code>, or <code>browser:&lt;site&gt;</code> provider.
              </p>
            </div>
          </div>
        </div>

      </div>

      {/* Full system API surface */}
      <APIExplorer />

      {/* Add Connection Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
          <div className="neo-surface neo-border-thick shadow-[8px_8px_0_0_#1c1c0f] p-6 bg-neo-surface max-w-md w-full relative">
            <button
              onClick={() => setShowAddModal(false)}
              className="absolute right-4 top-4 neo-icon-btn p-1.5"
            >
              <X size={16} />
            </button>
            <h4 className="neo-title-md mb-4 uppercase">Authorize via Nango</h4>
            <div className="flex flex-col gap-4 text-xs">
              <div className="flex flex-col gap-1">
                <label className="neo-label-sm">INTEGRATION PROVIDER</label>
                <select
                  value={selectedProvider}
                  onChange={(e) => setSelectedProvider(e.target.value)}
                  className="neo-input w-full bg-neo-surface cursor-pointer"
                >
                  {NANGO_PROVIDERS.map((p) => (
                    <option key={p.key} value={p.key}>{p.label}</option>
                  ))}
                </select>
              </div>

              {connectError && (
                <div className="p-3 bg-neo-surface-high border border-[var(--neo-red)] text-[11px] text-neo-red">
                  {connectError}
                </div>
              )}

              <button
                onClick={handleAuthorize}
                disabled={connecting}
                className="neo-btn bg-neo-yellow py-3 px-4 neo-label-md mt-2 disabled:opacity-50 flex items-center justify-center gap-2"
              >
                {connecting && <Loader2 size={14} className="animate-spin" />}
                {connecting ? 'Waiting for authorization…' : 'AUTHORIZE CONNECTION'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
