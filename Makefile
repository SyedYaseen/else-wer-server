# else-wer-server: one entry point for local dev, docker deploys and the Pi.
# `make help` lists targets. Machine-specific settings go in .deploy.env (gitignored, see
# .deploy.env.example); any of them can also be overridden on the command line.
-include .deploy.env

DOCKER_SSH       ?=
DOCKER_SSH_DIR   ?= ~/else-wer
IMAGE            ?= else-wer:latest

PI_HOST          ?= pi
PI_USER          ?= pi
PI_HOME          ?= /home/$(PI_USER)
PI_LIBRARY_MOUNT ?= $(PI_HOME)/drv
PI_LIBRARY_UUID  ?=
PI_TARGET        ?= aarch64-unknown-linux-gnu
PI_PWA_DIR       ?= $(PI_HOME)/else-wer-pwa/dist
PI_BIN           := target/$(PI_TARGET)/release/else-wer

PWA_DIR    := src/ui
DOCKER_DIR := deploy/docker
TRAEFIK    ?= 1
DC_FILES   := docker-compose.yml $(if $(filter 1,$(TRAEFIK)),docker-compose.traefik.yml)

# Real (gitignored) env files: seeded by env-init, preserved across `make pull`.
ENV_FILES := .env .env.local .env.docker .env.pi .deploy.env deploy/pi/.env deploy/docker/.env deploy/poochi/.env

# $(call compose,<args>): `docker compose <args>` for the docker deploy. Runs in deploy/docker
# here or, with DOCKER_SSH set, on that host against the .env in DOCKER_SSH_DIR. This
# checkout's compose files are shipped over each time; the remote's own copies are never used.
compose = $(if $(DOCKER_SSH),\
	tar cz -C $(DOCKER_DIR) $(DC_FILES) mkdirs.sh | ssh $(DOCKER_SSH) \
	't=$$(mktemp -d) && tar xz -C $$t && cd $(DOCKER_SSH_DIR) && sh $$t/mkdirs.sh \
	&& docker compose --project-directory . $(foreach f,$(DC_FILES),-f $$t/$(f)) $(1); r=$$?; rm -rf $$t; exit $$r',\
	cd $(DOCKER_DIR) && sh mkdirs.sh && docker compose $(foreach f,$(DC_FILES),-f $(f)) $(1))

# Renders the deploy/pi templates for this Pi. MOUNT_UNIT is resolved on the Pi itself.
PI_SED = sed -e 's\#@PI_USER@\#$(PI_USER)\#g' -e 's\#@PI_HOME@\#$(PI_HOME)\#g' \
	-e 's\#@PI_LIBRARY_MOUNT@\#$(PI_LIBRARY_MOUNT)\#g' -e 's\#@PI_LIBRARY_UUID@\#$(PI_LIBRARY_UUID)\#g' \
	-e "s\#@MOUNT_UNIT@\#$$(ssh $(PI_HOST) systemd-escape -p --suffix=mount $(PI_LIBRARY_MOUNT))\#g"

.PHONY: help dev dev-server dev-pwa pwa-build env-init db-truncate pull \
	docker-deploy docker-up docker-down docker-ps docker-logs docker-config \
	pi-deploy pi-build-server pi-deploy-server pi-deploy-pwa pi-env pi-install-unit pi-install-udev pi-logs pi-status

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
	@for pair in .env.example:.env deploy/docker/.env.example:deploy/docker/.env .deploy.env.example:.deploy.env; do \
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

# ---- docker (x86_64; see deploy/docker/README.md) ---------------------------
# Here by default; on another host when DOCKER_SSH is set. Traefik overlay on by default; TRAEFIK=0 skips it.

docker-deploy: ## rebuild + restart (with DOCKER_SSH: build here, ship the image over ssh)
ifeq ($(DOCKER_SSH),)
	$(call compose,up -d --build)
else
	docker build -t $(IMAGE) .
	docker save $(IMAGE) | gzip | ssh $(DOCKER_SSH) 'gunzip | docker load'
	$(call compose,up -d --no-build)
endif

docker-up: ## start (builds only if the image is missing)
	$(call compose,up -d)

docker-down:
	$(call compose,down)

docker-ps:
	$(call compose,ps)

docker-logs:
	$(call compose,logs -f)

docker-config: ## print the resolved compose config
	$(call compose,config)

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

# Pushes deploy/pi/.env (or the rendered .example with a fresh JWT_SECRET) only if the Pi has none.
pi-env: ## seed the Pi's ~/.env if missing (never overwrites)
	@if ssh $(PI_HOST) 'test -e $(PI_HOME)/.env'; then echo "keep    $(PI_HOST):$(PI_HOME)/.env"; \
	else $(PI_SED) -e "s/^JWT_SECRET=$$/JWT_SECRET=$$(openssl rand -hex 32)/" $(or $(wildcard deploy/pi/.env),deploy/pi/.env.example) \
		| ssh $(PI_HOST) 'cat > $(PI_HOME)/.env' && echo "created $(PI_HOST):$(PI_HOME)/.env"; fi

pi-install-unit: ## install the systemd unit (one-time)
	$(PI_SED) deploy/pi/else-wer.service | ssh $(PI_HOST) 'cat > /tmp/else-wer.service'
	ssh $(PI_HOST) 'sudo mv /tmp/else-wer.service /etc/systemd/system/else-wer.service && sudo systemctl daemon-reload'

# Auto-remount the library drive after a USB drop, which restarts else-wer with it.
pi-install-udev: ## install the library-drive udev rule (one-time; needs PI_LIBRARY_UUID)
ifeq ($(PI_LIBRARY_UUID),)
	@echo "PI_LIBRARY_UUID is not set (see .deploy.env.example); skipping the udev rule"
else
	$(PI_SED) deploy/pi/99-else-wer-library-drive.rules | ssh $(PI_HOST) 'cat > /tmp/99-else-wer-library-drive.rules'
	ssh $(PI_HOST) 'sudo install -o root -g root -m 644 /tmp/99-else-wer-library-drive.rules \
		/etc/udev/rules.d/99-else-wer-library-drive.rules \
		&& rm -f /tmp/99-else-wer-library-drive.rules \
		&& sudo udevadm control --reload-rules'
endif

pi-logs: ## follow the service journal
	ssh $(PI_HOST) 'journalctl -u else-wer.service -f'

pi-status:
	ssh $(PI_HOST) 'systemctl status else-wer.service --no-pager'
