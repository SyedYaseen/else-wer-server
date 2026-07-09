import type { ButtonHTMLAttributes } from 'react';
import './ui.css';

type Variant = 'primary' | 'secondary' | 'ghost' | 'destructive';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

export function Button({ variant = 'primary', className = '', ...props }: ButtonProps) {
  return <button className={`btn btn-${variant} ${className}`} {...props} />;
}
