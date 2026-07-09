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

window.addEventListener('beforeinstallprompt', (e) => {
  e.preventDefault();
  useInstallPromptStore.getState().setDeferredEvent(e as BeforeInstallPromptEvent);
});
window.addEventListener('appinstalled', () => {
  useInstallPromptStore.getState().setInstalled(true);
});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
