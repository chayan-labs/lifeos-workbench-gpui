import React, { useState } from 'react';
import { ShieldAlert, KeyRound, User, Briefcase, Sparkles, CheckCircle } from 'lucide-react';
import BrandMark from '../components/BrandMark';

export default function LoginPage({ onLogin }) {
  const [isLogin, setIsLogin] = useState(true);
  const [email, setEmail] = useState('chayan@example.com');
  const [password, setPassword] = useState('password');
  
  // Registration States
  const [regName, setRegName] = useState('');
  const [regEmail, setRegEmail] = useState('');
  const [regWorkspace, setRegWorkspace] = useState('');
  const [regSuccessData, setRegSuccessData] = useState(null);

  const [error, setError] = useState('');

  const handleLoginSubmit = (e) => {
    e.preventDefault();
    
    // Check default personal login
    if (email === 'chayan@example.com' && password === 'password') {
      localStorage.setItem('life_os_loggedin', 'true');
      localStorage.setItem('life_os_user_email', email);
      localStorage.setItem('life_os_user_name', 'Chayan Aggarwal');
      localStorage.setItem('life_os_workspace_id', 'ws_personal_default');
      localStorage.setItem('life_os_workspace_name', 'Personal Life OS');
      onLogin();
      return;
    }

    // Check custom registered users from localStorage
    const savedUsers = JSON.parse(localStorage.getItem('life_os_registered_users') || '[]');
    const match = savedUsers.find(u => u.email === email && u.key === password);

    if (match) {
      localStorage.setItem('life_os_loggedin', 'true');
      localStorage.setItem('life_os_user_email', email);
      localStorage.setItem('life_os_user_name', match.name);
      localStorage.setItem('life_os_workspace_id', match.workspace_id);
      localStorage.setItem('life_os_workspace_name', match.workspace_name);
      onLogin();
    } else {
      setError('Invalid email or key. Use chayan@example.com / password, or register a new workspace.');
    }
  };

  const handleRegisterSubmit = (e) => {
    e.preventDefault();
    setError('');

    const payload = {
      email: regEmail,
      name: regName,
      workspace_name: regWorkspace
    };

    // Attempt registration against the local Axum Rust API
    fetch('http://127.0.0.1:8080/api/register', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload)
    })
      .then(res => res.json())
      .then(data => {
        // Success handler from Rust server
        const newUser = {
          email: regEmail,
          name: regName,
          workspace_name: regWorkspace,
          workspace_id: data.workspace_id,
          key: data.key_token
        };
        
        saveUserLocally(newUser);
      })
      .catch(() => {
        // Fallback locally if Rust server is offline
        const mockWorkspaceId = "ws_" + Math.random().toString(36).substring(2, 10);
        const mockKey = "key_" + Math.random().toString(36).substring(2, 12);
        
        const newUser = {
          email: regEmail,
          name: regName,
          workspace_name: regWorkspace,
          workspace_id: mockWorkspaceId,
          key: mockKey
        };

        saveUserLocally(newUser);
      });
  };

  const saveUserLocally = (newUser) => {
    const savedUsers = JSON.parse(localStorage.getItem('life_os_registered_users') || '[]');
    savedUsers.push(newUser);
    localStorage.setItem('life_os_registered_users', JSON.stringify(savedUsers));
    
    // Show success view with generated keys
    setRegSuccessData(newUser);
  };

  const proceedToLogin = () => {
    setEmail(regSuccessData.email);
    setPassword(regSuccessData.key);
    setIsLogin(true);
    setRegSuccessData(null);
    setRegName('');
    setRegEmail('');
    setRegWorkspace('');
  };

  return (
    <div className="min-h-screen neo-bg flex items-center justify-center p-6">
      <div className="w-full max-w-[480px] neo-surface neo-border-thick neo-shadow bg-white p-8 flex flex-col gap-6">
        {/* Brand Header */}
        <div className="flex flex-col items-center gap-3 text-center border-b-4 border-black pb-5">
          <BrandMark className="w-20 h-20" />
          <div>
            <h1 className="neo-title-lg text-black leading-none">Life OS</h1>
            <span className="neo-label-sm text-[var(--neo-text-muted)] mt-1 block">Unified Agentic Terminal</span>
          </div>
        </div>

        {/* Tab Toggle */}
        {!regSuccessData && (
          <div className="flex border-4 border-black neo-shadow-sm">
            <button
              onClick={() => { setIsLogin(true); setError(''); }}
              className={`flex-1 py-2 font-mono text-xs font-bold uppercase transition-colors ${
                isLogin ? 'bg-[var(--neo-yellow)]' : 'bg-white'
              }`}
            >
              Sign In
            </button>
            <button
              onClick={() => { setIsLogin(false); setError(''); }}
              className={`flex-1 py-2 font-mono text-xs font-bold uppercase border-l-4 border-black transition-colors ${
                !isLogin ? 'bg-[var(--neo-yellow)]' : 'bg-white'
              }`}
            >
              Register Workspace
            </button>
          </div>
        )}

        {/* SUCCESS VIEW AFTER REGISTER */}
        {regSuccessData ? (
          <div className="flex flex-col gap-5">
            <div className="p-3 bg-[var(--neo-mint)] border-2 border-black flex items-center gap-2 neo-label-sm text-black">
              <CheckCircle size={16} />
              <span className="font-bold">Workspace Scaffolded Successfully!</span>
            </div>

            <p className="text-xs font-semibold leading-relaxed text-[var(--neo-text-muted)]">
              Your tenant domain is compiled. Store this private key carefully. You will use it as your identity access password.
            </p>

            <div className="p-4 bg-[var(--neo-bg)] neo-border font-mono text-xs flex flex-col gap-2">
              <div>
                <span className="text-gray-500 font-bold block">WORKSPACE NAME:</span>
                <span className="font-bold text-black">{regSuccessData.workspace_name}</span>
              </div>
              <div className="mt-2">
                <span className="text-gray-500 font-bold block">TENANT ID:</span>
                <span className="font-mono text-blue-600 font-bold">{regSuccessData.workspace_id}</span>
              </div>
              <div className="mt-2">
                <span className="text-gray-500 font-bold block">PRIVATE KEY (PASSWORD):</span>
                <span className="font-mono text-pink-600 font-bold break-all select-all">{regSuccessData.key}</span>
              </div>
            </div>

            <button
              onClick={proceedToLogin}
              className="w-full py-3 neo-border neo-shadow bg-[var(--neo-mint)] text-black font-bold uppercase transition-all"
            >
              Prefill & Login Now →
            </button>
          </div>
        ) : isLogin ? (
          /* LOGIN FORM */
          <form onSubmit={handleLoginSubmit} className="flex flex-col gap-5">
            {error && (
              <div className="p-3 bg-[var(--neo-red)] text-white border-2 border-black flex items-center gap-2 neo-label-sm">
                <ShieldAlert size={16} />
                <span>{error}</span>
              </div>
            )}

            <div className="flex flex-col gap-2">
              <label className="neo-label-md flex items-center gap-1.5" htmlFor="email">
                <User size={16} />
                <span>Identity Email</span>
              </label>
              <input
                id="email"
                type="email"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="p-3 neo-border bg-white text-black font-semibold focus:outline-none focus:bg-[var(--neo-bg)] font-mono text-sm"
                required
              />
            </div>

            <div className="flex flex-col gap-2">
              <label className="neo-label-md flex items-center gap-1.5" htmlFor="password">
                <KeyRound size={16} />
                <span>Identity Key / Password</span>
              </label>
              <input
                id="password"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="p-3 neo-border bg-white text-black font-semibold focus:outline-none focus:bg-[var(--neo-bg)] font-mono text-sm"
                required
              />
            </div>

            <button
              type="submit"
              className="w-full py-4 neo-border neo-shadow bg-[var(--neo-mint)] text-black font-bold uppercase transition-all"
            >
              Authenticate & Enter →
            </button>
          </form>
        ) : (
          /* REGISTRATION / CREATION FORM */
          <form onSubmit={handleRegisterSubmit} className="flex flex-col gap-4">
            <div className="flex flex-col gap-1.5">
              <label className="neo-label-md flex items-center gap-1.5" htmlFor="regName">
                <User size={14} />
                <span>Full Name</span>
              </label>
              <input
                id="regName"
                type="text"
                value={regName}
                onChange={(e) => setRegName(e.target.value)}
                placeholder="e.g. Chayan Aggarwal"
                className="p-2.5 neo-border bg-white text-xs font-semibold focus:outline-none"
                required
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="neo-label-md flex items-center gap-1.5" htmlFor="regEmail">
                <Sparkles size={14} />
                <span>Email Address</span>
              </label>
              <input
                id="regEmail"
                type="email"
                value={regEmail}
                onChange={(e) => setRegEmail(e.target.value)}
                placeholder="e.g. chayan@example.com"
                className="p-2.5 neo-border bg-white text-xs font-mono focus:outline-none"
                required
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="neo-label-md flex items-center gap-1.5" htmlFor="regWorkspace">
                <Briefcase size={14} />
                <span>Workspace Name</span>
              </label>
              <input
                id="regWorkspace"
                type="text"
                value={regWorkspace}
                onChange={(e) => setRegWorkspace(e.target.value)}
                placeholder="e.g. Chayan's Second Brain"
                className="p-2.5 neo-border bg-white text-xs font-semibold focus:outline-none"
                required
              />
            </div>

            <button
              type="submit"
              className="w-full py-4 neo-border neo-shadow bg-[var(--neo-mint)] text-black font-bold uppercase transition-all mt-2"
            >
              Scaffold Tenant Workspace →
            </button>
          </form>
        )}

        {/* Footer info */}
        <div className="pt-4 border-t-2 border-[var(--neo-border)] border-dashed text-center">
          <span className="neo-label-sm text-[var(--neo-text-muted)]">
            Local Host Node: <span className="font-mono text-black">127.0.0.1:8080</span>
          </span>
        </div>
      </div>
    </div>
  );
}
