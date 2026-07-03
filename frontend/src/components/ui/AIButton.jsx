import React from 'react';
import { Sparkles, Loader2 } from 'lucide-react';

// Consistent "do it with AI" action. Shows a spinner while `loading`.
export default function AIButton({ children, onClick, loading, disabled, className = '' }) {
  return (
    <button
      onClick={onClick}
      disabled={loading || disabled}
      className={`neo-btn bg-neo-blue text-white py-1.5 px-3 text-xs flex items-center gap-1.5 disabled:opacity-50 ${className}`}
    >
      {loading ? <Loader2 size={13} className="animate-spin" /> : <Sparkles size={13} />}
      {children}
    </button>
  );
}
