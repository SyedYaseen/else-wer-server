import { usePlayerStore } from '../store/player';
import { streamUrl, updateProgress } from '../api/progress';
import { coverUrl } from '../api/books';
import { getOfflineFileBlob } from '../offline/storage';
import type { AudioBookRow, FileMetadata } from '../types/book';
import { saveLocalProgress } from '../lib/playerResume';
import type { ResumePoint } from '../lib/playerResume';
import { consumeEndOfChapter } from './sleepTimer';

// Single module-level <audio> element, owned by this file only — persists
// across route navigation since it's never mounted inside a React tree.
export const audio = new Audio();
audio.preload = 'auto';

// Tracks a blob URL created for offline fallback playback so it can be
// revoked when a different file is loaded.
let offlineBlobUrl: string | null = null;

function revokeOfflineBlobUrl() {
  if (offlineBlobUrl) {
    URL.revokeObjectURL(offlineBlobUrl);
    offlineBlobUrl = null;
  }
}

const PERIODIC_SAVE_SEC = 10;
const COMPLETION_THRESHOLD_SEC = 3;
let lastSavedSec = -1;

function isNearEnd(currentSec: number, durationSec: number) {
  return durationSec > 0 && currentSec > durationSec - COMPLETION_THRESHOLD_SEC;
}

// Wall-clock (not currentTime-diff) tracking of listened time since the last save:
// immune to seeks by construction, since seeking changes currentTime without
// advancing the clock while playing. null baseline means "don't count the next tick"
// (fresh file/book load).
let lastSaveWallClockMs: number | null = null;
let accumulatedListenedMs = 0;

function trackListenedTime() {
  const now = performance.now();
  if (lastSaveWallClockMs !== null && !audio.paused) {
    // Cap each tick's contribution so one huge performance.now() jump (tab
    // suspend/debugger pause) can't inflate the accumulator; server also caps.
    accumulatedListenedMs += Math.min(now - lastSaveWallClockMs, PERIODIC_SAVE_SEC * 1000 * 2);
  }
  lastSaveWallClockMs = now;
}

export function saveProgressNow(): void {
  saveProgress(false);
}

// `atSec` defaults to the store's position; callers that already know where the
// element is headed pass it explicitly, because the store only catches up on the
// next 'timeupdate' (see applyPendingStart).
function saveProgress(complete: boolean, atSec?: number) {
  // Suppressed while a file is loading: assigning audio.src fires a 'pause' event
  // (and so a save) at a point where the store already points at the new file but
  // its currentTime has been reset to 0 by setIndex — which would persist
  // progress_ms: 0 for a book just opened part-way through, and push that 0 out to
  // every other device.
  if (loading) return;
  const { book, files, index, currentTime } = usePlayerStore.getState();
  const file = files[index];
  if (!book || !file) return;
  const listened_delta_ms = accumulatedListenedMs > 0 ? Math.round(accumulatedListenedMs) : undefined;
  accumulatedListenedMs = 0;
  // One shared timestamp for both writes, so the localStorage row and the server
  // row are last-write-wins comparable no matter which side is consulted later.
  // Without it the server row is stamped with the server's clock (CURRENT_TIMESTAMP
  // in upsert_progress) while the local row carries the client's, and
  // reconcileProgress then compares two different clocks — which is what stopped
  // progress syncing between devices.
  const updated_at = new Date().toISOString();
  const payload = {
    book_id: book.id,
    file_id: file.id,
    progress_ms: Math.floor((atSec ?? currentTime) * 1000),
    complete,
    updated_at,
    ...(listened_delta_ms !== undefined ? { listened_delta_ms } : {}),
  };
  updateProgress(payload).catch((e) => console.error('progress save failed', e));
  saveLocalProgress({ id: 0, user_id: 0, ...payload });
}

function updateMediaSessionMetadata() {
  if (!('mediaSession' in navigator)) return;
  const { book } = usePlayerStore.getState();
  if (!book) {
    navigator.mediaSession.metadata = null;
    return;
  }
  const cover = coverUrl(book.cover_art);
  navigator.mediaSession.metadata = new MediaMetadata({
    title: book.title,
    artist: book.author,
    album: book.title,
    artwork: cover ? [{ src: cover }] : [],
  });
}

// Guards against a stale async blob lookup applying after a newer loadFile call.
let loadSeq = 0;

