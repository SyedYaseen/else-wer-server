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

function saveProgress(complete: boolean) {
  const { book, files, index, currentTime } = usePlayerStore.getState();
  const file = files[index];
  if (!book || !file) return;
  const listened_delta_ms = accumulatedListenedMs > 0 ? Math.round(accumulatedListenedMs) : undefined;
  accumulatedListenedMs = 0;
  const payload = {
    book_id: book.id,
    file_id: file.id,
    progress_ms: Math.floor(currentTime * 1000),
    complete,
    ...(listened_delta_ms !== undefined ? { listened_delta_ms } : {}),
  };
  updateProgress(payload).catch((e) => console.error('progress save failed', e));
  saveLocalProgress({ id: 0, user_id: 0, ...payload, updated_at: new Date().toISOString() });
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

function loadFile(index: number, startSec: number, autoplay: boolean) {
  const { files } = usePlayerStore.getState();
  const file = files[index];
  if (!file) return;
  lastSavedSec = -1;
  lastSaveWallClockMs = null;
  accumulatedListenedMs = 0;
  usePlayerStore.getState().setIndex(index);
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
        audio.src = offlineBlobUrl;
      } else {
        audio.src = streamUrl(file.id);
      }
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
  const resumeSec = currentTime;
  revokeOfflineBlobUrl();
  offlineBlobUrl = URL.createObjectURL(blob);
  audio.src = offlineBlobUrl;
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
  audio.currentTime = sec;
  if (wasPlaying) audio.play().catch((e) => console.error('play failed', e));
}

export function skip(deltaSec: number) {
  const d = audio.duration || 0;
  const target = audio.currentTime + deltaSec;
  audio.currentTime = d > 0 ? Math.min(Math.max(target, 0), d) : Math.max(target, 0);
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
  usePlayerStore.getState().setTime(audio.currentTime, audio.duration || 0);
});
audio.addEventListener('play', () => usePlayerStore.getState().setPlaying(true));
audio.addEventListener('pause', () => {
  usePlayerStore.getState().setPlaying(false);
  saveProgress(isNearEnd(audio.currentTime, audio.duration || 0));
});
audio.addEventListener('error', () => {
  if (offlineBlobUrl && audio.currentSrc === offlineBlobUrl) return;
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
