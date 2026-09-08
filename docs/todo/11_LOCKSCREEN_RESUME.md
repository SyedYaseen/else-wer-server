# 11. Player: lock-screen resume, progress clobbering, dead seeker

> **Superseded for the lock-screen symptom by [12_AUDIO_SESSION_KEEPALIVE.md](12_AUDIO_SESSION_KEEPALIVE.md).**
> Round 5 established the cause is the iOS audio session being released on pause, not memory
> pressure. Rounds 3–4 below are refuted; rounds 1–2 shipped real save-path fixes and stand.
> Start at doc 12.
>
> **The "seeker right, plays from chapter start" symptom is solved — see Round 6 (2026-09-08),
> immediately below.** It was WebKit dropping a `currentTime` assigned before metadata, found
> by logging the `Range` header rather than by inference, and it was unrelated to everything
> rounds 3–5 pursued. Round 6 also corrects this document's "these MP3s are VBR with no Xing
> header" premise, which is false for the book every round was tested against.

Diagnosed 2026-08-15, revised 2026-08-16 (round 4). Rounds 1 and 2 fixed real defects in
the save path — the `progress` table is clean as a result — but neither addressed the
actual cause, and round 2's service-worker theory is **refuted by the Pi's own logs**.
Rounds 3–4 target the page being killed and reloaded mid-playback: round 3 established
that as the mechanism and picked the wrong lever, round 4 measured the cause and moved
the fix to the server. **Start at "Round 4"**; round 3 below is kept for the evidence.

## Todo
- [x] `resumeAudio()`: drop the seek before `play()`
- [x] `saveProgress()`: read the position from the element, not the store
- [x] Suppress saves while a source is loading
- [x] Re-apply the start position on `loadedmetadata`
- [x] Seeker: finite-duration guard, scanned-duration fallback, no latched drag
- [x] `applyPendingStart()` must not save a seek it can't read back yet (round-2 fix)
- [x] Save on `pagehide` — iOS discards a backgrounded PWA without `visibilitychange`
- [x] SPA deep links return 200, not 404 (`src/main.rs`)
- [x] `?boot=1` marker on the first health probe, so app starts are countable server-side
- [x] Build stamp on the Library page, so "which build is live" is never a question again
- [x] `audio.preload` `'auto'` → `'metadata'` (round-3 fix — **did not work**, see Round 4)
- [x] Measured the read-ahead rate: 2.79 MB/s on a 64 kbps stream, ~350× real-time
- [x] Clamp Range responses server-side to 4 MiB (`RANGE_CHUNK_BYTES`) — works, but does
      **not** reduce read-ahead; the ~42 MB working set is unchanged. Not the fix.
- [x] Found the counter double-counts: `grep -c 'boot=1'` matches Start *and* end lines
- [ ] **Re-run the gate over a real 15-minute listen** — the 20-min window held 45 s of
      playback, so nothing was measured
- [ ] If reloads persist, stop — see "If the reloads persist"
- [ ] Remove instrumentation (`stream range` log, `?boot=1`) once confirmed
- [x] **Round 6:** measured the Range header at the moment of failure — cold load asks
      `bytes=0-` and never seeks (see Round 6)
- [x] **Round 6:** `#t=` media-fragment start position — fixes the seek symptom on iOS,
      confirmed on device
- [x] **Round 6:** restored the shared `updated_at`, the `loading` guard and `clamp_range`
      that the baseline reset had dropped
- [ ] **Round 6:** restore the other seven fixes the reset removed — `effectiveDuration`,
      `updatePositionState`, chapter lock-screen title, `preload='metadata'`, prefetch/warm,
      `seek`/`skip` saves, `pagehide` save, end-of-book `playbackState`
- [ ] **Round 6:** confirm cross-device progress sync with only one device playing at a time

## Round 6 (2026-09-08) — **the seek symptom is solved**

