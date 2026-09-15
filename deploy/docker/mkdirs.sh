#!/bin/sh
# Run from the directory holding the deploy's .env (the Makefile does this). A bind mount
# masks the image's build-time mkdirs, so when DATA_DIR / LOGS_DIR are host paths they must
# exist before the container starts, owned by PUID:PGID. Named volumes need nothing.
set -e
[ -f .env ] || { echo "no .env in $(pwd) - copy .env.example to .env and set JWT_SECRET" >&2; exit 1; }
set -a; . ./.env; set +a
case "${DATA_DIR:-}" in */*) mkdir -p "$DATA_DIR/covers" "$DATA_DIR/creds" ;; esac
case "${LOGS_DIR:-}" in */*) mkdir -p "$LOGS_DIR" ;; esac
