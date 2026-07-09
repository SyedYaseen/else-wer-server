import { BottomSheet } from './BottomSheet';

interface InstallSheetProps {
  open: boolean;
  onClose: () => void;
}

export function InstallSheet({ open, onClose }: InstallSheetProps) {
  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">Add to Home Screen</h3>
      <img src="/apple-touch-icon.png" alt="" className="install-sheet-icon" />
      <ol className="install-steps">
        <li>Tap the Share icon in Safari's toolbar</li>
        <li>Scroll down and tap "Add to Home Screen"</li>
        <li>Tap "Add" in the top right</li>
      </ol>
      <button className="btn btn-secondary" onClick={onClose}>
        Got it
      </button>
    </BottomSheet>
  );
}
