import React, { useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import LoginPage from './pages/LoginPage';
import Dashboard from './pages/Dashboard';
import Database from './pages/Database';
import Modules from './pages/Modules';
import Knowledge from './pages/Knowledge';
import Harness from './pages/Harness';
import Storage from './pages/Storage';
import Integrations from './pages/Integrations';
import DocsHub from './pages/DocsHub';
import Profile from './pages/Profile';
import RefineDemo from './pages/RefineDemo';
import GraphView from './pages/GraphView';
import InstalledModulePage from './pages/InstalledModulePage';
import ModuleDashboards from './pages/ModuleDashboards';
import AgentLedger from './pages/AgentLedger';

export default function App() {
  const [isLoggedIn, setIsLoggedIn] = useState(
    localStorage.getItem('life_os_loggedin') === 'true'
  );

  const handleLogout = () => {
    localStorage.removeItem('life_os_loggedin');
    setIsLoggedIn(false);
  };

  if (!isLoggedIn) {
    return <LoginPage onLogin={() => setIsLoggedIn(true)} />;
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route
          path="/dashboard"
          element={
            <Layout onLogout={handleLogout}>
              <Dashboard />
            </Layout>
          }
        />
        <Route
          path="/database"
          element={
            <Layout onLogout={handleLogout}>
              <Database />
            </Layout>
          }
        />
        <Route
          path="/modules"
          element={
            <Layout onLogout={handleLogout}>
              <Modules />
            </Layout>
          }
        />
        <Route
          path="/knowledge"
          element={
            <Layout onLogout={handleLogout}>
              <Knowledge />
            </Layout>
          }
        />
        <Route
          path="/harness"
          element={
            <Layout onLogout={handleLogout}>
              <Harness />
            </Layout>
          }
        />
        <Route
          path="/storage"
          element={
            <Layout onLogout={handleLogout}>
              <Storage />
            </Layout>
          }
        />
        <Route
          path="/integrations"
          element={
            <Layout onLogout={handleLogout}>
              <Integrations />
            </Layout>
          }
        />
        <Route
          path="/docs"
          element={
            <Layout onLogout={handleLogout}>
              <DocsHub />
            </Layout>
          }
        />
        <Route
          path="/dashboards"
          element={
            <Layout onLogout={handleLogout}>
              <ModuleDashboards />
            </Layout>
          }
        />
        <Route
          path="/m/:id"
          element={
            <Layout onLogout={handleLogout}>
              <InstalledModulePage />
            </Layout>
          }
        />
        <Route
          path="/graph"
          element={
            <Layout onLogout={handleLogout}>
              <GraphView />
            </Layout>
          }
        />
        <Route
          path="/refine-demo"
          element={
            <Layout onLogout={handleLogout}>
              <RefineDemo />
            </Layout>
          }
        />
        <Route
          path="/profile"
          element={
            <Layout onLogout={handleLogout}>
              <Profile />
            </Layout>
          }
        />
        <Route
          path="/agent-ledger"
          element={
            <Layout onLogout={handleLogout}>
              <AgentLedger />
            </Layout>
          }
        />
        {/* Back-compat redirects for the pre-merge IA */}
        <Route path="/self-extension" element={<Navigate to="/harness" replace />} />
        <Route path="/agent-harness" element={<Navigate to="/harness" replace />} />
        <Route path="/harness-loop" element={<Navigate to="/harness" replace />} />
        <Route path="/repository" element={<Navigate to="/storage" replace />} />
        <Route path="/vcs-ingest" element={<Navigate to="/storage" replace />} />
        <Route path="/" element={<Navigate to="/dashboard" replace />} />
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
