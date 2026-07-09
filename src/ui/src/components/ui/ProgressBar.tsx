import './ui.css';

interface ProgressBarProps {
  /** 0–1 */
  value: number;
  thick?: boolean;
  className?: string;
}

export function ProgressBar({ value, thick = false, className = '' }: ProgressBarProps) {
  const pct = Math.min(1, Math.max(0, value)) * 100;
  return (
    <div className={`progress-track ${thick ? 'thick' : ''} ${className}`}>
      <div className="progress-fill" style={{ width: `${pct}%` }} />
    </div>
  );
}