Symptom as reported: the seeker shows the correct resume point but audio plays from the start
of the chapter. iOS Chrome deterministically, on both cold resume and manual drag-seek; iOS
Safari only after the page had been backgrounded. Android, laptop, and Safari's first play of
a session were always fine.

### First: everything rounds 1–5 blamed was cleared by direct test

Branch `player-engine-baseline-reset` (commit `ded0ada`) reset `engine.ts` and `stream_file`
to their pre-`e578e8a` state — no `applyPendingStart`, no `loading`/`pendingStartSec`, no
`RANGE_CHUNK_BYTES`, no `resumeAudio`. **The bug reproduced unchanged.** So it predates this
entire investigation, and none of the lock-screen work caused it. Service-worker interference
was separately ruled out: `vite.config.ts` has no route for `/api/stream/`.

### The measurement nobody had taken

Six rounds inferred; none had logged what byte range the phone actually asks for. With
`stream range` logging on, resuming file 1309 at `progress_ms: 397072`:

```
05:39:55  file=1309  req=bytes=0-1            206  cr=bytes 0-1/11015163
05:39:55  file=1309  req=bytes=0-11015162     206  cr=bytes 0-11015162/11015163   <- cold load
05:40:09  file=1309  req=bytes=0-1            206  cr=bytes 0-1/11015163
05:40:09  file=1309  req=bytes=3407872-...    206  cr=bytes 3407872-11015162/...  <- retry
05:40:10  file=1309  req=bytes=2200112-3407871 206                                <- gap fill
```

The cold load asks for **the whole file from byte 0 and never issues a seek at all**, while
the store holds 397 s. The retry asks for byte 3407872 — exactly the 64 KiB-aligned floor of
the true offset, `259275 + 397.072 × 8000 = 3435851` — and then played forward at precisely
1x, `progress_ms` going 397072 → 758321 over 361 s of wall clock. The client can compute the
right offset. The assignment was being dropped before it ever reached the network.

### Cause

`audio.currentTime = startSec` is assigned immediately after `audio.src`, while `readyState`
is `HAVE_NOTHING`. Per spec that should become the element's *default playback start
position*. WebKit ignores it. Chrome and Android honour it — hence iOS-only. A warmed element
sometimes succeeds, which is why Safari failed only after backgrounding and why the symptom
looked intermittent.

Note this is **not** new information to the codebase: `applySource` already carried the
comment "WebKit ignores it this early, so 'loadedmetadata' re-applies it". That re-seek alone
was on the failing build and was **not sufficient**.

### Fix

`withStartFragment()` in `src/ui/src/player/engine.ts` appends a Media Fragments URI
(`#t=<sec>`) to the source, so WebKit applies the start position while parsing the URL rather
than through the JS setter. `applyPendingStart()` on `loadedmetadata` is kept as the second
belt. A start of 0 (chapter advance) gets neither, so that path is unchanged. The fragment
never reaches the server. **Confirmed working on device.**

`audio.currentSrc === offlineBlobUrl` in the `error` listener became `startsWith` — the
source now carries a fragment.

### Regression introduced by the reset, and fixed here

The reset was a diagnostic instrument and reverted eight unrelated fixes with the suspect
code. One broke cross-device sync and was reported immediately: `saveProgress` lost its shared
`updated_at`, so the server row was stamped with the server's clock (`CURRENT_TIMESTAMP` in
`upsert_progress`) while the localStorage row for the same save carried the client's.
`reconcileProgress` compares those two directly, so clock skew decided which side won, in
either direction, and nothing converged. Restored, along with the `loading` guard and the
`atSec` parameter. `RANGE_CHUNK_BYTES` and `clamp_range` restored verbatim at 4 MiB.

**Still reverted, to be restored in a follow-up pass:** `effectiveDuration()`,
`updatePositionState()`, chapter name as lock-screen title, `preload = 'metadata'`,
`prefetchNext`/`warmNextStream`, saves on `seek`/`skip`, the `pagehide` save listener, and
`playbackState = 'none'` at end of book.

