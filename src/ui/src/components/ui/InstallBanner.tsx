import { useState } from 'react';
import { Button } from './Button';
import { InstallSheet } from './InstallSheet';
import { useInstallPrompt } from '../../pwa/useInstallPrompt';
import { CloseIcon } from './icons';
import './ui.css';

const DISMISS_KEY = 'elsewer-install-dismissed-at';
const DISMISS_COOLDOWN_MS = 1000 * 60 * 60 * 24 * 14;

function wasDismissedRecently(): boolean {
  const ts = localStorage.getItem(DISMISS_KEY);
  if (!ts) return false;
  return Date.now() - Number(ts) < DISMISS_COOLDOWN_MS;
}

function markDismissed() {
  localStorage.setItem(DISMISS_KEY, String(Date.now()));
}

export function InstallBanner() {
  const { canInstall, isIOS, promptInstall } = useInstallPrompt();
  const [sheetOpen, setSheetOpen] = useState(false);
  const [dismissed, setDismissed] = useState(wasDismissedRecently);

  if (!canInstall || dismissed) return null;

  function handleInstallClick() {
    if (isIOS) {
      setSheetOpen(true);
    } else {
      void promptInstall();
    }
  }

  function handleDismiss() {
    markDismissed();
    setDismissed(true);
  }

  return (
    <>
      <div className="install-banner">
        <img src="/apple-touch-icon.png" alt="" className="install-banner-icon" />
        <span>Add else-wer to your home screen</span>
        <Button variant="primary" onClick={handleInstallClick}>
          Install
        </Button>
        <button
          className="install-banner-close"
          onClick={handleDismiss}
          aria-label="Dismiss"
          title="Dismiss"
        >
          <CloseIcon size={16} />
        </button>
      </div>
      <InstallSheet open={sheetOpen} onClose={() => setSheetOpen(false)} />
    </>
  );
}
