export type ToastVariant = 'success' | 'error' | 'info';

export interface ToastMessage {
  id: number;
  message: string;
  variant: ToastVariant;
}

let nextId = 1;
let toasts: ToastMessage[] = [];
const listeners = new Set<(toasts: ToastMessage[]) => void>();

function emit() {
  for (const listener of listeners) listener(toasts);
}

export function showToast(message: string, variant: ToastVariant = 'info', durationMs = 3000): void {
  const id = nextId++;
  toasts = [...toasts, { id, message, variant }];
  emit();
  setTimeout(() => {
    toasts = toasts.filter((t) => t.id !== id);
    emit();
  }, durationMs);
}

export function subscribeToast(listener: (toasts: ToastMessage[]) => void): () => void {
  listeners.add(listener);
  listener(toasts);
  return () => listeners.delete(listener);
}