### Method note

Six rounds of inference produced six wrong answers. One log line produced the right one in a
single test. When a symptom crosses a process boundary, measure at the boundary first.

## Round 4 (2026-08-16)

### Round 3's fix failed, and the failure was informative

After deploying `preload = 'metadata'`, a 20-minute listen produced **16 boots** — a
*higher* rate than the 18-per-evening baseline. The gate was designed to be falsifiable and
it fired.

But the hypothesis wasn't wrong, the lever was: **`preload` only governs behaviour before
`play()` is called.** Once playback starts it no longer constrains anything, so the browser
was free to keep reading ahead exactly as before.

### The measurement: ~350× real-time read-ahead

Two consecutive ranges for one file, from the `stream range` log:

```
06:17:41  file_id=10  bytes=344129536-884053436
06:17:56  file_id=10  bytes=386007040-884053436
```

41,877,504 bytes in 15 seconds = **2.79 MB/s**. That file is 884053436 bytes over
110499.657 s — 64 kbps, i.e. **8,000 bytes/s** of real-time demand. The client is pulling
roughly **350× faster than it plays**. That is the memory pressure, confirmed rather than
hypothesised.

The reload it causes is visible in the same log, same session:

```
06:17:56  /api/stream/10        <- playing
06:17:57  /api/health?boot=1    <- page is gone and back, cold
06:17:59  /api/stream/10        <- fresh <audio>, restarted
```

**Why it reads unboundedly:** ~~these MP3s are VBR with no Xing/TOC header~~ — the same defect
that makes `duration` come back `Infinity` and forced `effectiveDuration()` into existence.
With no byte↔time map the browser can't convert "buffer N seconds ahead" into a byte
budget, so it reads until the pipe runs dry.

> **Over-generalised — corrected in Round 6.** This holds for `file_id=10`, an 884 MB
> 30-hour single file, and was wrongly extended to the whole library. It is false for the
> book these rounds were actually tested against: file 1309 (*Persepolis Rising*, ch. 15) is
> MPEG2 Layer III, 64 kbps **CBR**, 22050 Hz, carrying a complete `Info` header with frame
> count, byte count, TOC and quality flags, starting at byte 259275 after a 253 KB ID3v2
> tag. Its duration is well-defined and its byte↔time map is exact. `effectiveDuration()` is
> still worth keeping for files like 10, but the "no Xing header" premise must not be used to
> explain behaviour on files that have one.

### The fix: clamp the range server-side

`stream_file` (`src/api/audiobooks.rs`) now narrows an over-broad `Range` to
`RANGE_CHUNK_BYTES` (4 MiB) before handing the request to `ServeFile`. RFC 7233 explicitly
permits answering with a *narrower* range than requested; the media element issues a
follow-up request when it needs more. 4 MiB is >8 minutes of read-ahead at 64 kbps.

This is the right layer: it's browser-agnostic (Chromium's closed ranges and WebKit's open
ones are both capped), it's the one hop we fully control, and it's one small reversible
function.

**The one trap, checked:** `downloadBook()` in `src/ui/src/offline/storage.ts:145` fetches
**rangeless** and streams the whole body. A blanket clamp would silently truncate every
download to 4 MiB. `clamp_range` therefore only rewrites an existing `Range` header — a
rangeless GET still returns 200 and the entire file.

Verified locally against a 684942445-byte file:

| Request | Served |
| --- | --- |
| `bytes=0-1` (the probe both browsers open with) | `bytes 0-1/…`, 2 B — untouched |
| `bytes=0-` (WebKit) | `bytes 0-4194303/…`, 4 MiB |
| `bytes=0-884053436` (Chromium) | `bytes 0-4194303/…`, 4 MiB |
| `bytes=344129536-` | `bytes 344129536-348323839/…`, 4 MiB |
| `bytes=1000-2000` (already small) | untouched |
| `bytes=-500` (suffix) | untouched, passes through to `ServeFile` |
| `bytes=684942000-` (near EOF) | truncates to file length, 445 B |
| `bytes=684942445-` (past EOF) | 416, `bytes */684942445` |
| *(no Range)* | 200, full 684942445 B — download manager safe |

