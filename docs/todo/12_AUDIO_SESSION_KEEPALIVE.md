# 12. Lock-screen pause kills the audio session

Status: **Pause-while-locked CLOSED 2026-08-16 — abandoned, keepalive removed. Do not reopen;
see "The ending".** Locked chapter *advance* was still broken on device after that removal and
is fixed separately — see "The removal did not restore locked advance (2026-09-01)" at the end.
Pause-while-locked is a PWA ceiling and moves to `else-wer-app`. The keepalive broke chapter
auto-advance on the way out, which is what finally ended it.
The rest of this doc is kept as the record of what was tried and refuted — read "The ending"
first, then treat everything above it as history, not as the current state of the code.
Supersedes the memory-pressure theory in [11_LOCKSCREEN_RESUME.md](11_LOCKSCREEN_RESUME.md);
11 is kept only for the save-path fixes (rounds 1–2, shipped and good) and as a record of what
was refuted.

## Problem

Pausing from the lock screen releases the iOS audio session, and the PWA never gets it back
while backgrounded.

Observed on device, reproducible:

1. Play, lock the phone, pause from the lock screen → **the app is terminated a few seconds
   later** and the lock-screen player disappears. Must reopen from the PWA icon.
2. On later cycles the widget stays on screen, but pressing play on it **switches the lock
   screen to Spotify**. No audio from else-wer.
3. During that phantom play the lock-screen clock advances, but the position is not kept —
   unlocking resumes from the pause point, as if the play never happened.
4. Playing *immediately* after locking (before any pause) works fine.

### Why this is the audio session and not memory

- **The Spotify handoff is decisive.** iOS routes a remote play command to the next app in
  the Now Playing stack. It only does that once else-wer no longer owns an audio session.
  That is iOS reporting the session is gone, not an inference.
- **The page usually survives.** On unlock the app resumed *without* the play button being
  pressed. A cold reload cannot autoplay — no user activation. So the element was left
  un-paused and stalled by the phantom play, and foregrounding let it fetch and continue.
- **The advancing lock-screen clock is a phantom.** `setPositionState()` is called nowhere
  in the codebase, so iOS extrapolates the clock from the last published position and rate.
  A moving clock is not evidence that anything is playing. This misled rounds 3 and 4.
- Symptom 4 is the control: the session is still active there, and everything works.

Rounds 3–4 (`preload='metadata'`, the server-side `RANGE_CHUNK_BYTES` clamp) were aimed at
jetsam from over-buffering. That cause does not exist — the ~42 MB working set is Chromium's
own buffer cap and was unchanged by the clamp. Neither change is a fix.

## Approach (round 5 — superseded by "Round 6" below; kept for the reasoning)

Keep the audio session alive across a pause so iOS neither jettisons the process nor hands
the Now Playing slot away.

### `src/ui/src/player/engine.ts`

1. **Silent keepalive element.** A second module-level `Audio()` looping a short silent WAV
   as an inline `data:` URI — no network fetch, works offline, and needs no new Workbox
   `globPatterns` entry (the current list has no audio extension).
   - Start it in the `pause` listener; stop it in the `play` listener.
   - Force `navigator.mediaSession.playbackState = 'paused'` *after* starting it, so the
     lock screen still reads as paused.
   - Release it after 30 minutes of continuous pause, so a long pause stops hogging the
     Now Playing slot from other apps. This bound is the main trade-off — see Open questions.

2. **`setPositionState()`** on `loadedmetadata`, on seek, and on the periodic `timeupdate`
   save. Absent today, which is why the phantom play looked real for two rounds. Guard it:
   `duration` is `Infinity` for these VBR MP3s, so feed it `effectiveDuration()`, and skip
   the call entirely when that is not finite.

### `src/api/audiobooks.rs`

3. **Correct the `RANGE_CHUNK_BYTES` doc comment.** It still asserts the disproven "reads
   until the pipe runs dry / 2.79 MB/s / ~350× real-time" mechanism. Keep the clamp itself —
   bounded responses are still kinder to a Pi Zero 2 than an 884 MB body — but it must not
   be recorded as a fix for this bug. The equivalent comment in `engine.ts:13-19` was
   already corrected.

### Cleanup, only once the fix is confirmed on device

