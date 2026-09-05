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
// 'metadata', not 'auto': the library holds files up to ~884 MB, and there's no reason
// to pull any of one before the user presses play. Note this does *not* bound buffering
// during playback — preload stops applying once play() is called, which is why it alone
// didn't fix the mid-playback reloads; that's capped server-side in stream_file's
// RANGE_CHUNK_BYTES. 'metadata' still fires loadedmetadata, so applyPendingStart() is
// unaffected. A load that is going to autoplay raises this to 'auto' — see applySource().
audio.preload = 'metadata';

// Publishes the real position to the lock screen. Without it iOS extrapolates the
// clock from the last state it saw, so the widget kept ticking over audio that had
// stopped — which made a phantom resume look like a working one for two rounds of
// this investigation. Guarded because audio.duration is Infinity for these VBR MP3s.
function updatePositionState() {
  if (!('mediaSession' in navigator) || !navigator.mediaSession.setPositionState) return;
  const duration = effectiveDuration();
  if (!Number.isFinite(duration) || duration <= 0) return;
  try {
    navigator.mediaSession.setPositionState({
      duration,
      position: Math.min(Math.max(audio.currentTime, 0), duration),
      playbackRate: audio.playbackRate,
    });
  } catch {
    // Throws on out-of-range input. Swallowed because most call sites are event
    // listeners with real work queued after them (see the 'pause' listener).
  }
}

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

