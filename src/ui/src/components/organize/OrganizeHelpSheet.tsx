import { useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';
import { HelpIcon } from '../ui/icons';
import { PencilIcon, MoveIcon, MergeIcon, SearchIcon, LayersIcon } from './icons';

const LEGEND = [
  {
    icon: <PencilIcon />,
    name: 'Rename',
    description: "Change the name of an author, book, or file. This doesn't move any files.",
  },
  {
    icon: <MoveIcon />,
    name: 'Move',
    description:
      'Take selected files, a whole book, or a whole author and place them under a different author or book (or start a brand-new one). Typing a name shows matching existing authors/books to pick from, so you don’t accidentally create a near-duplicate.',
  },
  {
    icon: <MergeIcon />,
    name: 'Merge',
    description:
      "Combine an entire book (or author) into another existing one, or into a brand-new one. This deletes the source once its files are moved, so double-check the confirmation dialog before confirming.",
  },
  {
    icon: <SearchIcon />,
    name: 'Match',
    description: "Search an online catalog for this book's real title, author, and cover, then optionally apply it.",
  },
  {
    icon: <LayersIcon />,
    name: 'Set series',
    description: 'Assign a series name and book number to one or more books at once.',
  },
];

export function OrganizeHelpSheet() {
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        className="icon-btn"
        onClick={() => setOpen(true)}
        aria-label="What do these actions mean?"
        title="What do these actions mean?"
      >
        <HelpIcon size={20} />
      </button>
      <BottomSheet open={open} onClose={() => setOpen(false)}>
        <h3 className="sheet-title">How organizing works</h3>
        <div className="help-legend">
          {LEGEND.map((item) => (
            <div className="help-legend-row" key={item.name}>
              <span className="help-legend-icon">{item.icon}</span>
              <div>
                <div className="help-legend-name">{item.name}</div>
                <div className="help-legend-description">{item.description}</div>
              </div>
            </div>
          ))}
        </div>
        <p className="help-legend-tip">
          Every action shows a confirmation screen describing exactly what will change before
          anything is saved — nothing is applied until you tap Confirm there.
        </p>
        <p className="help-legend-tip">
          Tip: on your phone, press and hold a button with just an icon to see what it does. On a
          computer, hover your mouse over it.
        </p>
        <Button variant="primary" onClick={() => setOpen(false)}>
          Got it
        </Button>
      </BottomSheet>
    </>
  );
}