4. Remove the `stream range` `tracing::info!` from `src/api/audiobooks.rs`.
5. Remove `?boot=1` from `src/ui/src/lib/healthPoll.ts` (`probe`/`tick`/`startHealthPolling`).

Keep both until then — they are the only vantage point into a standalone iOS PWA.

## What landed in round 5 (2026-08-16) — reverted, see "Round 6"

`src/ui/src/player/engine.ts`

- `silentWavUrl()` builds a 1 s, 8 kHz, 8-bit mono silent WAV at runtime and hands it to a
  module-level `keepalive` element with `loop = true`. Built rather than shipped: no network
  fetch, works offline, and needs no Workbox `globPatterns` entry. A `blob:` source is what
  the offline fallback already uses, so it's known to play on WebKit. (The plan said `data:`
  URI; a runtime blob is the same thing without 5 KB of base64 in the source.)
- `startKeepalive()` / `stopKeepalive()`, released after `KEEPALIVE_MAX_MS = 30 min`.
  `playbackState = 'paused'` is re-asserted *after* `play()` resolves, so the widget still
  reads as paused.
- Wiring is **background-only**: the `pause` listener starts it behind `document.hidden`,
  `visibilitychange → hidden` picks up the "paused in-app, then locked" case, `→ visible`
  and the `play` listener stop it. An in-app pause holds nothing — the next tap is a user
  gesture and can start audio on its own, so there is no reason to burn battery there.
- `primeKeepalive()` from `togglePlay()`, the one call site guaranteed to be a user gesture;
  iOS won't let the listeners start the element otherwise.
- `updatePositionState()` called from `loadedmetadata`, `seek`, `skip`, `setRate`, the
  periodic-save branch of `timeupdate`, both `play`/`pause` listeners, and `startKeepalive`.
- `navigator.audioSession.type = 'playback'` behind a feature check — a hint, not a fix.

`src/api/audiobooks.rs`: `RANGE_CHUNK_BYTES` comment rewritten (item 3).

Instrumentation (items 4–5) deliberately left in place.

### Research backing the approach

- The symptom is a known, unresolved WebKit/iOS PWA limitation — paused PWA audio stops
  functioning after ~30 s backgrounded, recoverable only by foregrounding the app.
  <https://developer.apple.com/forums/thread/762582>. Android is unaffected. Nobody in that
  thread has a workaround.
- A looping silent `<audio>` is the established community mitigation on iOS: HTML5 media
  elements keep running with the screen locked, while Web Audio and WebRTC are suspended.
  There is no web API to hold a session directly — `AVAudioSession` is native-only.
- So this is a mitigation with a real chance of failing, not a guaranteed fix. See the
  failure criterion below.

## Round 6 (2026-08-16) — the two-element keepalive was wrong

Device result for the round 5 build: **worse than before.** Locked while playing, the widget
drew a **Play** button over audio that was playing; pressing it made the widget vanish while
audio carried on; the Bluetooth button did nothing at all. Lock-screen pause, which used to
work, was gone.

Three defects, all in the round 5 code:

1. **iOS binds Now Playing to one media element per page.** `primeKeepalive()` ran from
   `togglePlay()` on the first tap, played the silent element and paused it, so the platform's
   session state became *that* element's — paused — while the book played.
   `mediaSession.playbackState` is advisory and does not override it. Same conflict reported at
   <https://developer.apple.com/forums/thread/109723> ("sometimes it shows the progress bar of
   one player and sometimes the controls for the other").
2. **A remote command that changes nothing makes iOS retire the widget.** The `play` handler
   called `resumeAudio()`, which early-returns on `!audio.paused`, so the command was swallowed,
   no state transition followed, and iOS dropped the Now Playing entry. Bluetooth used the same
   path, hence the dead button.
3. **The keepalive was never armed.** `stopKeepalive()` in the `play` listener called
   `keepalive.pause()` while the priming `play()` promise was still pending; that rejects with
   `AbortError`, the `.catch` reset `keepalivePrimed = false`, and no later `play()` had user
   activation. It only ever did harm.

### Why the replacement has to be single-element

A timer-based heartbeat cannot work — a paused, backgrounded PWA has its timers frozen within
seconds, which is the bug itself. Something must play *continuously*. Continuous silence on a
second element takes the Now Playing slot (defect 1). So the only coherent design is to swap
the **existing** element's `src` to the silent loop for the duration of a background pause.
Ownership never moves and the widget keeps describing the book.

