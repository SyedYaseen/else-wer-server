import { useInstallPromptStore } from '../store/installPrompt';
import { isIOSSafari, isStandalone } from './platform';

export function useInstallPrompt() {
  const deferredEvent = useInstallPromptStore((s) => s.deferredEvent);
  const installed = useInstallPromptStore((s) => s.installed);
  const setDeferredEvent = useInstallPromptStore((s) => s.setDeferredEvent);
  const setInstalled = useInstallPromptStore((s) => s.setInstalled);

  const ios = isIOSSafari();
  const canInstall = !isStandalone() && !installed && (ios || deferredEvent !== null);

  async function promptInstall() {
    if (!deferredEvent) return;
    await deferredEvent.prompt();
    const choice = await deferredEvent.userChoice;
    if (choice.outcome === 'accepted') {
      setInstalled(true);
    } else {
      setDeferredEvent(null);
    }
  }

  return { canInstall, isIOS: ios, promptInstall };
}
