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