### Round 4 result: the clamp works, but not the way I claimed

**First, the counter has been lying all along.** `main.rs` logs both `Request Start` and
`Request end`, and both lines carry the URI — so `grep -c 'boot=1'` counts every app start
**twice**. Every number in this document before now is 2× the truth. Corrected command:

```sh
ssh pi "journalctl -u else-wer.service --since '20 min ago' | grep 'boot=1' | grep -c 'Request Start'"
```

Restated: the round-3 baseline was **9** app starts in an evening, not 18; the failed
`preload` deploy was **8** in 20 minutes, not 16. The shape of the finding survives — those
are still far too many — but the magnitudes were wrong.

**Second, the clamp did not reduce read-ahead.** It changed request granularity, not the
working set. From the round-4 log, `file_id=10`:

```
12:37:28.504  bytes=344326144-  served 4 MiB
   … 10 requests, back to back …
12:37:30.353  bytes=382074880-  served 4 MiB
12:37:46.229  bytes=386203648-      <- 16 seconds later
```

41.9 MB in **1.85 seconds**, then idle for 16. Before the clamp it was 41.9 MB per 15
seconds from one open-ended response. **Same ~42 MB buffered either way** — the client just
chains the chunks and fetches the same amount faster. ~42 MB is about Chromium's media
buffer cap, so the browser was self-limiting all along and the "unbounded read-ahead"
reading of the round-3 log was wrong: those two log lines 15 s apart were one refill cycle,
not a continuous 2.79 MB/s drain.

The clamp stays — bounded responses beat a single 884 MB streamed body on a Pi Zero 2, and
it costs one request per 4 MiB — but **it is not the fix and must not be credited as one.**

**Third, the gate was never actually run.** Pi clock was 12:39; all activity in the
"20 minute" window fell between 12:37:01 and 12:37:46. That's ~45 seconds of playback, in
which there were still **2 app starts** (12:37:10 and 12:37:24, 14 s apart, both
mid-stream). That is the same kill rate as before, measured over too short a window to mean
anything either way.

One genuine positive: the resume seek is visibly correct — `bytes=0-1`, then
`bytes=0-884053436`, then straight to `bytes=344326144-`, the saved position. And the final
stretch from 12:37:28 ran 18 s with no boot and no fresh `bytes=0-1`.

### The gate, again

Needs a **continuous 15-minute listen** — the run above covered 45 seconds and settled
nothing. Then, with the corrected counter:

```sh
ssh pi "journalctl -u else-wer.service --since '20 min ago' | grep 'boot=1' | grep -c 'Request Start'"
```

One deliberate open should read 1. Anything above ~2 over a quarter-hour of continuous
playback means the page is still being killed.

### If the reloads persist

Stop; do not layer a fifth fix. The remaining suspects, re-prioritised now that ~42 MB is
known to be the client's own buffer cap rather than runaway read-ahead:

1. **Do reloads happen with nothing playing at all?** If so this was never the media
   element and the search moves to the app shell. This is now first — the media-element
   memory theory has survived two rounds without producing a working fix.
2. `getOfflineFileBlob()` materialising a whole downloaded file as an in-memory `Blob`.
   Note it does **not** apply to `file_id=10`: that book streams from `/api/stream`, so it
   isn't downloaded and this path never runs for it. Only relevant if reloads also happen
   on downloaded books.
3. Whether reloads correlate with book size (a grep of the existing log, not an experiment).

