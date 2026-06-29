import React, { useState } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import Layout from './components/Layout';
import LoginPage from './pages/LoginPage';
import Dashboard from './pages/Dashboard';
import Database from './pages/Database';
import Modules from './pages/Modules';
import SelfExtension from './pages/SelfExtension';
import HarnessLoop from './pages/HarnessLoop';
import VcsIngest from './pages/VcsIngest';
import Integrations from './pages/Integrations';
import DocsHub from './pages/DocsHub';

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
          path="/self-extension"
          element={
            <Layout onLogout={handleLogout}>
              <SelfExtension />
            </Layout>
          }
        />
        <Route
          path="/harness-loop"
          element={
            <Layout onLogout={handleLogout}>
              <HarnessLoop />
            </Layout>
          }
        />
        <Route
          path="/vcs-ingest"
          element={
            <Layout onLogout={handleLogout}>
              <VcsIngest />
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
          path="/docs_hub"
          element={
            <Layout onLogout={handleLogout}>
              <DocsHub />
            </Layout>
          }
        />
        <Route
          path="/docs-hub"
          element={
            <Layout onLogout={handleLogout}>
              <DocsHub />
            </Layout>
          }
        />
        
        {/* Redirects */}
        <Route path="/" element={<Navigate to="/dashboard" replace />} />
        <Route path="*" element={<Navigate to="/dashboard" replace />} />
      </Routes>
    </BrowserRouter>
  );
}
