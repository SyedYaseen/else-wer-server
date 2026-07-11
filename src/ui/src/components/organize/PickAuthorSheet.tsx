import { BottomSheet } from '../ui/BottomSheet';
import { SearchableCombobox } from './SearchableCombobox';
import type { AuthorGroup } from '../../types/scan';

interface PickAuthorSheetProps {
  open: boolean;
  onClose: () => void;
  title: string;
  tree: AuthorGroup[];
  excludeAuthor?: string;
  onPick: (author: string) => void;
}

export function PickAuthorSheet({ open, onClose, title, tree, excludeAuthor, onPick }: PickAuthorSheetProps) {
  if (!open) return null;

  const authors = tree.map((a) => a.author).filter((a) => a !== excludeAuthor);

  function pick(author: string) {
    onPick(author);
    onClose();
  }

  return (
    <BottomSheet open={open} onClose={onClose}>
      <h3 className="sheet-title">{title}</h3>
      <label className="organize-field">
        <span>Author</span>
        <SearchableCombobox
          items={authors}
          getLabel={(a) => a}
          value=""
          placeholder="Author"
          onSelectExisting={pick}
          onCreateNew={pick}
        />
      </label>
    </BottomSheet>
  );
}