// Set from the moment a new src is assigned until its start position has been
// applied — see saveProgress. The start position is held alongside it because
// WebKit drops a currentTime set while readyState is HAVE_NOTHING, so it has to
// be re-applied once metadata arrives.
let loading = false;
let pendingStartSec = 0;

// iOS WebKit silently drops `audio.currentTime = n` when it is assigned right
// after `audio.src` — readyState is still HAVE_NOTHING there, so per spec the
// value should become the element's "default playback start position", and
// WebKit does not apply it. Measured on iOS Chrome resuming file 1309 at 397s:
// the element requested `bytes=0-` and played from the chapter start while the
// store still held 397s. That is exactly the "seeker correct, audio from the
// chapter start" symptom. Desktop and Android honour the early assignment,
// which is why only iOS is affected.
//
// Two independent belts, so resuming does not depend on WebKit honouring either
// one:
//   1. a media-fragment URI (`#t=`), which WebKit applies while parsing the
//      source URL rather than through the JS setter;
//   2. a one-shot re-seek on `loadedmetadata`, by which point readyState is
//      >= HAVE_METADATA and a seek is honoured.
// A start of 0 (chapter advance) adds neither, so that path is unchanged.
function withStartFragment(url: string, startSec: number): string {
  return startSec > 0 ? `${url}#t=${startSec.toFixed(3)}` : url;
}

// Re-applies the requested start position once the element can actually seek,
// then records the book/file as most-recently-played at that position. Driven by
// the single module-level 'loadedmetadata' listener below rather than a per-load
// one: a load that is replaced before metadata arrives is simply superseded when
// the next loadFile re-arms `loading`, so nothing is left attached or stuck on.
function applyPendingStart() {
  if (!loading) return;
  if (Math.abs(audio.currentTime - pendingStartSec) > 1) {
    audio.currentTime = pendingStartSec;
  }
  loading = false;
  // Deliberately reports pendingStartSec rather than reading audio.currentTime
  // back. A seek is applied asynchronously — while the element is still buffering
  // its first bytes currentTime can still read 0 for a moment after the
  // assignment above, and saving that reading is what put books back at their
  // start: the 0 reached the server, and the next load resumed from it.
  lastSavedSec = Math.floor(pendingStartSec);
  saveProgress(false, pendingStartSec);
}

function loadFile(index: number, startSec: number, autoplay: boolean) {
  const { files } = usePlayerStore.getState();
  const file = files[index];
  if (!file) return;
  lastSavedSec = -1;
  lastSaveWallClockMs = null;
  accumulatedListenedMs = 0;
  usePlayerStore.getState().setIndex(index);
  loading = true;
  pendingStartSec = startSec;
  const seq = ++loadSeq;
  // Prefer a downloaded local copy outright instead of waiting for a network
  // error: iOS Safari fires the <audio> error event late or not at all for an
  // unreachable stream URL, which blocked offline playback of downloaded books.
  getOfflineFileBlob(file.id)
    .catch(() => null)
    .then((blob) => {
      if (seq !== loadSeq) return;
      revokeOfflineBlobUrl();
      if (blob) {
        offlineBlobUrl = URL.createObjectURL(blob);
        audio.src = withStartFragment(offlineBlobUrl, startSec);
      } else {
        audio.src = withStartFragment(streamUrl(file.id), startSec);
      }
      // Honoured by Chrome as the default playback start position; WebKit ignores
      // it this early, which is why the source also carries a `#t=` fragment and
      // 'loadedmetadata' re-applies it (see applyPendingStart).
      audio.currentTime = startSec;
      audio.playbackRate = usePlayerStore.getState().rate;
      if (autoplay) audio.play().catch((e) => console.error('play failed', e));
      updateMediaSessionMetadata();
    });
}

// Server-unreachable fallback: on a network/stream error, fall back to a
// locally downloaded copy of the current file if one exists, preserving
// playback position instead of relying on navigator.onLine (unreliable for
// a self-hosted server that may be down while Wi-Fi still reports online).
async function tryOfflineFallback() {
  const { book, files, index, currentTime, playing } = usePlayerStore.getState();
  const file = files[index];
  if (!book || !file) return;
  const blob = await getOfflineFileBlob(file.id);
  if (!blob) return;
  // If the stream died before its start position was ever applied, that pending
  // position — not the element's 0 — is what this fallback has to resume from.
  const resumeSec = loading ? pendingStartSec : currentTime;
  revokeOfflineBlobUrl();
  offlineBlobUrl = URL.createObjectURL(blob);
  audio.src = withStartFragment(offlineBlobUrl, resumeSec);
  loading = true;
  pendingStartSec = resumeSec;
  audio.currentTime = resumeSec;
  if (playing) audio.play().catch((e) => console.error('play failed', e));
}

