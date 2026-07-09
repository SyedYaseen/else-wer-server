import type { HTMLAttributes } from 'react';
import './ui.css';

export function Card({ className = '', ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`card ${className}`} {...props} />;
}
