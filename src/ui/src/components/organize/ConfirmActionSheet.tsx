import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';

interface ConfirmActionSheetProps {
  open: boolean;
  lines: string[];
  onCancel: () => void;
  onConfirm: () => void;
}

export function ConfirmActionSheet({ open, lines, onCancel, onConfirm }: ConfirmActionSheetProps) {
  if (!open) return null;

  return (
    <BottomSheet open={open} onClose={onCancel}>
      <h3 className="sheet-title">Confirm changes</h3>
      <div className="confirm-list">
        {lines.map((line, i) => (
          <div className="confirm-list-item" key={i}>
            {line}
          </div>
        ))}
      </div>
      <div className="confirm-actions">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button variant="primary" onClick={onConfirm}>
          Confirm
        </Button>
      </div>
    </BottomSheet>
  );
}