De-risking evidence: `advance()` already assigns a new `src` and autoplays at chapter
boundaries while the phone is locked, and that works. The restore path is the same operation.

### What landed in round 6 (`src/ui/src/player/engine.ts`)

- Second element, `primeKeepalive`, `keepalivePrimed` deleted. `silentWavUrl()` kept, its
  result hoisted to `SILENT_SRC`.
- `startKeepalive()` captures `keepaliveHeld = { src: audio.currentSrc, sec: audio.currentTime }`,
  then puts `SILENT_SRC` on `audio` with `loop = true` and plays it, re-asserting
  `playbackState = 'paused'` after the promise resolves. 30-minute release timer unchanged.
- `stopKeepalive(resume)` restores **synchronously** through the existing
  `loading`/`pendingStartSec`/`applyPendingStart()` machinery, so a remote command's user
  activation survives and `loadedmetadata` re-seeks as it does for any load. `clearKeepalive()`
  stands it down without touching the element, for `loadFile()`.
- `resumeAudio()` routes through `stopKeepalive(true)`, so lock screen, headphones and the
  in-app button share one path. Checked *before* `audio.paused`, since during a keepalive the
  element is playing.
- Both `mediaSession` handlers now re-publish `playbackState` and position unconditionally,
  even for a no-op command (defect 2).
- Guards so nothing reads the silent clip as the book: `saveProgress()` substitutes
  `keepaliveHeld.sec` (without this the `pagehide` save writes `progress_ms ≈ 300` and
  replicates it to every device), `effectiveDuration()` ignores `audio.duration` (the clip's
  1.0 is finite and would win), and the `timeupdate` / `loadedmetadata` / `ended` / `error`
  listeners early-return. `updatePositionState()` early-returns and is now `try`-wrapped.

## Open questions

- **Does the restore succeed while backgrounded?** The gamble in round 6. Reassigning the
  book's `src` and seeking needs the network and a media load serviced with the screen locked.
  Chapter advance already does exactly this and works, and the silent loop keeps the page
  awake for it — but it is not proven for a mid-file seek. Verification step 4 is this test.
- **Does the keepalive survive iOS's jetsam?** Holding the session should stop the
  termination, still the least evidenced part. Covered by the boot-marker check in step 4.
- **Release window: 30 minutes** (decided; covers a pocket pause without holding Now Playing
  overnight). Revisit if battery cost is noticeable or if pauses regularly outlast it.
- The metadata question from round 5 is moot — with one element there is nothing to steal
  from. So is the priming question: nothing needs priming now, because the element the
  keepalive uses is the one the user already started by hand.

## Verification

Note wall-clock times for each step; the `boot=1` marker and `stream range` log are still
deployed and answer this from the server side.

The widget fix and the keepalive are independently testable in one build. Steps 1–3 exercise
only the handler fix; the keepalive is not involved until step 4.

1. Play, lock. The widget must show a **Pause** button. Press it: audio stops, the button
   becomes Play, the widget stays on screen.
2. Press Play on the widget. **Must not switch to Spotify**, audio starts, position continuous.
3. Same cycle on the Bluetooth headphone button with the phone locked in a pocket.
4. Pause from the lock screen, wait 60 s, press Play. Must resume from else-wer. Then open
   the app: **no new `boot=1`**, and the position must match where you paused, not 0.
5. Repeat step 4 with a ~5 minute pause, then unlock and check the in-app position is intact —
   this is the check that the guards against the silent clip held.
6. Confirm the lock-screen clock tracks real playback rather than free-running.
7. Pause for >30 min, confirm the session is released and another app can take Now Playing.

**Failure criterion.** Steps 1–3 failing means the round 5 removal was incomplete — fix that.
Steps 1–3 passing but 4 failing (fresh `boot=1`, or the resume produces silence) means a
backgrounded iOS PWA cannot hold an audio session across a pause even when it owns the
element. Record that here as the ceiling and move pause-while-locked to `else-wer-app`.
**No round 7.**

Corrected boot counter — the old form double-counts, `src/main.rs` logs both `Request Start`
and `Request end` with the URI on each:

