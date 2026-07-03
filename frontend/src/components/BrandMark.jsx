import React from 'react';

export default function BrandMark({ className = '' }) {
  return (
    <div className={`relative flex items-center justify-center ${className}`}>
      {/* Globe Container with spinning animation */}
      <svg
        viewBox="0 0 100 100"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        className="w-full h-full animate-spin-slow"
        style={{ transformOrigin: 'center' }}
      >
        {/* Outer Ring with thick border */}
        <circle cx="50" cy="50" r="46" stroke="#1c1c0f" strokeWidth="4" fill="#ffffff" />
        
        {/* Grid lines (Longitudes & Latitudes) */}
        {/* Prime meridian */}
        <line x1="50" y1="4" x2="50" y2="96" stroke="#1c1c0f" strokeWidth="2.5" />
        {/* Equator */}
        <line x1="4" y1="50" x2="96" y2="50" stroke="#1c1c0f" strokeWidth="2.5" />
        
        {/* Curved Longitudes */}
        <path d="M50 4 C20 30, 20 70, 50 96" stroke="#2f29e8" strokeWidth="3" fill="none" />
        <path d="M50 4 C80 30, 80 70, 50 96" stroke="#ff4b4b" strokeWidth="3" fill="none" />
        
        {/* Curved Latitudes */}
        <path d="M6 30 C30 42, 70 42, 94 30" stroke="#00ff9d" strokeWidth="2.5" fill="none" />
        <path d="M6 70 C30 58, 70 58, 94 70" stroke="#ffff00" strokeWidth="2.5" fill="none" />
        
        {/* Inner core circle */}
        <circle cx="50" cy="50" r="14" fill="#ffff00" stroke="#1c1c0f" strokeWidth="3" />
        
        {/* Decorative orbits / dots representing data nodes (Life OS events) */}
        <circle cx="28" cy="30" r="4" fill="#2f29e8" stroke="#1c1c0f" strokeWidth="2" />
        <circle cx="72" cy="70" r="4" fill="#ff4b4b" stroke="#1c1c0f" strokeWidth="2" />
        <circle cx="50" cy="14" r="3" fill="#00ff9d" stroke="#1c1c0f" strokeWidth="2" />
        <circle cx="50" cy="86" r="3" fill="#ffffff" stroke="#1c1c0f" strokeWidth="2" />
      </svg>
    </div>
  );
}
