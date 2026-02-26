interface SmoothCounterProps {
  value: number | null;
  className?: string;
}

export default function SmoothCounter({ value, className = '' }: SmoothCounterProps) {
  const text = value !== null ? new Intl.NumberFormat('en-US').format(Math.floor(value)) : '—';
  return (
    <span className={`font-mono tabular-nums ${className}`}>{text}</span>
  );
}
