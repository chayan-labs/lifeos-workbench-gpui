import React from 'react';
import KnowledgeAtlas from '../components/KnowledgeAtlas';

// Knowledge Atlas is now a top-level workspace (previously buried in Modules).
// Every domain is a full learning/notes/roadmap/papers/projects workspace with
// AI at each layer, and any new domain is addable.
export default function Knowledge() {
  return <KnowledgeAtlas />;
}
