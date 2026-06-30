import React, { useEffect, useRef, useState } from 'react';
import { marked } from 'marked';
import DOMPurify from 'dompurify';
import katex from 'katex';
import 'katex/dist/katex.min.css';

// Markdown setup matching Knowledge Atlas
marked.setOptions({ gfm: true, breaks: false });

const renderTeX = (tex, display) => {
  try {
    return katex.renderToString(tex, { displayMode: display, throwOnError: true, output: "html" });
  } catch (e) {
    const lit = (display ? "$$" : "$") + tex + (display ? "$$" : "$");
    return String(lit).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }
};

marked.use({
  extensions: [
    {
      name: "mathBlock",
      level: "block",
      start(s) { const i = s.indexOf("$$"); return i < 0 ? undefined : i; },
      tokenizer(src) {
        const m = /^\$\$([\s\S]+?)\$\$/.exec(src);
        if (m) return { type: "mathBlock", raw: m[0], text: m[1].trim() };
      },
      renderer(t) { return renderTeX(t.text, true); }
    },
    {
      name: "mathInline",
      level: "inline",
      start(s) { const i = s.indexOf("$"); return i < 0 ? undefined : i; },
      tokenizer(src) {
        const m = /^\$(?!\s)((?:\\.|[^\n$])+?)(?<!\s)\$/.exec(src);
        if (m) return { type: "mathInline", raw: m[0], text: m[1] };
      },
      renderer(t) { return renderTeX(t.text, false); }
    }
  ]
});

export default function MarkdownRenderer({ content, className = '' }) {
  const [html, setHtml] = useState('');

  useEffect(() => {
    let source = Array.isArray(content) ? content.join('\n\n') : String(content || '');
    const parsed = marked.parse(source);
    // Open links in new tab.
    const withTargets = parsed.replace(/<a /g, '<a target="_blank" rel="noopener" ');
    // Sanitize before injecting: this content is AI- and user-authored (notes,
    // summaries, annotation answers, console plans), so unsanitized HTML through
    // dangerouslySetInnerHTML is a stored-XSS vector. DOMPurify strips scripts
    // and event handlers while preserving KaTeX's span markup and our links.
    const finalHtml = DOMPurify.sanitize(withTargets, { ADD_ATTR: ['target'] });
    setHtml(finalHtml);
  }, [content]);

  return (
    <div
      className={`prose prose-sm dark:prose-invert max-w-none font-medium leading-relaxed ${className}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}