import { useEffect, useState } from 'react';
import { BottomSheet } from '../ui/BottomSheet';
import { Button } from '../ui/Button';

export type RenameScope = 'author' | 'book' | 'file';

interface RenameSheetProps {
  open: boolean;
  onClose: () => void;
  scope: RenameScope;
  initialAuthor?: string;
  initialTitle?: string;
  onSubmit: (values: { author?: string; title?: string }) => void;
}

export function RenameSheet({ open, onClose, scope, initialAuthor, initialTitle, onSubmit }: RenameSheetProps) {
  const [author, setAuthor] = useState(initialAuthor ?? '');
  const [title, setTitle] = useState(initialTitle ?? '');

  useEffect(() => {
    if (open) {
      setAuthor(initialAuthor ?? '');
      setTitle(initialTitle ?? '');
    }
  }, [open, initialAuthor, initialTitle]);

  if (!open) return null;

  const showAuthor = scope === 'author' || scope === 'book';
  const showTitle = scope === 'book' || scope === 'file';
  const heading = scope === 'author' ? 'Rename author' : scope === 'book' ? 'Rename book' : 'Rename file';
  const titleLabel = scope === 'file' ? 'File name' : 'Title';

  function submit() {
    const values: { author?: string; title?: string } = {};
    if (showAuthor && author.trim() && author.trim() !== initialAuthor) values.author = author.trim();
    if (showTitle && title.trim() && title.trim() !== initialTitle) values.title = title.trim();
    if (Object.keys(values).length === 0) {
      onClose();
      return;
    }
    onSubmit(values);
    onClose();
  }

  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">{heading}</h3>
      <div className="rename-form">
        {showAuthor && (
          <label className="organize-field">
            <span>Author</span>
            <input className="organize-input" value={author} onChange={(e) => setAuthor(e.target.value)} autoFocus />
          </label>
        )}
        {showTitle && (
          <label className="organize-field">
            <span>{titleLabel}</span>
            <input className="organize-input" value={title} onChange={(e) => setTitle(e.target.value)} />
          </label>
        )}
        <Button onClick={submit}>Save</Button>
      </div>
    </BottomSheet>
  );
}
