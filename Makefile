# else-wer-server: one entry point for local dev, the Pi (native/systemd) and poochi (docker).
# `make help` lists targets. Hosts/paths are overridable, e.g. `make pi-deploy PI_HOST=pi2`.

PI_HOST         ?= pi
PI_HOME         ?= /home/pi
PI_PWA_DIR      ?= $(PI_HOME)/else-wer-pwa/dist
PI_TARGET       ?= aarch64-unknown-linux-gnu
PI_BIN          := target/$(PI_TARGET)/release/else-wer

POOCHI_HOST     ?= poochi
POOCHI_HOSTNAME ?= poochi
POOCHI_DIR      ?= ~/projects/else-wer-server
IMAGE           ?= else-wer:latest

PWA_DIR := src/ui

# Real (gitignored) env files: seeded by env-init, preserved across `make pull`.
ENV_FILES := .env .env.local .env.docker .env.pi deploy/pi/.env deploy/poochi/.env

IS_POOCHI := $(filter $(POOCHI_HOSTNAME),$(shell uname -n))

POOCHI_COMPOSE = docker compose --env-file deploy/poochi/compose.env \
	-f docker-compose.yml -f deploy/poochi/docker-compose.yml \
	$(if $(TRAEFIK),-f deploy/poochi/docker-compose.traefik.yml)

# $(call poochi,<cmd>): run <cmd> here when this is poochi, otherwise re-run the same
# target on poochi over ssh (uses poochi's checkout at POOCHI_DIR).
poochi = $(if $(IS_POOCHI),$(1),ssh -t $(POOCHI_HOST) 'cd $(POOCHI_DIR) && make $@ $(if $(TRAEFIK),TRAEFIK=1)')

.PHONY: help dev dev-server dev-pwa pwa-build env-init db-truncate pull \
	pi-deploy pi-build-server pi-deploy-server pi-deploy-pwa pi-env pi-install-unit pi-install-udev pi-logs pi-status \
	poochi-deploy poochi-up poochi-down poochi-ps poochi-logs

help: ## list targets
	@grep -hE '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-18s %s\n", $$1, $$2}'

# ---- local (native) ---------------------------------------------------------

# Vite proxies /api to localhost:3000 (see src/ui/vite.config.ts). Ctrl-C kills both.
dev: ## cargo run + vite dev side by side
	@trap 'kill 0' EXIT; \
	cargo run & \
	(cd $(PWA_DIR) && npm run dev) & \
	wait

dev-server: ## cargo run only
	cargo run

dev-pwa: ## vite dev only
	cd $(PWA_DIR) && npm run dev

pwa-build: ## build the PWA into src/ui/dist
	cd $(PWA_DIR) && npm run build

