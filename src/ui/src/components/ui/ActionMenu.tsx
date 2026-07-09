import { useState, type ReactNode } from 'react';
import { BottomSheet } from './BottomSheet';
import { MoreIcon } from '../organize/icons';
import './ui.css';

interface ActionMenuItem {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}

interface ActionMenuProps {
  items: (ActionMenuItem | null | false)[];
  title?: string;
}

export function ActionMenu({ items, title = 'More actions' }: ActionMenuProps) {
  const [open, setOpen] = useState(false);
  const visibleItems = items.filter((item): item is ActionMenuItem => Boolean(item));

  if (visibleItems.length === 0) return null;

  return (
    <>
      <button
        type="button"
        className="action-menu-trigger"
        aria-label="More actions"
        onClick={() => setOpen(true)}
      >
        <MoreIcon size={18} />
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)}>
        <h3 className="sheet-title">{title}</h3>
        <div className="action-menu-list">
          {visibleItems.map((item) => (
            <button
              key={item.label}
              className="action-menu-item"
              disabled={item.disabled}
              onClick={() => {
                setOpen(false);
                item.onClick();
              }}
            >
              {item.icon}
              {item.label}
            </button>
          ))}
        </div>
      </BottomSheet>
    </>
  );
}