```sh
ssh pi "journalctl -u else-wer.service --since '20 min ago' \
  | grep 'boot=1' | grep -c 'Request Start'"
```

Deploy is a full `make deploy` from `piconfig/` if the Rust comment change goes with it,
otherwise `make deploy-pwa`. Read the build stamp under Library before trusting any result.

## The ending (2026-08-16) — keepalive removed, investigation closed

The verification above was never reached. The keepalive was removed first, because it had
broken a feature that used to work: **with the phone locked, a chapter ending no longer started
the next one.** In-app advance was unaffected — that asymmetry is the whole diagnosis, since
`document.hidden` gated exactly one code path.

Mechanism, and it applies to *any* version of this idea:

1. Reaching the end of a file fires `pause` **before** `ended` (HTML spec). The `pause`
   listener started the silence.
2. The silence took the Now Playing slot — round 6's defect 1, which was known and written
   down here, but only reasoned about for a user-initiated pause.
3. `ended` → `advance()` → `loadFile()` → `stopKeepalive()` left the page with nothing
   playing, and the new file's `play()` — backgrounded, no user activation, session just
   dropped — never started.

Note what this does to the design's own de-risking argument. Round 6 justified swapping `src`
on the live element with: "`advance()` already assigns a new `src` and autoplays at chapter
boundaries while the phone is locked, and that works." The keepalive had already broken that
premise by the time it was written. A mitigation that runs on `pause` cannot be reasoned about
without accounting for end-of-media firing `pause`.

Per the failure criterion above: **no round 7.** Pause-while-locked is the ceiling for a
standalone iOS PWA. It moves to `else-wer-app`, where `AVAudioSession` exists.

### What was removed

`src/ui/src/player/engine.ts`: `silentWavUrl()`, the `keepalive` element, `KEEPALIVE_MAX_MS`,
`startKeepalive()` / `stopKeepalive()`, and all five call sites (`loadFile`, the `playing`
listener, the `pause` listener, both `visibilitychange` branches). The `pause` and
`visibilitychange` listeners are back to their pre-investigation shape. A comment on the
`pause` listener records why nothing is started there, so this isn't rediscovered.

### What was deliberately kept — do not "finish the revert"

These are independent of the keepalive and each fixes something real:

- `loading` / `pendingStartSec` / `applyPendingStart()` and `saveProgress(complete, atSec)` —
  the `progress_ms: 0` replication fix ([11_LOCKSCREEN_RESUME.md](11_LOCKSCREEN_RESUME.md)
  rounds 1–2).
- `effectiveDuration()` and the `Seeker.tsx` finite guards — `Infinity` duration on VBR MP3s.
- `updatePositionState()` — a real lock-screen clock instead of an extrapolated one.
- The `republish()` wrapper on the `play`/`pause` mediaSession handlers — round 6's defect 2;
  without it a no-op remote command retires the Now Playing widget mid-playback.
- `preload='metadata'` and the server's `RANGE_CHUNK_BYTES` clamp. Neither is a fix for this
  bug (rounds 3–4, refuted above) and neither caused the advance regression — both are
  file-level and would have broken foreground advance too. Kept on their own merit: bounded
  responses are kinder to a Pi Zero 2. `http-range-header` 0.4.2 clamps an end past EOF to
  `file_size - 1`, so the final `bytes=N-` chunk of a file is served normally, not 416'd.

`resumeAudio()` stays seek-free — the old 5 s rewind on resume is not coming back (decided).

### Instrumentation still deployed

The `stream range` `tracing::info!` in `src/api/audiobooks.rs` and `?boot=1` in
`src/ui/src/lib/healthPoll.ts` are still in. They are the only vantage point into a standalone
iOS PWA. Remove them once locked chapter advance is confirmed good on device — that check is
the last thing this doc is waiting on, after which the doc can be deleted.

### Verification for the removal

1. `cd src/ui && npm run build` + `npm run lint` — done, clean.
2. Desktop Chrome regression guard: seek to ~10 s before the end of a chapter and let it run
   out. The next file must load and play.
