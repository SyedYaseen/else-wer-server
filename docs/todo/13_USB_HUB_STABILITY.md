# 13. The USB hub drops the library drive (and sometimes the whole Pi)

Status: **PARTIALLY SHIPPED 2026-09-01.** Auto-recovery from a drive-only drop is deployed and
tested. The underlying hardware fault is not fixed, and the whole-box variant has no mitigation.
Remaining work is the hub swap in "Approach, step 2" and the two gaps in "Open questions".
Supersedes the mitigation advice in [troubleshooting-library-drive.md](../troubleshooting-library-drive.md),
which documents the *symptoms* well but predates `RequiresMountsFor=` and is corrected below.

## Problem

The audiobook library drive drops off the USB bus repeatedly, taking else-wer down with it.

Recorded occurrences (all without a reboot — uptime kept climbing through them):

| When | What dropped | Recovery |
| ---- | ------------ | -------- |
| 2026-08-18 18:55 | drive | manual mount |
| 2026-08-27 20:40 | drive | remounted 21:34 |
| 2026-08-28 22:28 | drive | remounted 22:40 |
| 2026-08-29 13:28 | drive | manual mount 13:36 + manual `systemctl start else-wer` |
| 2026-09-01 ~06:00 | **the whole box** — off the network entirely | power cycle |

### The hardware, which is the root of it

`/proc/device-tree/model` is a **Raspberry Pi Zero 2 W Rev 1.0**. It has a single `dwc_otg`
root port (`lsusb -t` shows `Driver=dwc_otg/1p`), and *everything* hangs off one hub on it:

```
/:  Bus 001.Port 001: Dev 001, Class=root_hub, Driver=dwc_otg/1p, 480M
    |__ Port 001: Dev 003, If 0, Class=Vendor Specific Class, Driver=r8152, 480M   <- ethernet
    |__ Port 002: Dev 004, If 0, Class=Mass Storage, Driver=usb-storage, 480M      <- library drive
```

- Hub: Genesys Logic `05e3:0610` — **externally powered**
- Ethernet: Realtek RTL8153 `0bda:8153` (there is no onboard NIC on a Zero 2 W)
- Drive: OWC SATA adapter `7825:a2a4` → `/dev/sda1`, ext4, 112 GB,
  UUID `27cca5db-fbf5-420d-8abe-7c2ffb84d369`

Two consequences follow from that topology, and they explain both failure shapes:

1. **There is no second port to move anything to.** Separating the drive from the ethernet
   adapter is not an option on this board; they must share the one hub.
2. **A hub reset takes the network with it.** When only the drive drops you get a degraded but
   reachable box. When the hub itself fails to re-enumerate you lose ethernet too, and the Pi is
   gone from the LAN — up and running, but unreachable and headless.

By 2026-08-29 the USB device numbers had climbed to 62/63/64, i.e. dozens of re-enumerations.
After the 09-01 power cycle they reset to 2/3/4.

**Power is ruled out.** `vcgencmd get_throttled` returns `0x0` and the kernel log has zero
under-voltage or over-current events. The hub is powered. So this is the hub itself or the
Zero 2 W's `dwc_otg` driver, not brownout — which is what makes the hub swap the real fix.

### Why the server did not come back on its own

`piconfig/else-wer.service` already had the right pieces:

- `RequiresMountsFor=/home/pi/drv` — so systemd **stops** else-wer when the mount drops,
  instead of leaving it serving 404s against a bare mountpoint. This works, and is a real
  improvement over the behaviour the troubleshooting doc describes.
- `WantedBy=home-pi-drv.mount` in `[Install]` — intended to start it again when the mount returns.

The second one silently did not fire. `Wants=` is only honoured when systemd starts the mount
**as a job**. A filesystem that reappears outside a systemd job — the kernel re-attaching the
disk, or someone running `mount(8)` — only gets *marked* active, and none of its dependencies
run. So on 08-29 the drive came back at 13:36 and else-wer stayed dead until started by hand,
even though `/etc/systemd/system/home-pi-drv.mount.wants/else-wer.service` was correctly in place.

`nofail` in the fstab entry compounds it: no boot target wants the mount at runtime either, so
nothing else pulls it in.

## Approach

### Step 1 — auto-recovery from a drive-only drop  ✅ SHIPPED 2026-09-01

Make the *reappearing device* pull the mount in as a genuine systemd job, so the existing
`WantedBy=` chain fires. A udev rule matching the filesystem UUID does this in one line.

Files:

- **`piconfig/99-else-wer-library-drive.rules`** (new) — matches `ENV{ID_FS_UUID}` and sets
  `ENV{SYSTEMD_WANTS}+="home-pi-drv.mount"`. Matches on UUID rather than `/dev/sda1` because the
  kernel name is not stable across re-enumeration. Full reasoning is in the file's comments.