Also unresolved: the counter doesn't distinguish clients, and the PWA, Safari and Chrome
were all in use during these measurements. If the next number is ambiguous, add a
per-client discriminator to `?boot=1` before trusting it.

## Round 3 (2026-08-16)

### What the logs settled

- **The deploy was current.** `dist/assets/index-DYyDIDbj.js` and `sw.js` on the Pi were
  byte-identical to the local build; the binary contained the `stream range` log. The
  "stale bundle" explanation is exhausted — hence the build stamp, so this costs no more
  time.
- **Range requests work, in both browsers.** Chrome sends closed ranges, WebKit open ones,
  both carrying real seek offsets (`bytes=344129536-884053436`, `bytes=16482304-`), and
  **every one returned 206**. The service worker was never eating them. Round 2's theory
  was wrong. The `/api/stream/` route stays removed anyway — one less hop — but the comment
  in `vite.config.ts` no longer claims it fixed anything.
- **Saved progress is correct.** No `progress_ms: 0` since 2026-08-13, no spurious
  `complete`. Rounds 1–2 did their job; they just weren't the bug.

### The actual finding: 18 page loads in one evening

`boot=1` markers between 23:20 and 00:43 — three inside three seconds, several pairs one to
two seconds apart — each followed immediately by a fresh `bytes=0-1` + `bytes=0-<end>`
pair, i.e. a brand-new `<audio>` resource load. The web process is dying and the page is
coming back cold. That single fact explains everything reported:

- **Lock-screen resume silence (Safari and PWA).** The page was discarded while locked.
  Pressing play lands on a freshly reloaded page whose `<audio>` has nothing buffered and
  no user activation. Round 1's fix never regressed — the page it lives in stopped
  existing.
- **"Seeker shows 16:52 but plays from start" (Chrome-on-iPhone only).** After a reload,
  `loadFile()` sets `audio.src` then `audio.currentTime = startSec`. Chrome treats that as
  the *default playback start position* and reports it from `currentTime` at once, so the
  seeker reads 16:52 while the decoder is still at the head of the file. WebKit ignores the
  early assignment and takes the `loadedmetadata` path, which is why Safari doesn't show it.
- **"Tried a different book and the count climbed."** Each new book is a new large buffer.

### The fix, and why it's shaped as a measurement — **refuted, see Round 4**

`audio.preload = 'auto'` on files from 20 MB to 884 MB let the browser buffer as much as it
liked; on iOS that is a jetsam invitation. Chromium's multibuffer preloads harder than
WebKit's and third-party iOS browsers get a smaller memory allowance, which matches Chrome
being worst-hit. Changed to `'metadata'` — `loadedmetadata` still fires, so
`applyPendingStart()` is untouched.

This is a hypothesis. Two confident single-shot theories have already been wrong, so the
gate is a number, not a feeling:

```sh
ssh pi "journalctl -u else-wer.service --since '20 min ago' | grep -c 'boot=1'"
```

Baseline 18/evening in clusters. Target ~1 per deliberate app open.

**Result: 16 boots in 20 minutes — worse.** `preload` stops applying once `play()` is
called, so it never constrained the read-ahead. See Round 4.

### If the boot count doesn't drop

Stop; do not layer another fix. In priority order:

1. `src/ui/src/offline/storage.ts` → `getOfflineFileBlob()` runs on *every* `loadFile()`.
   For a downloaded book it materialises the whole file as an in-memory `Blob` held alive
   by `URL.createObjectURL`. On an 884 MB file that alone would jetsam the process.
2. Do reloads correlate with book size? `file_id=10` is 884 MB, `file_id=522` is 20 MB, and
   the log already pairs boots with `file_id` — a grep, not an experiment.
3. Do reloads happen with nothing playing? If so it isn't the media element, and the search
   moves to the app shell.

## What the Pi's logs proved

The evening's `journalctl -u else-wer.service` is the first hard evidence in this
investigation. Two things fell out of it.

### The app was restarting constantly

