import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import '@fontsource/dm-serif-display/400.css';
import '@fontsource/dm-serif-display/400-italic.css';
import '@fontsource/dm-sans/300.css';
import '@fontsource/dm-sans/400.css';
import '@fontsource/dm-sans/500.css';
import './styles/tokens.css';
import App from './App.tsx';
import { useInstallPromptStore, type BeforeInstallPromptEvent } from './store/installPrompt';
import { isIOSSafari, isStandalone } from './pwa/platform';

window.addEventListener('beforeinstallprompt', (e) => {
  e.preventDefault();
  useInstallPromptStore.getState().setDeferredEvent(e as BeforeInstallPromptEvent);
});
window.addEventListener('appinstalled', () => {
  useInstallPromptStore.getState().setInstalled(true);
});

// iOS standalone PWAs can render the first paint at a stale zoom scale
// (clipped left/right edges) until something forces WebKit to recompute
// layout. Nudging the viewport meta tag's content triggers that recompute.
if (isStandalone() && isIOSSafari()) {
  const fixIOSViewportZoom = () => {
    const viewport = document.querySelector('meta[name="viewport"]');
    if (!viewport) return;
    const original = viewport.getAttribute('content') ?? '';
    viewport.setAttribute('content', `${original}, maximum-scale=1.0`);
    setTimeout(() => viewport.setAttribute('content', original), 300);
  };
  window.addEventListener('load', fixIOSViewportZoom);
  window.addEventListener('pageshow', fixIOSViewportZoom);
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