- **`piconfig/Makefile`** — new `install-udev` target, following the existing `install-unit`
  pattern; installs root-owned 0644 and runs `udevadm control --reload-rules`.
- **`piconfig/else-wer.service`** — unchanged. It was already correct; the missing piece was
  the trigger, not the unit.

Deployed to `/etc/udev/rules.d/99-else-wer-library-drive.rules`. See Verification below.

### Step 2 — replace the hub  ⬜ NOT DONE

The Genesys `05e3:0610` is common to every outage and power has been ruled out. Swap it for a
different powered hub (ideally a different chipset) and watch whether re-enumerations stop.

Worth capturing a baseline first so the swap can be judged rather than guessed at:

```sh
sudo journalctl -k --since "7 days ago" | grep -c "new high-speed USB device"
```

If a new hub does not help, the remaining suspect is the Zero 2 W's `dwc_otg` driver, and the
honest fix is different hardware — a Pi 4/5 has a real USB controller and separate ports, which
would also decouple ethernet from storage and eliminate the whole-box failure mode outright.

## Open questions

- **The whole-box failure has no mitigation.** When the hub takes ethernet down, nothing
  on the Pi can help — it is unreachable and headless. Options, none implemented:
  bring up onboard WiFi (`wlan0`) as a fallback path so the box stays reachable when the USB
  NIC dies; or a hardware watchdog (`/dev/watchdog`, `RuntimeWatchdogSec` in
  `/etc/systemd/system.conf`) to auto-reboot on a hard hang. WiFi is the cheaper win and would
  have turned 09-01 from a power cycle into a normal SSH session. **This is the most valuable
  remaining item** — step 1 does nothing for this case.
- **A dirty filesystem defeats the udev rule.** If the drive returns needing a journal replay,
  `mount` fails and the rule cannot `e2fsck`. Has not happened yet — the fs has come back
  `clean` every time, including after the 09-01 hard power cut — so no speculative machinery was
  added. If it starts happening, the place to fix it is a `systemd` drop-in with an
  `ExecStartPre` fsck on the mount unit, not the udev rule.
- Should `x-systemd.automount` be added to the fstab entry as a belt-and-braces second trigger?
  It would also mount on access. Deliberately not done: it interacts with `RequiresMountsFor=`
  in ways that need thought, and the udev path is tested and sufficient.

## Verification

Step 1 was tested with a genuine USB detach/re-attach — not just a synthetic `udevadm trigger`.
Unbinding only the storage device (`1-1.2:1.0`) leaves the hub alone, so SSH over ethernet
survives the test:

```sh
# drop the drive
echo "1-1.2:1.0" | sudo tee /sys/bus/usb/drivers/usb-storage/unbind
# bring it back
echo "1-1.2:1.0" | sudo tee /sys/bus/usb/drivers/usb-storage/bind
```

Observed, unattended:

| Step | `/dev/sda1` | `/home/pi/drv` | `else-wer` |
| ---- | ----------- | -------------- | ---------- |
| before | present | mounted | active |
| after unbind | gone | unmounted | **inactive** |
| after rebind | back | **mounted** | **active** |

The journal shows the whole chain with no human in it, 8 seconds end to end:

```
07:05:14 Unmounting home-pi-drv.mount...
07:05:14 Unmounted home-pi-drv.mount.
07:05:22 Mounting home-pi-drv.mount...
07:05:22 Mounted home-pi-drv.mount.
07:05:22 Started else-wer.service - Else-Wer.
```

Post-test health, all green: filesystem `clean`; **807/807** files in `files` readable off the
drive; `/api/health` 200 locally and 200 over HTTPS with SNI
(`--resolve valvasban.duckdns.org:443:127.0.0.1`); `/api/stream/3` → **401**, not 404.

To re-verify the rule matches after any change:

```sh
sudo udevadm test /sys/class/block/sda1 2>&1 | grep SYSTEMD_WANTS
#   SYSTEMD_WANTS=home-pi-drv.mount
```

### Correction to [troubleshooting-library-drive.md](../troubleshooting-library-drive.md)

That doc predates `RequiresMountsFor=` and is wrong in two places, now fixed there:

- It says *"No restart of `else-wer` is needed"* after remounting. That stopped being true when
  `RequiresMountsFor=` was added — systemd now stops the service with the mount.
- Its headline symptom ("everything works but nothing plays", 404 not 401) is now the **old**
  behaviour. Today a drive drop stops the server outright, so the symptom is a connection
  refused through Caddy. The 404 signature only appears if the unit is ever reverted.