env-init: ## create missing env files from their .example (never overwrites), fresh JWT_SECRET
	@for pair in .env.example:.env .env.docker.example:.env.docker deploy/poochi/.env.example:deploy/poochi/.env; do \
		src=$${pair%%:*}; dst=$${pair##*:}; \
		if [ -e "$$dst" ]; then echo "keep    $$dst"; \
		else sed "s/^JWT_SECRET=.*/JWT_SECRET=$$(openssl rand -hex 32)/" "$$src" > "$$dst" && echo "created $$dst"; fi; \
	done

db-truncate: ## empty every table in DB (default ./else-wer.db)
	scripts/trunc.sh $(or $(DB),./else-wer.db)

# The env files used to be tracked; a plain `git pull` of the commit that untracked them
# deletes them from an older checkout (or aborts if they were edited locally). This backs
# them up, resets any still-tracked ones so the pull goes through, then puts every one back.
pull: ## git pull that keeps local env files (backup in ~/.else-wer-env-backup)
	@bk=$$HOME/.else-wer-env-backup/$$(date +%Y%m%d-%H%M%S); \
	for f in $(ENV_FILES); do if [ -e $$f ]; then mkdir -p $$bk/$$(dirname $$f) && cp -p $$f $$bk/$$f; fi; \
		git ls-files --error-unmatch $$f >/dev/null 2>&1 && git checkout -q -- $$f; done; \
	git pull; rc=$$?; \
	for f in $(ENV_FILES); do if [ -e $$bk/$$f ] && ! cmp -s $$bk/$$f $$f; then cp -p $$bk/$$f $$f && echo "restored $$f"; fi; done; \
	echo "env backup: $$bk"; exit $$rc

# ---- pi (native binary + systemd; see deploy/pi/README.md) ------------------

pi-deploy: pi-deploy-pwa pi-deploy-server ## build + ship server and PWA to the Pi

pi-build-server: ## cross-compile the server for the Pi
	cross build --target $(PI_TARGET) --release

pi-deploy-server: pi-build-server
	ssh $(PI_HOST) 'sudo systemctl stop else-wer.service || true'
	scp $(PI_BIN) $(PI_HOST):$(PI_HOME)/
	ssh $(PI_HOST) 'sudo systemctl enable else-wer.service && sudo systemctl start else-wer.service'

# No service restart needed here: the server reads the pwa dist dir from disk on every
# request (tower_http::ServeDir), so a fresh rsync takes effect immediately.
pi-deploy-pwa: pwa-build pi-env
	ssh $(PI_HOST) 'mkdir -p $(PI_PWA_DIR)'
	rsync -avz --delete $(PWA_DIR)/dist/ $(PI_HOST):$(PI_PWA_DIR)/

# Pushes deploy/pi/.env (or the .example with a fresh JWT_SECRET) only if the Pi has none.
pi-env: ## seed /home/pi/.env if missing (never overwrites)
	@if ssh $(PI_HOST) 'test -e $(PI_HOME)/.env'; then echo "keep    $(PI_HOST):$(PI_HOME)/.env"; \
	else sed "s/^JWT_SECRET=$$/JWT_SECRET=$$(openssl rand -hex 32)/" $(or $(wildcard deploy/pi/.env),deploy/pi/.env.example) \
		| ssh $(PI_HOST) 'cat > $(PI_HOME)/.env' && echo "created $(PI_HOST):$(PI_HOME)/.env"; fi

pi-install-unit: ## install the systemd unit (one-time)
	scp deploy/pi/else-wer.service $(PI_HOST):/tmp/else-wer.service
	ssh $(PI_HOST) 'sudo mv /tmp/else-wer.service /etc/systemd/system/else-wer.service && sudo systemctl daemon-reload'

# Auto-remount the library drive after a USB drop, which restarts else-wer with it.
pi-install-udev: ## install the library-drive udev rule (one-time)
	scp deploy/pi/99-else-wer-library-drive.rules $(PI_HOST):/tmp/99-else-wer-library-drive.rules
	ssh $(PI_HOST) 'sudo install -o root -g root -m 644 /tmp/99-else-wer-library-drive.rules \
		/etc/udev/rules.d/99-else-wer-library-drive.rules \
		&& rm -f /tmp/99-else-wer-library-drive.rules \
		&& sudo udevadm control --reload-rules'

pi-logs: ## follow the service journal
	ssh $(PI_HOST) 'journalctl -u else-wer.service -f'

pi-status:
	ssh $(PI_HOST) 'systemctl status else-wer.service --no-pager'

# ---- poochi (docker; see deploy/poochi/README.md) ---------------------------
# Every poochi-* target works from poochi itself or from any other machine (via ssh).

# On poochi: build from this checkout. Elsewhere: build the image from THIS working tree
# (same x86_64 arch), stream it over ssh, and restart without building there. The compose
# files and Makefile used on poochi still come from poochi's own checkout.
poochi-deploy: ## deploy to poochi (from poochi or from here)
ifeq ($(IS_POOCHI),)
	docker build -t $(IMAGE) .
	docker save $(IMAGE) | gzip | ssh $(POOCHI_HOST) 'gunzip | docker load'
	ssh $(POOCHI_HOST) 'cd $(POOCHI_DIR) && make poochi-up NO_BUILD=1 $(if $(TRAEFIK),TRAEFIK=1)'
else
	$(MAKE) poochi-up
endif

poochi-up: ## (re)start on poochi, building from poochi's checkout; TRAEFIK=1 adds the overlay
	$(call poochi,mkdir -p deploy/poochi/data/covers deploy/poochi/data/creds deploy/poochi/logs \
		&& $(POOCHI_COMPOSE) up -d $(if $(NO_BUILD),--no-build,--build))

poochi-down:
	$(call poochi,$(POOCHI_COMPOSE) down)

poochi-ps:
	$(call poochi,$(POOCHI_COMPOSE) ps)

poochi-logs:
	$(call poochi,$(POOCHI_COMPOSE) logs -f)