Every burst in the log has the same shape:

```
/api/health  → /api/refresh_token          <- App.tsx module scope: one per app start
/api/list_books + /api/file_metadata/40 + /api/get_book_progress/40   <- loadPlayerData
/api/update_progress → /api/stream/522     <- loadBook() → loadFile() → new audio.src
```

`attemptRefresh()` runs at module scope on boot (and on an offline→online transition
only), so that leading pair counts app starts. There were 18 across the evening, five of
them inside 1.3 seconds (21:35:29–21:35:31 UTC). No human force-quits an app five times in
1.3 seconds, and the app contains no `location.reload()` — so those are the page being
discarded and relaunched under it.

That matters because **every relaunch re-runs `loadBook()` with autoplay**, which assigns a
new `audio.src` and restarts playback from whatever was last saved. So any defect that lets
a bad position get saved doesn't just cost you that one save — it gets replayed as the
resume point seconds later, over and over. That is the "kept starting from the beginning"
symptom, and it is why the seeker felt frozen: the page was going away underneath it.

### The `progress_ms: 0` clobber is visible in the data

`progress` still holds `book_id=19, file_id=207, progress_ms=0` from 2026-08-13 — a
surviving artefact of root cause 3 below.

## Symptoms

1. **Lock screen / Bluetooth: pause works, resume produces silence.** Reported on both
   Safari and Chrome on iPhone — the same bug, since every iOS browser is WebKit
   underneath.
2. **Books restart from the beginning**, persistently and across devices.
3. **Seeker unusable on Safari** — the handle froze, or wouldn't move at all.

## Root causes

### 1. Seek before play (symptom 1)

`resumeAudio()` seeked before playing:

```js
audio.currentTime = Math.max(0, audio.currentTime - RESUME_REWIND_SEC);  // seek
audio.play();                                                           // then play
```

While the page is hidden, that seek forces a re-buffer iOS won't service until the page is
foregrounded. WebKit starts the element anyway: `play()` resolves,
`mediaSession.playbackState` becomes `playing`, and the lock-screen clock ticks over no
decoded audio. `e578e8a` added the rewind; before it the handler was a bare `audio.play()`,
which is why lock-screen resume used to work.

**Why it was hard to see**

- **No console error.** `play()` genuinely succeeded — there was never an exception for the
  existing `.catch()` to catch. That log will never appear for this bug.
- **Pause kept working**, because pausing needs no buffered data.
- **Safari's Web Inspector cannot observe this case at all** — the target detaches the
  moment the phone locks (`cmds.md`, "Targets vanish when the phone locks").

### 2. Progress saved from a stale position (symptom 2)

`saveProgress()` read `currentTime` from the Zustand store, which only refreshes on
`timeupdate`. `seek()`/`skip()` assign `audio.currentTime` and save in the **same tick**, so
every explicit seek persisted the position from *before* the seek. Both also set
`lastSavedSec` first, suppressing the periodic save that would have corrected it.

### 3. A source swap wrote `progress_ms: 0` (symptom 2)

Assigning `audio.src` runs the media load algorithm, which fires `pause` if the element was
playing. `e578e8a` added a `saveProgress()` call to the `pause` listener. By then
`loadBook()`/`setIndex()` had reset the store's `currentTime` to 0 and repointed
`files[index]` at the **new** file — so opening a part-way book saved it at 0.

It stuck and it spread: `saveProgress` also writes localStorage, and `reconcileProgress`
treats a newer local row as authoritative and pushes it to the server.

### 4. Start position dropped on load (symptom 2)

`loadFile()` assigned `audio.currentTime = startSec` in the same tick as `audio.src`, with
`readyState` at `HAVE_NOTHING`. Chrome honours that as the default playback start position;
WebKit routinely drops it.

### 5. Seeker (symptom 3)

