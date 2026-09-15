# else-wer on a Raspberry Pi (native, arm64)

This runs the cross-compiled binary under systemd, with no Docker. Every command is a target in
the repo root `Makefile`, and they work from any machine with ssh access to the Pi and
[`cross`](https://github.com/cross-rs/cross) installed.

Settings go in the root `.deploy.env` (see `.deploy.env.example`): `PI_HOST`, `PI_USER`,
`PI_HOME`, `PI_LIBRARY_MOUNT` (where the library drive is mounted) and `PI_LIBRARY_UUID`
(its filesystem UUID from `blkid`). `else-wer.service`, the udev rule and `.env.example`
here are templates, and the install targets fill them in.

One-time setup:

```sh
make pi-install-unit   # systemd unit; refuses to run without PI_LIBRARY_MOUNT mounted
make pi-install-udev   # remounts the library drive after a USB drop (skipped without PI_LIBRARY_UUID)
make pi-env            # seeds the Pi's ~/.env from deploy/pi/.env (or .env.example); never overwrites
```

Every deploy after that:

```sh
make pi-deploy         # builds + ships the server binary and PWA build, restarts the service
make pi-status / pi-logs
```

The Pi reads `~/.env` and has no git checkout, so `git pull` never touches its config.
`mdr.sh` mounts the first USB disk at `$MOUNTPOINT` (default `/home/pi/drv`), for manual recovery.

**sudo:** the `sudo ...` calls run over key-only ssh. If the Pi user needs a password for
sudo, these targets hang waiting for a TTY prompt. Fix it by adding a scoped NOPASSWD rule on
the Pi, as root: `visudo -f /etc/sudoers.d/else-wer` with (adjust the user)

    pi ALL=(ALL) NOPASSWD: /usr/bin/systemctl stop else-wer.service, /usr/bin/systemctl start else-wer.service, /usr/bin/systemctl enable else-wer.service, /usr/bin/systemctl daemon-reload

Do this on the Pi yourself. It's not something to automate blindly from the dev machine.
