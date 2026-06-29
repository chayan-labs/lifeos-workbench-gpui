#!/bin/bash
find frontend/src -type f -name "*.jsx" -print0 | xargs -0 sed -i '' -e 's/bg-white/bg-\[var(--neo-surface)\]/g' -e 's/text-black/text-\[var(--neo-text)\]/g' -e 's/border-black/border-\[var(--neo-border)\]/g'
