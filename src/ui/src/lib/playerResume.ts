import type { FileMetadata, Progress } from '../types/book';

export interface ResumePoint {
  index: number;
  startSec: number;
}

// Web-only port of else-wer-app's data/lib/conflict-handling.ts getBookProgress —
// no local DB here, server progress is the only source of truth, so the
// local/server merge logic doesn't apply.
export function resolveResumePoint(files: FileMetadata[], progress: Progress[]): ResumePoint {
  if (files.length === 0) return { index: 0, startSec: 0 };

  const completeCount = progress.reduce((n, p) => (p.complete ? n + 1 : n), 0);
  if (progress.length === 0 || completeCount === files.length) {
    return { index: 0, startSec: 0 };
  }

  const mostRecent = progress.reduce((prev, curr) =>
    new Date(prev.updated_at) > new Date(curr.updated_at) ? prev : curr,
  );

  const index = files.findIndex((f) =>
    mostRecent.complete ? f.id > mostRecent.file_id : f.id >= mostRecent.file_id,
  );

  if (index === -1) return { index: 0, startSec: 0 };
  return { index, startSec: mostRecent.complete ? 0 : mostRecent.progress_ms / 1000 };
}

const LOCAL_PROGRESS_PREFIX = 'else-wer-progress-';

// Offline fallback for resolveResumePoint's input when the server is unreachable —
// a single locally-cached row is enough since resolveResumePoint only needs the
// most recent one, and saveProgress() only ever reports the currently-playing file.
export function saveLocalProgress(progress: Progress): void {
  try {
    localStorage.setItem(`${LOCAL_PROGRESS_PREFIX}${progress.book_id}`, JSON.stringify(progress));
  } catch {
    // storage unavailable/full — offline resume just degrades to start-of-book, not fatal
  }
}

export function getLocalProgress(bookId: number): Progress[] {
  try {
    const raw = localStorage.getItem(`${LOCAL_PROGRESS_PREFIX}${bookId}`);
    return raw ? [JSON.parse(raw) as Progress] : [];
  } catch {
    return [];
  }
}
