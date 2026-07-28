import type { AuthorGroup, ChangeDto } from '../types/scan';

function pluralize(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? '' : 's'}`;
}

function findBook(tree: AuthorGroup[], bookId: number) {
  for (const a of tree) {
    const book = a.books.find((b) => b.bookId === bookId);
    if (book) return { author: a, book };
  }
  return null;
}

// Whether every file currently in `bookId` is included in `fileIds` - if so,
// applying the change will leave that book with zero files (auto-deleted).
function bookBecomesEmpty(tree: AuthorGroup[], bookId: number, fileIds: number[]): boolean {
  const found = findBook(tree, bookId);
  if (!found) return false;
  const moving = new Set(fileIds);
  return found.book.files.every((f) => moving.has(f.id));
}

// Produces human-readable dry-run lines describing what a batch of
// ChangeDto's will actually do, so the user can confirm before it's applied.
export function describeChanges(changes: ChangeDto[], tree: AuthorGroup[]): string[] {
  return changes.map((change) => describeOne(change, tree));
}

function describeOne(change: ChangeDto, tree: AuthorGroup[]): string {
  const fileCount = change.file_ids.length;

  switch (change.change_type) {
    case 'rename': {
      const parts: string[] = [];
      if (change.new_author) {
        const existing = tree.find((a) => a.author === change.new_author);
        if (existing) {
          const sourceAuthor = tree.find((a) =>
            a.books.some((b) => b.files.some((f) => change.file_ids.includes(f.id))),
          );
          const sourceLabel = sourceAuthor ? `'${sourceAuthor.author}'` : 'the selected files';
          parts.push(
            `Merge author ${sourceLabel} into existing author '${change.new_author}' (${pluralize(
              existing.books.length,
              'book',
            )}) — ${pluralize(fileCount, 'file')} affected`,
          );
        } else {
          parts.push(`Rename author to '${change.new_author}' — ${pluralize(fileCount, 'file')} affected`);
        }
      }
      if (change.new_series) {
        parts.push(`Set title/series to '${change.new_series}' for ${pluralize(fileCount, 'file')}`);
      }
      if (change.new_filetitle) {
        parts.push(`Rename file to '${change.new_filetitle}'`);
      }
      return parts.join('; ') || `Update ${pluralize(fileCount, 'file')}`;
    }

    case 'move-title': {
      return `Move ${pluralize(fileCount, 'file')} to author '${change.new_author ?? '?'}'`;
    }

    case 'merge-title': {
      const destBookId = change.new_book_id ?? -1;
      const sourceIds = change.current_book_ids ?? [];
      const sourceLabels = sourceIds
        .map((id) => findBook(tree, id)?.book.title)
        .filter((t): t is string => Boolean(t));
      const sourceText = sourceLabels.length > 0 ? sourceLabels.map((t) => `'${t}'`).join(', ') : 'the selected book(s)';

      if (destBookId < 0) {
        return `Merge ${sourceText} (${pluralize(fileCount, 'file')}) into new book '${
          change.new_series ?? '?'
        }' under author '${change.new_author ?? '?'}' — source book(s) will be deleted`;
      }
      const dest = findBook(tree, destBookId);
      const destLabel = dest ? `'${dest.book.title}'` : 'the destination book';
      return `Merge ${sourceText} (${pluralize(fileCount, 'file')}) into ${destLabel} — source book(s) will be deleted`;
    }

    case 'file-move': {
      const destBookId = change.new_book_id ?? -1;
      if (destBookId < 0) {
        return `Move ${pluralize(fileCount, 'file')} to new book '${change.new_series ?? '?'}' under author '${
          change.new_author ?? '?'
        }'`;
      }
      const dest = findBook(tree, destBookId);
      const destLabel = dest ? `'${dest.book.title}' (${dest.author.author})` : 'the destination book';

      // Note any source book(s) that will be emptied and auto-deleted by this move.
      const sourceBookIds = new Set<number>();
      for (const a of tree) {
        for (const b of a.books) {
          if (b.files.some((f) => change.file_ids.includes(f.id))) sourceBookIds.add(b.bookId);
        }
      }
      const emptied = Array.from(sourceBookIds).filter(
        (id) => id !== destBookId && bookBecomesEmpty(tree, id, change.file_ids),
      );
      const emptiedNote =
        emptied.length > 0
          ? ` — ${pluralize(emptied.length, 'source book')} will become empty and be deleted`
          : '';
      return `Move ${pluralize(fileCount, 'file')} to ${destLabel}${emptiedNote}`;
    }

    default:
      return `Apply ${pluralize(fileCount, 'file')} change`;
  }
}
