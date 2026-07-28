import type { HTMLAttributes } from 'react';
import './ui.css';

type Tone = 'neutral' | 'accent' | 'sage' | 'warning' | 'danger';

interface PillProps extends HTMLAttributes<HTMLSpanElement> {
  tone?: Tone;
}

export function Pill({ tone = 'neutral', className = '', ...props }: PillProps) {
  return <span className={`pill ${tone !== 'neutral' ? `pill-${tone}` : ''} ${className}`} {...props} />;
}