3. **Device, the actual fix.** `make deploy-pwa`, check the build stamp under Library, then
   play, lock the phone, and let a chapter end with the screen still locked. The next chapter
   must start on its own and the widget must switch to it. Confirm from the server that the
   boundary really crossed while locked, rather than foregrounding having fixed it:
   ```sh
   ssh pi "journalctl -u else-wer.service --since '20 min ago' | grep 'stream range'"
   ```
   A new `file_id` should appear at the boundary.
4. Lock-screen *resume* remains broken. That is the accepted ceiling, not a regression.

> Nothing is to be committed — review first.


## The removal did not restore locked advance (2026-09-01)

The removal's verification step 3 was finally run on device: the keepalive-removed build is
live, and with the phone locked, **audio still stopped at a chapter boundary and the next
chapter never started**. New signal from the same test: **foreground chapter skip is slow**.

That second observation refutes this doc's own reason for keeping `preload='metadata'` —
"neither caused the advance regression; both are file-level and would have broken foreground
advance too." Foreground advance *was* degraded; it just degraded to slow rather than to
broken, which is the same defect under a deadline the foreground doesn't have.

Two causes, both in the gap between one file ending and the next one playing:

1. **The `pause` listener told iOS the session was idle.** End-of-media fires `pause` before
   `ended`, and the listener published `mediaSession.playbackState = 'paused'` (from `e578e8a`)
   plus `updatePositionState()` at a position at the end of the file. Backgrounded, that is the
   page announcing playback has stopped — after which `advance()`'s `play()` runs with no user
   activation against a session iOS has retired. Same shape as the keepalive failure, through
   mediaSession state instead of silence: *anything* the `pause` listener does at end-of-media
   lands in that gap.
2. **`advance()` was slow and asynchronous.** `loadFile()` awaited `getOfflineFileBlob()` before
   assigning `src`, so `play()` never ran in the `ended` event's own task, and then
   `preload='metadata'` made WebKit fetch metadata before fetching to play. A backgrounded page
   with no audio output gets a short window; that spent it.

### What shipped (`src/ui/src/player/engine.ts`)

- The `pause` listener early-returns on `audio.ended` — no `playbackState`, no
  `updatePositionState()`, no save. The `ended` listener saves with `complete: true` a moment
  later, so nothing is lost. The comment there records why, next to the existing one about
  starting nothing.
- Because the `pause` listener is now silent at end-of-media, the two paths that legitimately
  stop there publish the widget state themselves: `advance()`'s end-of-book branch clears the
  metadata and sets `playbackState = 'none'`, and the sleep timer's end-of-chapter branch in
  the `ended` listener sets `'paused'`. Without this the widget is left reading 'playing' over
  a book that has finished.
- `nextBlob` prefetch cache + `takePrefetched()`: the next file's offline-blob lookup resolves
  ahead of time (kicked off from `applyPendingStart()` once a load settles, topped up near the
  end from `timeupdate`), so `loadFile()` has a **synchronous** path that assigns `src` and
  calls `play()` inline. Covers a forward skip as well as a natural boundary.
- `warmNextStream()`: one discarded `Range: bytes=0-65535` fetch of the next file ~60 s before
  the boundary, once per file, skipped when a downloaded copy exists. Makes the Pi open the
  file and warm the page cache off the critical path.
- `applySource()` factors the source-assignment body shared by both paths and raises
  `audio.preload` to `'auto'` when the load will autoplay. `'metadata'` stays the idle default.
- `updateMediaSessionMetadata()` now publishes the chapter's `file_name` as the title (book as
  the album). Book-level metadata every time made the lock screen read identically either side
  of a boundary, so neither a crossing nor a failure to cross was visible.

Not touched: no keepalive in any form, no resume rewind, and lock-screen resume after a pause
remains the accepted ceiling.

### Still to confirm on device

`make deploy-pwa`, check the build stamp under Library, then:

1. Foreground chapter skip is quick again.
2. Play, lock, let a chapter end with the screen still locked: the next chapter starts on its
   own and the widget switches to its name.
3. The boundary really crossed while locked, not on foregrounding —
   `ssh pi "journalctl -u else-wer.service --since '20 min ago' | grep 'stream range'"` shows a
   new `file_id` at the boundary, and no fresh `boot=1`.
4. After the locked boundary, unlock and confirm the position is in the new chapter and not 0.

The `stream range` log and `?boot=1` stay in until that passes; then they, and this doc, can go.
