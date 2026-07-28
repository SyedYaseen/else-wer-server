import type { FileMetadata, Progress } from '../types/book';
import { updateProgress, getBookProgress } from '../api/progress';
import { useNetworkStatusStore } from '../store/networkStatus';

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

  // Resolve by position in the (possibly reordered) files array, not by comparing
  // numeric file ids — a manual chapter reorder can leave file.id ordering out of
  // sync with actual playback order.
  const mostRecentIndex = files.findIndex((f) => f.id === mostRecent.file_id);
  if (mostRecentIndex === -1) return { index: 0, startSec: 0 };
  const index = mostRecent.complete ? mostRecentIndex + 1 : mostRecentIndex;

  if (index >= files.length) return { index: 0, startSec: 0 };
  return { index, startSec: mostRecent.complete ? 0 : mostRecent.progress_ms / 1000 };
}

// Port of else-wer-app's data/lib/conflict-handling.ts local/server merge: compares
// the single cached local row (see saveLocalProgress) against the server's most recent
// row by updated_at, pushes the local row up if it's newer (covers progress made while
// offline, or a push that silently failed), and resolves from whichever side won.
export function reconcileProgress(
  files: FileMetadata[],
  serverProgress: Progress[],
  localProgress: Progress[],
): ResumePoint {
  if (localProgress.length === 0) return resolveResumePoint(files, serverProgress);

  const local = localProgress[0];
  const serverMostRecent =
    serverProgress.length > 0
      ? serverProgress.reduce((prev, curr) => (new Date(prev.updated_at) > new Date(curr.updated_at) ? prev : curr))
      : null;

  if (!serverMostRecent || new Date(local.updated_at) > new Date(serverMostRecent.updated_at)) {
    updateProgress({
      book_id: local.book_id,
      file_id: local.file_id,
      progress_ms: local.progress_ms,
      complete: local.complete,
      // Replay of an offline save: carry the original timestamp so the server's
      // LWW guard ranks it against other devices correctly, not as "now".
      updated_at: local.updated_at,
    }).catch((e) => console.error('progress push-back failed', e));
    return resolveResumePoint(files, [local]);
  }

  return resolveResumePoint(files, serverProgress);
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

// Pushes every locally-cached progress row (across all books, not just the one
// currently playing) up to the server if it's newer than what the server has —
// covers a device that recorded progress while the server was unreachable, so a
// different device on the LAN doesn't see stale progress once this one reconnects.
export async function flushLocalProgressToServer(): Promise<void> {
  const keys = Object.keys(localStorage).filter((k) => k.startsWith(LOCAL_PROGRESS_PREFIX));
  for (const key of keys) {
    const bookId = Number(key.slice(LOCAL_PROGRESS_PREFIX.length));
    if (!Number.isFinite(bookId)) continue;
    const [local] = getLocalProgress(bookId);
    if (!local) continue;
    try {
      const serverProgress = await getBookProgress(bookId);
      const serverMostRecent =
        serverProgress.length > 0
          ? serverProgress.reduce((prev, curr) =>
              new Date(prev.updated_at) > new Date(curr.updated_at) ? prev : curr,
            )
          : null;
      if (!serverMostRecent || new Date(local.updated_at) > new Date(serverMostRecent.updated_at)) {
        await updateProgress({
          book_id: local.book_id,
          file_id: local.file_id,
          progress_ms: local.progress_ms,
          complete: local.complete,
          // Replayed offline save — original timestamp, see reconcileProgress.
          updated_at: local.updated_at,
        });
      }
    } catch (e) {
      console.error('progress flush failed for book', bookId, e);
    }
  }
}

let flushInFlight = false;

// Wires flushLocalProgressToServer to fire as soon as the server becomes reachable
// again, instead of waiting for the user to happen to reopen the affected book.
export function startProgressSyncOnReconnect(): void {
  useNetworkStatusStore.subscribe((state, prevState) => {
    if (state.reachable && !prevState.reachable && !flushInFlight) {
      flushInFlight = true;
      flushLocalProgressToServer().finally(() => {
        flushInFlight = false;
      });
    }
  });
}
