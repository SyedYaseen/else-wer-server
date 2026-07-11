import { useEffect, useState } from 'react';
import { subscribeToast, type ToastMessage } from '../../lib/toast';
import './ui.css';

export function Toaster() {
  const [toasts, setToasts] = useState<ToastMessage[]>([]);

  useEffect(() => subscribeToast(setToasts), []);

  if (toasts.length === 0) return null;

  return (
    <div className="toast-stack">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.variant}`}>
          {t.message}
        </div>
      ))}
    </div>
  );
}