- `audio.duration || 0` passed `Infinity` through. WebKit reports `Infinity` for a streamed
  VBR MP3 with no Xing header (true of some files, e.g. `file_id=10`, but not all — see the
  Round 6 correction above); React renders `max="Infinity"`, the browser rejects it.
- While the duration was unusable the slider still had `max={1}`, so a drag committed a
  seek into 0..1 — a third route to "starts from the beginning".
- `seeking` was only cleared on `pointerup`/`keyup`; lose the pointer any other way and it
  latched true — the frozen handle.

### 6. The round-1 fix saved a seek it couldn't read back (new)

`applyPendingStart()` re-applied the start position on `loadedmetadata` and then saved
`audio.currentTime`. But a seek is applied **asynchronously** — while the element is still
buffering its first bytes, `currentTime` can still read `0` for a moment after the
assignment. So the round-1 fix could itself persist `progress_ms: 0`, and with the app
relaunching every few seconds that 0 came straight back as the next resume point. It now
reports `pendingStartSec` — the position it asked for — instead.

### 7. Nothing saved the position when iOS discarded the page (new)

Only `visibilitychange → hidden` saved on teardown. iOS does not reliably fire it before
discarding a backgrounded standalone PWA, so everything since the last 10-second periodic
save was lost on each lock. Added a `pagehide` listener alongside it.

### 8. Deep links returned 404 (new)

`ServeDir::not_found_service()` forces the response status to 404 even though the body it
serves is `index.html`. Every SPA route (`/player/16`, `/book/3`) was a 404 — visible in the
log as `GET /player/40 → 404`. Browsers still render it, but a 404 is not a cacheable
navigation response. Switched to `.fallback()`; deep links now return 200.

## The deploy trap (why "I deployed and nothing changed")

Reproduced locally, and it is almost certainly what happened on the phone:

1. Built a new `dist/`, reloaded the page **three times**.
2. The new service worker sat in `waiting` the whole time; the old one stayed `active` and
   kept serving the **old** precached bundle. `document.querySelector('script[type=module]')`
   still pointed at the previous hash.
3. Only after navigating away (no client left holding the old worker) and back did the new
   worker activate and the new bundle load.

`registerType: 'autoUpdate'` emits `self.skipWaiting()` in `sw.js`, but that did not evict
the running worker while a client was still attached. On iOS this is stickier still — the
PWA has to be swiped out of the app switcher, not just backgrounded.

**So a deploy cannot be assumed live.** Check it, don't infer it — see Deploy.

## The fix

`src/ui/src/player/engine.ts`

- `resumeAudio()` is seek-free. **Keep the explanatory comment** — without it the rewind is
  an obvious "improvement" for a future session to reintroduce, reproducing symptom 1.
- `saveProgress(complete, atSec = audio.currentTime)` — position defaults to the element,
  and callers pass it explicitly when the element can't be trusted yet. Returns early while
  `loading` is set.
- `loading` + `pendingStartSec` are set in `loadFile()` before the src is assigned;
  `applyPendingStart()` (from `loadedmetadata`) re-applies the position, clears the flag and
  saves **`pendingStartSec`**, not `audio.currentTime` (cause 6). The `error` listener also
  clears `loading` so a source that never loads can't suppress saves for good.
- `effectiveDuration()` prefers a finite element duration, else the scanned
  `FileMetadata.duration` (ms — `src/file_ops/scan_files.rs`, `as_millis()`). Every
  `audio.duration || 0` call site goes through it, so `MiniPlayer`'s bar benefits too.
- `pagehide` save alongside the `visibilitychange` one (cause 7).

`src/ui/src/components/player/Seeker.tsx`: `Number.isFinite` guard on `ready`,
`disabled={!ready}`, `setPointerCapture` on pointerdown, `onPointerCancel` /
`onLostPointerCapture` clearing `seeking`.

`src/main.rs`: `ServeDir::fallback()` instead of `not_found_service()` (cause 8).

