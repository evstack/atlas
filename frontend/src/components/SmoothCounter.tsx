import React from 'react';

interface SmoothCounterProps {
  value: number | null;
  className?: string;
}

export default function SmoothCounter({ value, className = '' }: SmoothCounterProps) {
  const text = value !== null ? new Intl.NumberFormat('en-US').format(Math.floor(value)) : '—';
  // Key on value so the animation restarts on change
  return (
    <span className={`font-mono ${className}`}>
      <span key={text} className="fade-in-up inline-block align-bottom">{text}</span>
    </span>
  );
}

