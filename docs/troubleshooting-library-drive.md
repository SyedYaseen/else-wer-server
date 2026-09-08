# Playback fails but the app otherwise works: the library drive isn't mounted

> **Read this first (updated 2026-09-01).** `else-wer.service` now has
> `RequiresMountsFor=/home/pi/drv`, so systemd **stops the server** when the drive
> drops instead of letting it serve 404s. The symptoms below — app alive, nothing
> plays — are therefore the *old* behaviour, seen only if that unit is reverted.
> Today a drive drop looks like a connection refused through Caddy, with
> `systemctl is-active else-wer` reporting `inactive`.
>
> Recovery is also automatic now: a udev rule remounts the drive and restarts the
> server when it reappears. If that worked, you are not reading this page. See
> [todo/13_USB_HUB_STABILITY.md](todo/13_USB_HUB_STABILITY.md) for the mechanism,
> the hardware fault behind these outages, and what is still unfixed.

The most confusing failure mode this server has. Everything looks alive — you
can log in, browse books, covers render, progress syncs across devices — but
**no book will play, on any client**.

The cause is almost always that the external library drive is unmounted, so
`AUDIOBOOKS_LOCATION` points at an empty directory and every stream request
404s.

## Why the symptoms mislead you

The app is built to survive a flaky network, and that resilience hides the
outage:

- **Progress keeps working.** Progress lives in SQLite on the Pi's SD card
  (`DATABASE_URL=sqlite:./else-wer.db`), not on the library drive. Reads and
  writes succeed normally, so books still show as "in progress" and positions
  still sync between phone and laptop.
- **Covers still render.** The service worker caches them for 30 days
  (`StaleWhileRevalidate`, `src/ui/vite.config.ts`). They are 404ing on the
  server the whole time — you're looking at cache.
- **Only audio breaks visibly.** `/api/stream/` is deliberately `NetworkOnly`
  (same file), because a 206 range response must never be cached. It has no
  cache to fall back on, so it's the first thing to visibly fail.
- **Pause works, resume doesn't.** Pausing is local to the `<audio>` element
  and always succeeds. Resuming needs bytes from the server. This can look
  exactly like a Media Session / lock-screen bug.

The single most useful signal: **404, not 401.** A 401 means auth; a 404 on a
stream URL means the server accepted your token and then couldn't find the
file.

## Diagnose

Run these on the server box (`ssh pi`).

### 1. Confirm the server is actually up

```sh
systemctl is-active else-wer
systemctl is-active caddy
ss -tlnp | grep 3000          # else-wer should be listening
```

### 2. Read the request log — this is the decisive step

```sh
journalctl -u else-wer --since "1 hour ago" --no-pager | tail -60
```

Look at the status code on `/api/stream/` lines:

| Status | Meaning |
| ------ | ------- |
| `404`  | File not found on disk — **this document's problem** |
| `401`  | Auth: bad, expired, or missing token |
| `200` / `206` | Streaming fine; the problem is client-side |

A drive-unmounted log looks like this (note that covers 404 too):

```
uri=/api/stream/246?token=...              status=404
uri=/api/covers/the_children_of_hurin.jpg  status=404
```

> Request URLs contain a JWT in the `?token=` query param. Redact it before
> pasting logs anywhere.

### 3. Check whether the drive is mounted

```sh
lsblk -o NAME,SIZE,MOUNTPOINT,LABEL
```

The failure is obvious — the partition is present but has no mountpoint:

```
sda         111.8G
└─sda1      111.8G                <-- empty MOUNTPOINT column
```

Cross-check the mountpoint and the configured library path:

```sh
findmnt /home/pi/drv              # no output = not mounted
grep AUDIOBOOKS_LOCATION /home/pi/.env
ls -la /home/pi/drv/              # empty = drive missing, not a path typo
df -h | grep -v tmpfs
```

## Fix

### Mount it now

```sh
sudo mount /dev/sda1 /home/pi/drv
ls /home/pi/drv/AudioBooks
```

Then start the server, which systemd stopped when the mount went away:

```sh
sudo systemctl start else-wer
systemctl is-active else-wer
```

