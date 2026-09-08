# Deploy else-wer-server (including its src/ui PWA) to the Pi Zero 2 (192.168.1.10, host alias `pi`).
#
# One-time setup on a fresh Pi:
#   make install-unit   # installs the systemd unit (else-wer.service in this dir)
#   make install-udev   # installs the udev rule that remounts the library drive after a USB drop
# Every deploy after that:
#   make deploy          # builds + ships server binary and pwa static build, restarts service
#   make status / logs
# Local dev:
#   make dev              # runs the server (cargo run) and PWA (vite dev) side by side
#
# NOTE: `sudo systemctl ...` runs over the same key-only ssh used everywhere else here. If your
# pi user needs a password for sudo, these targets will hang waiting for a TTY prompt. Fix by
# adding a scoped NOPASSWD rule on the Pi (as root): `visudo -f /etc/sudoers.d/else-wer` with:
#   pi ALL=(ALL) NOPASSWD: /usr/bin/systemctl stop else-wer.service, /usr/bin/systemctl start else-wer.service, /usr/bin/systemctl enable else-wer.service, /usr/bin/systemctl daemon-reload
# Do this on the Pi yourself; not something to automate blindly from the dev machine.

PI             := pi
REMOTE_HOME    := /home/pi
REMOTE_PWA_DIR := $(REMOTE_HOME)/else-wer-pwa/dist
SERVER_DIR     := ..
PWA_DIR        := ../src/ui
TARGET         := aarch64-unknown-linux-gnu
SERVER_BIN     := $(SERVER_DIR)/target/$(TARGET)/release/else-wer

.PHONY: deploy build-server build-pwa deploy-server deploy-pwa remote-env install-unit install-udev logs status dev

deploy: deploy-pwa deploy-server

build-server:
	cd $(SERVER_DIR) && cross build --target $(TARGET) --release

build-pwa:
	cd $(PWA_DIR) && npm run build

deploy-server: build-server
	ssh $(PI) 'sudo systemctl stop else-wer.service || true'
	scp $(SERVER_BIN) $(PI):$(REMOTE_HOME)/
	ssh $(PI) 'sudo systemctl enable else-wer.service && sudo systemctl start else-wer.service'

# No service restart needed here: the server reads the pwa dist dir from disk on every
# request (tower_http::ServeDir), so a fresh rsync takes effect immediately.
deploy-pwa: build-pwa remote-env
	ssh $(PI) 'mkdir -p $(REMOTE_PWA_DIR)'
	rsync -avz --delete $(PWA_DIR)/dist/ $(PI):$(REMOTE_PWA_DIR)/

# Server defaults PWA_DIST_LOCATION to "src/ui/dist" relative to its working dir
# (/home/pi/), which doesn't resolve on the Pi. Point it at the rsynced dir instead, once.
remote-env:
	ssh $(PI) 'grep -q "^PWA_DIST_LOCATION=" $(REMOTE_HOME)/.env 2>/dev/null || echo "PWA_DIST_LOCATION=$(REMOTE_PWA_DIR)" >> $(REMOTE_HOME)/.env'

# Runs the server (debug build) and the PWA's vite dev server side by side for local dev.
# Vite proxies /api to localhost:3000 (see src/ui/vite.config.ts). Ctrl-C kills both.
dev:
	@trap 'kill 0' EXIT; \
	(cd $(SERVER_DIR) && cargo run) & \
	(cd $(PWA_DIR) && npm run dev) & \
	wait

install-unit:
	scp else-wer.service $(PI):/tmp/else-wer.service
	ssh $(PI) 'sudo mv /tmp/else-wer.service /etc/systemd/system/else-wer.service && sudo systemctl daemon-reload'

# Auto-remount the library drive after a USB drop, which restarts else-wer with it.
# See the comments in the rules file. Independent of install-unit, but pointless
# without it: the rule starts the mount, the unit's WantedBy= starts the server.
install-udev:
	scp 99-else-wer-library-drive.rules $(PI):/tmp/99-else-wer-library-drive.rules
	ssh $(PI) 'sudo install -o root -g root -m 644 /tmp/99-else-wer-library-drive.rules \
		/etc/udev/rules.d/99-else-wer-library-drive.rules \
		&& rm -f /tmp/99-else-wer-library-drive.rules \
		&& sudo udevadm control --reload-rules'

logs:
	ssh $(PI) 'journalctl -u else-wer.service -f'

status:
	ssh $(PI) 'systemctl status else-wer.service --no-pager'