export function loadBook(book: AudioBookRow, files: FileMetadata[], resume: ResumePoint) {
  const prev = usePlayerStore.getState();
  if (prev.book && prev.book.id !== book.id && prev.files.length > 0) {
    saveProgress(false);
  }
  usePlayerStore.getState().loadBook(book, files);
  loadFile(resume.index, resume.startSec, true);
}

export function togglePlay() {
  if (audio.paused) audio.play().catch((e) => console.error('play failed', e));
  else audio.pause();
}

export function seek(sec: number) {
  const wasPlaying = !audio.paused;
  // Seeking a file that hasn't loaded yet redirects where it will start, rather
  // than being overwritten by the pending start position a moment later.
  if (loading) pendingStartSec = sec;
  audio.currentTime = sec;
  if (wasPlaying) audio.play().catch((e) => console.error('play failed', e));
}

export function skip(deltaSec: number) {
  const d = audio.duration || 0;
  const target = audio.currentTime + deltaSec;
  const sec = d > 0 ? Math.min(Math.max(target, 0), d) : Math.max(target, 0);
  if (loading) pendingStartSec = sec;
  audio.currentTime = sec;
}

export function setRate(rate: number) {
  audio.playbackRate = rate;
  usePlayerStore.getState().setRate(rate);
}

export function switchToFile(index: number) {
  const { files, index: currentIndex } = usePlayerStore.getState();
  if (index === currentIndex || !files[index]) return;
  saveProgress(false);
  loadFile(index, 0, true);
}

function advance() {
  const { files, index } = usePlayerStore.getState();
  const nextIndex = index + 1;
  if (nextIndex < files.length) {
    loadFile(nextIndex, 0, true);
  } else {
    audio.pause();
    updateMediaSessionMetadata();
    usePlayerStore.getState().clear();
  }
}

audio.addEventListener('timeupdate', () => {
  trackListenedTime();
  usePlayerStore.getState().setTime(audio.currentTime, audio.duration || 0);
  const sec = Math.floor(audio.currentTime);
  if (sec > 0 && sec % PERIODIC_SAVE_SEC === 0 && sec !== lastSavedSec) {
    lastSavedSec = sec;
    saveProgress(isNearEnd(audio.currentTime, audio.duration || 0));
  }
});
audio.addEventListener('loadedmetadata', () => {
  applyPendingStart();
  usePlayerStore.getState().setTime(audio.currentTime, audio.duration || 0);
});
audio.addEventListener('play', () => usePlayerStore.getState().setPlaying(true));
audio.addEventListener('pause', () => {
  usePlayerStore.getState().setPlaying(false);
  saveProgress(isNearEnd(audio.currentTime, audio.duration || 0));
});
audio.addEventListener('error', () => {
  // startsWith, not ===: the source now carries a `#t=` media fragment.
  if (offlineBlobUrl && audio.currentSrc.startsWith(offlineBlobUrl)) return;
  tryOfflineFallback().catch((e) => console.error('offline fallback failed', e));
});
audio.addEventListener('ended', () => {
  saveProgress(true);
  if (consumeEndOfChapter()) {
    audio.pause();
    return;
  }
  advance();
});

// Best-effort save when the tab is backgrounded/closed — mobile Safari can
// suspend the page without firing 'pause' first.
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden' && usePlayerStore.getState().book) {
    saveProgress(isNearEnd(audio.currentTime, audio.duration || 0));
  }
});

if ('mediaSession' in navigator) {
  navigator.mediaSession.setActionHandler('play', () => audio.play().catch(() => {}));
  navigator.mediaSession.setActionHandler('pause', () => audio.pause());
  navigator.mediaSession.setActionHandler('seekbackward', () => skip(-30));
  navigator.mediaSession.setActionHandler('seekforward', () => skip(30));
  navigator.mediaSession.setActionHandler('previoustrack', () => {
    const { index } = usePlayerStore.getState();
    if (index > 0) switchToFile(index - 1);
  });
  navigator.mediaSession.setActionHandler('nexttrack', () => advance());
}
