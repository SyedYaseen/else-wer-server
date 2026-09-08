# Debugging the PWA on iPhone (iOS 26.5.2)

Toolchain: go-ios v1.3.2 (`~/.local/bin/ios`) + Chromium DevTools.
Replaces ios-webkit-debug-proxy + ios-safari-remote-debug-kit, which crashed with
`getResourceContent` errors due to a WebKit-trunk / device version mismatch.

Prereqs: iPhone plugged in and unlocked, trusted, and
Settings -> Safari -> Advanced -> Web Inspector enabled. `usbmuxd` must be running.

sudo systemctl start usbmuxd

## 0. One-time: make `ios` visible to sudo

`sudo` uses `secure_path` (/usr/local/sbin:/usr/local/bin:/usr/bin) and ignores
`~/.local/bin`, so `sudo ios ...` fails with "command not found". Install it into
a secure_path dir once:

    sudo install -m755 ~/.local/bin/ios /usr/local/bin/ios

Otherwise, call it by absolute path every time: `sudo /home/loop/.local/bin/ios ...`

## 1. Start the tunnel (required for iOS 17+, keep running)

    sudo /home/loop/.local/bin/ios tunnel start

If it misbehaves, try the userspace tunnel:

    sudo /home/loop/.local/bin/ios tunnel start --userspace

Check it: `ios tunnel ls`

## 2. Verify the device

    ios list
    ios devicename

## 3. List inspectable pages

Open the PWA in Safari on the phone first, then:

    ios webinspector list

## 4. Bridge to Chrome DevTools Protocol

    sudo ios webinspector cdp --port=9222

Leave it running. Then either:

a) Chromium -> chrome://inspect -> Configure -> add `localhost:9222`, or
b) attach directly (more reliable):

    chromium "devtools://devtools/bundled/inspector.html?ws=127.0.0.1:9222/devtools/page/1"

Find the page id first:

    curl -s http://localhost:9222/json | jq '.[] | {id, url, title}'

### Gotchas (verified on iOS 26.5.2, go-ios 1.3.2)

- `/json` returns `[]` for the first few seconds until target enumeration runs.
  Wait and re-curl; it is not broken.
- The `webSocketDebuggerUrl` advertised by `/json/version`
  (`/devtools/browser/<uuid>`) returns **404**. Only the per-page endpoints
  `/devtools/page/<id>` accept the websocket upgrade. Attach per page, not per browser.
- Page ids change as tabs open/close - re-check `/json` rather than reusing an old id.
- Targets vanish when the phone locks or Safari backgrounds. Keep it awake and foregrounded.
- The trailing `webinspector read failed ... use of closed network connection` after
  `webinspector list` is just connection teardown; harmless.

## Quick alternatives (no DevTools UI)

    ios webinspector js-shell
    ios webinspector eval

## Notes

- `ios tunnel stopagent` stops the tunnel agent.
- Userspace/kernel daemon mode: `ENABLE_GO_IOS_AGENT=user` or `=kernel`.
- Tunnel info API defaults to 127.0.0.1:28100.
