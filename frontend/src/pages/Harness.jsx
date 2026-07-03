import React, { useState } from 'react';
import { Boxes, Zap, History, Cpu } from 'lucide-react';
import Tabs from '../components/ui/Tabs';
import AgentHarness from './AgentHarness';
import SelfExtension from './SelfExtension';
import HarnessLoop from './HarnessLoop';
import PipelineBuilder from './PipelineBuilder';

// Merges the harness surfaces (previously separate nav items) into one page
// with tabs: Compose (agent layering), Build (self-extension), Loop
// (observe/eval/release runs), Pipelines (DAG inspection + run history,
// issue #94).
const TABS = [
  { id: 'compose', label: 'Compose', icon: Boxes },
  { id: 'build', label: 'Build', icon: Zap },
  { id: 'loop', label: 'Loop', icon: History },
  { id: 'pipelines', label: 'Pipelines', icon: Cpu },
];

export default function Harness() {
  const [tab, setTab] = useState('compose');
  return (
    <div className="flex flex-col gap-6">
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      {tab === 'compose' && <AgentHarness />}
      {tab === 'build' && <SelfExtension />}
      {tab === 'loop' && <HarnessLoop />}
      {tab === 'pipelines' && <PipelineBuilder />}
    </div>
  );
}
