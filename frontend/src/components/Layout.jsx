import React, { useState } from 'react';
import { Link, useLocation } from 'react-router-dom';
import BrandMark from './BrandMark';
import { 
  LayoutDashboard, 
  Database, 
  Cpu, 
  Settings, 
  History, 
  Zap, 
  Globe, 
  Menu, 
  X, 
  FileCode,
  Terminal,
  FileText,
  LogOut
} from 'lucide-react';

export default function Layout({ children, onLogout }) {
  const location = useLocation();
  const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);

  const navigation = [
    { name: 'Dashboard', href: '/dashboard', icon: LayoutDashboard },
    { name: 'Unified Database', href: '/database', icon: Database },
    { name: 'Seed Modules', href: '/modules', icon: Cpu },
    { name: 'Self-Extension', href: '/self-extension', icon: Zap },
    { name: 'Harness Loop', href: '/harness-loop', icon: History },
    { name: 'VCS & Media Ingest', href: '/vcs-ingest', icon: FileCode },
    { name: 'Integrations', href: '/integrations', icon: Settings },
    { name: 'System Docs', href: '/docs', icon: FileText },
  ];

  const isActive = (path) => location.pathname === path;
  const activeLabel = navigation.find((item) => isActive(item.href))?.name || 'Overview';

  return (
    <div className="min-h-screen neo-bg text-[var(--neo-text)] flex">
      {/* Mobile Drawer Menu */}
      {isMobileMenuOpen && (
        <div className="fixed inset-0 z-50 md:hidden">
          <button
            type="button"
            className="absolute inset-0 w-full h-full bg-[var(--neo-border)]/40"
            aria-label="Close navigation menu"
            onClick={() => setIsMobileMenuOpen(false)}
          />
          <aside className="absolute left-0 top-0 h-full w-[280px] neo-surface neo-border-thick shadow-[4px_0px_0px_0px_#1c1c0f] flex flex-col">
            <div className="px-6 py-6 neo-divider flex flex-col items-center gap-3 text-center">
              <BrandMark className="w-20 h-20" />
              <div>
                <div className="neo-title-md leading-none">Life OS</div>
                <div className="neo-label-sm text-[var(--neo-text-muted)] mt-1">Agentic Personal OS</div>
              </div>
            </div>

            <nav className="flex-1 px-4 py-4 space-y-2 overflow-y-auto">
              {navigation.map((item) => (
                <Link
                  key={item.name}
                  to={item.href}
                  onClick={() => setIsMobileMenuOpen(false)}
                  className={`flex items-center gap-3 px-4 py-3 neo-label-md transition-all border-2 ${
                    isActive(item.href)
                      ? 'bg-[var(--neo-blue)] text-white border-[var(--neo-border)] neo-shadow'
                      : 'text-[var(--neo-text)] border-transparent hover:bg-[var(--neo-surface-high)] hover:border-[var(--neo-border)] neo-shadow-hover'
                  }`}
                >
                  <item.icon size={20} />
                  {item.name}
                </Link>
              ))}
            </nav>

            <div className="px-4 py-6 mt-auto border-t-2 border-[var(--neo-border)] flex flex-col gap-2">
              <div className="text-center neo-label-sm text-[var(--neo-text-muted)]">
                Local Trusted Host (Mac OS)
              </div>
              <div className="flex items-center gap-2 justify-center neo-tag">
                <Terminal size={14} />
                <span>ONLINE: 127.0.0.1</span>
              </div>
            </div>
          </aside>
        </div>
      )}

      {/* Desktop Sidebar */}
      <aside className="hidden md:flex md:flex-col w-[280px] neo-surface neo-border-thick border-r-4 border-[var(--neo-border)] shadow-[4px_0px_0px_0px_#1c1c0f]">
        <div className="px-6 py-8 neo-divider flex flex-col items-center gap-4 text-center">
          <BrandMark className="w-24 h-24" />
          <div>
            <div className="neo-title-lg leading-none">Life OS</div>
            <div className="neo-label-sm text-[var(--neo-text-muted)] mt-2">Agentic Personal OS</div>
          </div>
        </div>

        <nav className="flex-1 px-4 py-6 space-y-3">
          {navigation.map((item) => (
            <Link
              key={item.name}
              to={item.href}
              className={`flex items-center gap-3 px-4 py-3 neo-label-md transition-all border-2 ${
                isActive(item.href)
                  ? 'bg-[var(--neo-blue)] text-white border-[var(--neo-border)] neo-shadow'
                  : 'text-[var(--neo-text)] border-transparent hover:bg-[var(--neo-surface-high)] hover:border-[var(--neo-border)] neo-shadow-hover'
              }`}
            >
              <item.icon size={20} />
              {item.name}
            </Link>
          ))}
        </nav>

        <div className="px-4 py-6 mt-auto border-t-2 border-[var(--neo-border)] flex flex-col gap-2">
          <div className="text-center neo-label-sm text-[var(--neo-text-muted)]">
            Local Trusted Host (Mac OS)
          </div>
          <div className="flex items-center gap-2 justify-center neo-tag">
            <Terminal size={14} />
            <span>ONLINE: 127.0.0.1</span>
          </div>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col min-w-0 overflow-hidden">
        <header className="sticky top-0 z-40 neo-surface neo-divider shadow-[0px_4px_0px_0px_#1c1c0f] px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <button
              onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
              className="md:hidden neo-icon-btn neo-radius-none p-2"
              aria-label="Toggle menu"
            >
              {isMobileMenuOpen ? <X size={20} /> : <Menu size={20} />}
            </button>
            <h1 className="neo-title-md hidden md:block">{activeLabel}</h1>
            <span className="neo-tag bg-[var(--neo-yellow)] hidden lg:inline-flex">SAAS-READY</span>
          </div>

          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2 px-3 py-1.5 neo-border bg-[var(--neo-mint)] neo-shadow-sm neo-label-sm">
              <div className="w-2.5 h-2.5 rounded-full bg-[var(--neo-text)] animate-pulse" />
              <span>Turso replica synced</span>
            </div>
            
            <button
              onClick={onLogout}
              className="neo-btn p-2 bg-[var(--neo-red)] text-white hover:bg-[var(--neo-red)]/90 flex items-center gap-1 text-xs font-mono font-bold"
              title="Logout session"
            >
              <LogOut size={16} />
              <span className="hidden sm:inline">LOGOUT</span>
            </button>
            
            <div className="w-10 h-10 neo-border neo-shadow bg-[var(--neo-yellow)] flex items-center justify-center font-bold">
              LO
            </div>
          </div>
        </header>

        <div className="flex-1 overflow-y-auto p-6 md:p-8">
          {children}
        </div>
      </main>
    </div>
  );
}