> This used to say no restart was needed, because else-wer opens files per request.
> That is no longer true: `RequiresMountsFor=/home/pi/drv` means a dropped mount
> takes the service down with it. In normal operation the udev rule
> (`piconfig/99-else-wer-library-drive.rules`) does both steps for you.

### Verify every file in the DB resolves

This is the real confirmation, better than spot-checking one book:

```sh
sqlite3 /home/pi/else-wer.db "select count(*) from files;"

sqlite3 /home/pi/else-wer.db "select file_path from files;" \
  | while read -r f; do [ -f "$f" ] && echo ok; done | wc -l
```

The two numbers must match. If the second is lower, some files are genuinely
missing or moved, which is a different problem (rescan the library).

## Make it survive reboots

> **Already done on this deployment** — the entry below is present in `/etc/fstab`
> and boots clean; verified after a hard power cycle on 2026-09-01. Keep this
> section for rebuilding a Pi from scratch. Note that a correct fstab entry does
> *not* address the mid-run USB drops, which are a hardware fault: see
> [todo/13_USB_HUB_STABILITY.md](todo/13_USB_HUB_STABILITY.md).

The root cause of that first outage was that the drive had **no `/etc/fstab` entry**
— it had been mounted by hand, so it never came back after a reboot.

Get the filesystem UUID (stable across replug; `/dev/sdX` names are not):

```sh
sudo blkid /dev/sda1
```

Back up fstab, then append the entry:

```sh
sudo cp /etc/fstab /etc/fstab.bak.$(date +%Y%m%d)

echo "UUID=27cca5db-fbf5-420d-8abe-7c2ffb84d369  /home/pi/drv  ext4  defaults,nofail,x-systemd.device-timeout=10  0  2" \
  | sudo tee -a /etc/fstab
```

Substitute your own UUID. The options matter:

- **`nofail`** — if the drive is absent or dead at boot, the Pi still boots
  normally instead of dropping into emergency mode. Non-negotiable for an
  external drive on a headless box.
- **`x-systemd.device-timeout=10`** — wait at most 10s for a slow-spinning USB
  disk rather than systemd's 90s default.
- **`0 2`** — no dump; fsck pass 2 (after root).

Validate the syntax **before** trusting it to a reboot — a malformed fstab can
leave the box unbootable:

```sh
sudo findmnt --verify --verbose
sudo systemctl daemon-reload      # systemd caches the old fstab otherwise
```

Then confirm the entry actually mounts, ideally when nobody is streaming
(unmounting yanks the library out from under the running server):

```sh
sudo umount /home/pi/drv
sudo mount  /home/pi/drv          # no device arg = resolved from fstab
findmnt /home/pi/drv
```

If that round-trip succeeds, the next reboot will mount cleanly.

## Current values on this deployment

| Thing | Value |
| ----- | ----- |
| Server host | `pi` (`192.168.1.10`), user `pi` |
| Service | `else-wer.service`, binary `/home/pi/else-wer`, port 3000 |
| Config | `/home/pi/.env` |
| Library path | `/home/pi/drv/AudioBooks` |
| Drive | `/dev/sda1`, ext4, 112 GB |
| Drive UUID | `27cca5db-fbf5-420d-8abe-7c2ffb84d369` |
| Database | `/home/pi/else-wer.db` (SD card, not the drive) |

## If mounting doesn't fix it

- **`lsblk` doesn't list `sda` at all** — the drive is not detected. Check
  `dmesg | tail -40` for USB errors, reseat the cable, check power. A Pi can
  brown-out a bus-powered 2.5" drive; use a powered hub.
- **Mount fails with a filesystem error** — check with `sudo fsck -n /dev/sda1`
  (`-n` is read-only, safe to run first).
- **Mounted, but files still 404** — the DB paths and the on-disk layout have
  diverged (drive mounted at a different path, or the library was
  reorganized). Compare `sqlite3 else-wer.db "select file_path from files limit 5;"`
  against the actual tree, and rescan if they don't line up.
- **Streams return 401, not 404** — not this problem. That's auth: check the
  token and `refresh_token` handling.