`src/ui/src/lib/healthPoll.ts`: first probe of each page load goes to `/api/health?boot=1`.
Pure instrumentation — app starts become greppable in the server log, which is the only
vantage point available for a standalone iOS PWA. Remove it once the relaunch behaviour is
understood.

`audio.play()` was deliberately **not** moved into `loadedmetadata`: iOS ties playback to
user activation and the current call site is proven to work.

An intermediate version of fix 1 gated the rewind on `document.visibilityState ===
'visible'`, keeping it in-app while skipping it on the locked path. That also tested working
on device, but was dropped for consistent behaviour across surfaces. It is the fallback if
the in-app rewind is ever missed.

## Verified locally

Chrome against `./target/debug/else-wer` on :3000 serving the **production** build (so the
service worker is in play — the round-1 testing used `npm run dev`, which has no service
worker at all, and that is why none of this surfaced).

| Action | Result |
| --- | --- |
| Reload mid-playback at 14635 s | resumed 14643 s and kept playing |
| Three rapid reloads in a row | 14650 → 14670 s, no regression |
| Any `progress_ms = 0` written during the run | **none** |
| Seek to 3600 s | saved `progress_ms: 3600000` exactly, playback continued from there |
| Deep link `/player/16`, `/book/16` | 200 (was 404) |
| `?boot=1` markers | present in the log once the new worker took over |

`npm run lint`, `tsc --noEmit` and `cargo build` are clean (pre-existing warnings only:
`SearchableCombobox.tsx:17`, and dead-code warnings in `src/models/meta_scan.rs`).

Still device-only, not reproducible on Linux Chrome: the lock-screen/Bluetooth resume
itself, and whether WebKit really reports a non-finite duration for these MP3s.

## Deploy

The PWA is served from the Pi as a static `dist/`; there is no build step on the server.
Note this deploys the **server binary too** now (`src/main.rs` changed):

```sh
cd piconfig && make deploy      # builds + ships both the PWA and the server binary
```

PWA only:

```sh
cd src/ui && npm run build && rsync -a --delete dist/ pi:/home/pi/else-wer-pwa/dist/
```

> `rsync` to the live server is blocked by the agent permission classifier, so this has to
> be run by hand.

Then on the phone: **swipe the PWA out of the app switcher**, not just background it, and
reopen. Backup of the pre-fix live build (2026-08-15):
`/home/pi/else-wer-pwa/dist.bak.20260815-152948`.

### Confirm the deploy is actually live

Don't infer this. After reopening the app on the phone:

```sh
ssh pi "journalctl -u else-wer.service --since '10 min ago' | grep -c 'boot=1'"
```

Non-zero means the new bundle is running. Zero means the phone is still on the old service
worker and **nothing you changed is being tested** — that is what round 1 most likely hit.

## Verify

On device (iPhone, Safari and Chrome), after confirming `boot=1` appears:

- Lock screen: play → lock → pause → play. Audio resumes immediately, same position, no 5s
  jump. Bluetooth headset behaves the same.
- In-app pause/resume no longer jumps back 5s (intended — the rewind is gone everywhere).
- Open a part-way book: starts where you left it, not at 0.
- Drag the seeker: playback lands where you dropped it and stays there after a reopen; the
  handle keeps tracking afterwards; the total duration renders.
- Lock and unlock a few times, then check the boot count:
  `ssh pi "journalctl -u else-wer.service --since '10 min ago' | grep -c 'boot=1'"`.
  If that climbs with every unlock, iOS is discarding the PWA on background, and the next
  piece of work is making a relaunch resume seamlessly (it should now resume at the right
  position, but it will still re-request the stream).

One caveat: any position already corrupted to 0 by the old build stays 0 until the book is
played past that point again. `book_id=19, file_id=207` is one such row.

## Not mine

`.github/workflows/ci.yml` and `release.yml` show up as modified (both fully commented
out). That was not part of this work — left untouched.
