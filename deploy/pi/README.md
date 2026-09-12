# else-wer on the Pi Zero 2 W (native)

Pi is `192.168.1.10`, ssh alias `pi`. It runs the cross-compiled binary under systemd,
with no Docker. Every command is a target in the repo root `Makefile`, and they work from
any machine with the `pi` ssh alias and `cross` installed.

One-time setup on a fresh Pi:

```sh
make pi-install-unit   # systemd unit (else-wer.service here)
make pi-install-udev   # udev rule that remounts the library drive after a USB drop
make pi-env            # seeds /home/pi/.env from deploy/pi/.env (or .env.example); never overwrites
```

Every deploy after that:

```sh
make pi-deploy         # builds + ships the server binary and PWA build, restarts the service
make pi-status / pi-logs
```

`deploy/pi/.env` (gitignored) is your local copy of the Pi's env. The Pi reads
`/home/pi/.env` and has no git checkout, so `git pull` never touches its config.

`mdr.sh` mounts the first USB disk at `$MOUNTPOINT` (default `/home/pi/drv`), for manual recovery.

**sudo:** the `sudo systemctl ...` calls run over key-only ssh. If the pi user needs a
password for sudo, these targets hang waiting for a TTY prompt. Fix it by adding a scoped
NOPASSWD rule on the Pi, as root: `visudo -f /etc/sudoers.d/else-wer` with

    pi ALL=(ALL) NOPASSWD: /usr/bin/systemctl stop else-wer.service, /usr/bin/systemctl start else-wer.service, /usr/bin/systemctl enable else-wer.service, /usr/bin/systemctl daemon-reload

Do this on the Pi yourself. It's not something to automate blindly from the dev machine.
