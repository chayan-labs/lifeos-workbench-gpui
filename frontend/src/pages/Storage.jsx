import React, { useState } from 'react';
import { FolderGit2, FileClock, FileCode } from 'lucide-react';
import Tabs from '../components/ui/Tabs';
import Repository from './Repository';
import TimeTravel from '../components/TimeTravel';
import VcsIngest from './VcsIngest';

// Merges Repository + VCS/time-travel + Media Ingest into one Storage page.
// Files tab = repo browser (bytes live in chosen backend); Versions = VCS
// time-travel (AI-gated); Ingest = media -> text.
const TABS = [
  { id: 'files', label: 'Files', icon: FolderGit2 },
  { id: 'versions', label: 'Versions', icon: FileClock },
  { id: 'ingest', label: 'Media Ingest', icon: FileCode },
];

export default function Storage() {
  const [tab, setTab] = useState('files');
  return (
    <div className="flex flex-col gap-6">
      <Tabs tabs={TABS} active={tab} onChange={setTab} />
      {tab === 'files' && <Repository />}
      {tab === 'versions' && <TimeTravel />}
      {tab === 'ingest' && <VcsIngest />}
    </div>
  );
}