// WebKit reports duration as Infinity (occasionally NaN) for a streamed VBR MP3
// with no Xing header, which makes the seeker unusable. Fall back to the duration
// recorded by the library scan (stored in ms) so the UI always has a finite total.
function effectiveDuration(): number {
  if (Number.isFinite(audio.duration) && audio.duration > 0) return audio.duration;
  const { files, index } = usePlayerStore.getState();
  const scanned = files[index]?.duration;
  return scanned && scanned > 0 ? scanned / 1000 : 0;
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

// `atSec` defaults to the element's position rather than the store's: the store's
// copy only catches up on the next 'timeupdate', so seek()/skip() — which save in
// the same tick as the seek — would otherwise persist the position from *before*
// the seek. Callers pass it explicitly when the element's own reading can't be
// trusted yet (see applyPendingStart).
function saveProgress(complete: boolean, atSec: number = audio.currentTime) {
  // Suppressed while a file is loading: assigning audio.src fires a 'pause' event
  // (and so a save) at a point where the store already points at the new file but
  // audio.currentTime is still 0 — which would persist progress_ms: 0 for a book
  // just opened part-way through, and push that 0 out to every other device.
  if (loading) return;
  const { book, files, index } = usePlayerStore.getState();
  const file = files[index];
  if (!book || !file) return;
  const currentTime = atSec;
  const listened_delta_ms = accumulatedListenedMs > 0 ? Math.round(accumulatedListenedMs) : undefined;
  accumulatedListenedMs = 0;
  // One shared timestamp for both writes, so the localStorage row and the server
  // row are last-write-wins comparable no matter which side is consulted later.
  const updated_at = new Date().toISOString();
  const payload = {
    book_id: book.id,
    file_id: file.id,
    progress_ms: Math.floor(currentTime * 1000),
    complete,
    updated_at,
    ...(listened_delta_ms !== undefined ? { listened_delta_ms } : {}),
  };
  updateProgress(payload).catch((e) => console.error('progress save failed', e));
  saveLocalProgress({ id: 0, user_id: 0, ...payload });
}

function updateMediaSessionMetadata() {
  if (!('mediaSession' in navigator)) return;
  const { book, files, index } = usePlayerStore.getState();
  if (!book) {
    navigator.mediaSession.metadata = null;
    return;
  }
  const cover = coverUrl(book.cover_art);
  // Chapter name as the title, book as the album. Publishing book-level metadata
  // for every file made the lock screen read identically either side of a chapter
  // boundary, so a crossing that did happen was invisible and one that didn't
  // looked the same.
  const chapter = files[index]?.file_name;
  navigator.mediaSession.metadata = new MediaMetadata({
    title: chapter || book.title,
    artist: book.author,
    album: book.title,
    artwork: cover ? [{ src: cover }] : [],
  });
}

// Guards against a stale async blob lookup applying after a newer loadFile call.
let loadSeq = 0;

// Set from the moment a new src is assigned until its start position has been
// applied — see saveProgress. The start position is held alongside it because
// WebKit drops a currentTime set while readyState is HAVE_NOTHING, so it has to be
// re-applied once metadata arrives.
let loading = false;
let pendingStartSec = 0;

// Applies the requested start position once the element can actually seek, then
// records the book/file as most-recently-played at that position.
function applyPendingStart() {
  if (!loading) return;
  if (Math.abs(audio.currentTime - pendingStartSec) > 1) {
    audio.currentTime = pendingStartSec;
  }
  loading = false;
  // Deliberately reports pendingStartSec rather than reading audio.currentTime
  // back. A seek is applied asynchronously — while the element is still buffering
  // its first bytes, currentTime can still read 0 for a moment after the
  // assignment above. Saving that reading is what put books back at their start:
  // the 0 reached the server, and the next load then resumed from it for real.
  lastSavedSec = Math.floor(pendingStartSec);
  saveProgress(false, pendingStartSec);
  // Resolve the next file's source now that this one is settled, so a forward
  // skip gets the synchronous path too, not only a natural chapter end.
  prefetchNext();
}

// Resolved offline-blob lookup for the *next* file, so a chapter boundary can put
// a source on the element without awaiting IndexedDB. `blob: null` is a real
// answer — no local copy, stream it — and is distinct from holding no answer.
let nextBlob: { fileId: number; blob: Blob | null } | null = null;
// The next file id whose stream has already been warmed, so the warm request is
// issued once per boundary rather than on every timeupdate near the end.
let warmedFileId: number | null = null;

const NEAR_END_PREFETCH_SEC = 60;

// undefined = no cached answer for this file; Blob | null = a cached answer.
function takePrefetched(fileId: number): Blob | null | undefined {
  if (!nextBlob || nextBlob.fileId !== fileId) return undefined;
  const { blob } = nextBlob;
  nextBlob = null;
  return blob;
}

function prefetchNext() {
  const { files, index } = usePlayerStore.getState();
  const next = files[index + 1];
  if (!next) {
    nextBlob = null;
    return;
  }
  if (nextBlob && nextBlob.fileId === next.id) return;
  nextBlob = null;
  const fileId = next.id;
  getOfflineFileBlob(fileId)
    .catch(() => null)
    .then((blob) => {
      // Switching books mid-lookup would otherwise cache an answer for a file
      // nothing is going to ask for, holding a reference to its blob for good.
      const cur = usePlayerStore.getState();
      if (cur.files[cur.index + 1]?.id !== fileId) return;
      nextBlob = { fileId, blob };
    });
}

// One ranged request for the head of the next file shortly before the boundary.
// The response is discarded: the point is to make the Pi open the file and warm
// the OS page cache (and spin the library drive up) off the critical path, so the
// element's own first request at the boundary is served from cache.
function warmNextStream() {
  const { files, index } = usePlayerStore.getState();
  const next = files[index + 1];
  if (!next || warmedFileId === next.id) return;
  // Wait for the prefetch's answer rather than racing it: a downloaded copy plays
  // from a blob URL and has no request to warm, so warming before the lookup
  // returns would fire a pointless request at every boundary of a book that is
  // fully downloaded — the case most likely to have no server to reach.
  if (!nextBlob || nextBlob.fileId !== next.id || nextBlob.blob) return;
  warmedFileId = next.id;
  fetch(streamUrl(next.id), { headers: { Range: 'bytes=0-65535' } }).catch(() => {});
}

// Puts a resolved source on the element. `blob` is the downloaded copy when one
// exists, null for a network stream; both callers below resolve it, one
// synchronously from the prefetch cache and one from a fresh lookup.
function applySource(file: FileMetadata, startSec: number, autoplay: boolean, blob: Blob | null) {
  revokeOfflineBlobUrl();
  if (blob) {
    offlineBlobUrl = URL.createObjectURL(blob);
    audio.src = offlineBlobUrl;
  } else {
    audio.src = streamUrl(file.id);
  }
  // 'auto' only when we are about to play anyway. preload governs pre-play()
  // fetching only, so this costs nothing here, and leaving it at 'metadata' made
  // WebKit fetch metadata and then re-fetch to play — the two-step that made
  // chapter changes slow, and slow is fatal at a boundary reached while locked.
  audio.preload = autoplay ? 'auto' : 'metadata';
  // Honoured by Chrome as the default playback start position; WebKit ignores
  // it this early, so 'loadedmetadata' re-applies it (see applyPendingStart).
  audio.currentTime = startSec;
  audio.playbackRate = usePlayerStore.getState().rate;
  if (autoplay) audio.play().catch((e) => console.error('play failed', e));
  updateMediaSessionMetadata();
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
  const prefetched = takePrefetched(file.id);
  if (prefetched !== undefined) {
    // The whole point of the cache: at a chapter boundary the page is often
    // backgrounded with nothing playing, and iOS grants it only a short window
    // before suspending it. Spending that window on an IndexedDB round-trip is
    // what leaves the next file's play() unserviced.
    applySource(file, startSec, autoplay, prefetched);
    return;
  }
  // Prefer a downloaded local copy outright instead of waiting for a network
  // error: iOS Safari fires the <audio> error event late or not at all for an
  // unreachable stream URL, which blocked offline playback of downloaded books.
  getOfflineFileBlob(file.id)
    .catch(() => null)
    .then((blob) => {
      if (seq !== loadSeq) return;
      applySource(file, startSec, autoplay, blob);
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
  audio.src = offlineBlobUrl;
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

// Shared resume path for every surface (in-app toggle, lock screen, Bluetooth).
//
// Deliberately does not seek before playing. Rewinding a few seconds on resume
// reads well in-app, but the same seek breaks a lock-screen resume outright: iOS
// won't service the re-buffer it forces until the page is foregrounded, so play()
// resolves and the lock-screen clock advances while the element produces silence.
// Keeping resume seek-free keeps every surface behaving the same.
function resumeAudio() {
  if (!audio.paused) return;
  audio.play().catch((e) => console.error('play failed', e));
}

export function togglePlay() {
  if (audio.paused) resumeAudio();
  else audio.pause();
}

export function seek(sec: number) {
  const wasPlaying = !audio.paused;
  // Seeking a file that hasn't loaded yet redirects where it will start, rather
  // than being overwritten by the pending start position a moment later.
  if (loading) pendingStartSec = sec;
  audio.currentTime = sec;
  if (wasPlaying) audio.play().catch((e) => console.error('play failed', e));
  lastSavedSec = Math.floor(sec);
  saveProgress(isNearEnd(sec, effectiveDuration()), sec);
  updatePositionState();
}

export function skip(deltaSec: number) {
  const d = effectiveDuration();
  const target = audio.currentTime + deltaSec;
  const sec = d > 0 ? Math.min(Math.max(target, 0), d) : Math.max(target, 0);
  if (loading) pendingStartSec = sec;
  audio.currentTime = sec;
  lastSavedSec = Math.floor(sec);
  saveProgress(isNearEnd(sec, d), sec);
  updatePositionState();
}

export function setRate(rate: number) {
  audio.playbackRate = rate;
  usePlayerStore.getState().setRate(rate);
  updatePositionState();
}

export function switchToFile(index: number) {
  const { files, index: currentIndex } = usePlayerStore.getState();
  if (index === currentIndex || !files[index]) return;
  saveProgress(false);
  // The new file registers itself as most-recently-played from applyPendingStart,
  // once it can report a position — a save here would only be suppressed by the
  // loading guard, and before that guard existed it wrote progress_ms: 0.
  loadFile(index, 0, true);
}

function advance() {
  const { files, index } = usePlayerStore.getState();
  const nextIndex = index + 1;
  if (nextIndex < files.length) {
    loadFile(nextIndex, 0, true);
  } else {
    audio.pause();
    usePlayerStore.getState().clear();
    // The 'pause' listener publishes nothing at end-of-media (see there), so the
    // end of the last file has to retire the widget itself — otherwise it is left
    // reading 'playing' over a book that has finished. Metadata is cleared after
    // clear(), so this publishes the empty state rather than the finished book.
    updateMediaSessionMetadata();
    if ('mediaSession' in navigator) navigator.mediaSession.playbackState = 'none';
  }
}

audio.addEventListener('timeupdate', () => {
  trackListenedTime();
  usePlayerStore.getState().setTime(audio.currentTime, effectiveDuration());
  const remaining = effectiveDuration() - audio.currentTime;
  if (remaining > 0 && remaining < NEAR_END_PREFETCH_SEC) {
    prefetchNext();
    warmNextStream();
  }
  const sec = Math.floor(audio.currentTime);
  if (sec > 0 && sec % PERIODIC_SAVE_SEC === 0 && sec !== lastSavedSec) {
    lastSavedSec = sec;
    saveProgress(isNearEnd(audio.currentTime, effectiveDuration()));
    updatePositionState();
  }
});
audio.addEventListener('loadedmetadata', () => {
  applyPendingStart();
  usePlayerStore.getState().setTime(audio.currentTime, effectiveDuration());
  updatePositionState();
});
audio.addEventListener('play', () => {
  usePlayerStore.getState().setPlaying(true);
  if ('mediaSession' in navigator) navigator.mediaSession.playbackState = 'playing';
  updatePositionState();
});
// Nothing is started here to hold the iOS audio session across a background pause.
// Playing silence from this listener broke chapter auto-advance: reaching the end of a
// file fires 'pause' *before* 'ended', so the silence took the Now Playing slot in the
// gap and the next file's play() never started. See docs/todo/12_AUDIO_SESSION_KEEPALIVE.md —
// pause-while-locked is a PWA ceiling and belongs to else-wer-app.
audio.addEventListener('pause', () => {
  usePlayerStore.getState().setPlaying(false);
  // End-of-media fires 'pause' *before* 'ended' (HTML spec sets the ended flag
  // first, which is what this tests), so this listener runs in the gap before
  // advance() loads the next file. Publishing 'paused' and a position at the end
  // of the file there tells iOS playback has stopped; a backgrounded page that
  // has just said so does not get to start the next file, because advance()'s
  // play() then runs with no user activation against a session iOS has retired.
  // So nothing is published in that gap. 'ended' saves progress with
  // complete: true immediately after, so the skipped save loses nothing.
  if (audio.ended) return;
  if ('mediaSession' in navigator) navigator.mediaSession.playbackState = 'paused';
  saveProgress(isNearEnd(audio.currentTime, effectiveDuration()));
  updatePositionState();
});
audio.addEventListener('error', () => {
  if (offlineBlobUrl && audio.currentSrc === offlineBlobUrl) return;
  // A source that never loads would otherwise leave saves suppressed for good.
  loading = false;
  tryOfflineFallback().catch((e) => console.error('offline fallback failed', e));
});
audio.addEventListener('ended', () => {
  saveProgress(true);
  if (consumeEndOfChapter()) {
    audio.pause();
    // Same reason as the end-of-book branch in advance(): the 'pause' listener
    // stays silent at end-of-media, so a sleep timer stopping here has to publish
    // the paused state itself.
    if ('mediaSession' in navigator) navigator.mediaSession.playbackState = 'paused';
    updatePositionState();
    return;
  }
  advance();
});

// Best-effort save when the tab is backgrounded/closed — mobile Safari can
// suspend the page without firing 'pause' first.
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden' && usePlayerStore.getState().book) {
    saveProgress(isNearEnd(audio.currentTime, effectiveDuration()));
  }
});

// 'pagehide' as well as 'visibilitychange': iOS discards a backgrounded standalone
// PWA and reloads it on the next open, and that teardown does not reliably fire
// visibilitychange first. Without this, everything since the last periodic save is
// lost on every lock/unlock, and the reloaded app resumes from the older position.
window.addEventListener('pagehide', () => {
  if (usePlayerStore.getState().book) {
    saveProgress(isNearEnd(audio.currentTime, effectiveDuration()));
  }
});

// Safari's Audio Session API (still being standardised): declares this page as
// playback audio rather than an incidental sound, which is what a native app gets
// from AVAudioSession. Feature-detected and unavailable almost everywhere — a hint,
// not the fix.
const nav = navigator as Navigator & { audioSession?: { type: string } };
if (nav.audioSession) nav.audioSession.type = 'playback';

if ('mediaSession' in navigator) {
  // Both handlers re-publish state unconditionally, even when the command was a
  // no-op. A remote command that produces no state transition makes iOS retire the
  // Now Playing entry outright — the widget vanishes mid-playback and the headphone
  // button goes dead with it.
  const republish = () => {
    navigator.mediaSession.playbackState = audio.paused ? 'paused' : 'playing';
    updatePositionState();
  };
  navigator.mediaSession.setActionHandler('play', () => {
    resumeAudio();
    republish();
  });
  navigator.mediaSession.setActionHandler('pause', () => {
    audio.pause();
    republish();
  });
  navigator.mediaSession.setActionHandler('seekbackward', () => skip(-usePlayerStore.getState().rewindSec));
  navigator.mediaSession.setActionHandler('seekforward', () => skip(usePlayerStore.getState().ffwdSec));
  navigator.mediaSession.setActionHandler('previoustrack', () => {
    const { index } = usePlayerStore.getState();
    if (index > 0) switchToFile(index - 1);
  });
  navigator.mediaSession.setActionHandler('nexttrack', () => advance());
}
