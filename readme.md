# Phase 1 Technical Plan (Rust server on Pi Zero)
## File Scanning
- Use walkdir or ignore crate to recursively list files.
- Extract metadata with symphonia or lofty (for ID3/tags).

## API Layer
- Use axum or actix-web (Axum is lighter and async-friendly).
- Expose endpoints like:
    - GET /audiobooks → returns list with metadata
    - GET /audiobooks/:id/download → returns file
    - POST /sync → accepts playback progress and stores per-user/book

## State Storage
- Use sled, sqlite, or even simple JSON files (for now).
- Store:
    - User playback positions
    - File index/cache

## Proxy/VPN
- Setup Tailscale or WireGuard for secure remote access to the Pi.
- Or add NGINX + basic auth over HTTPS.

## React Native App - Android
- Browse + download files
- Play audio locally
- Sync position (e.g. every minute or on pause/stop)


# Phase 2, we can optimize for:
- Streaming with HTTP range requests
- Playlist/queue management
- Optional transcode (with gstreamer or ffmpeg) for low bandwidth

# Phase 3
- Include books
- Ability to sync progress between audio and epub

Folder structure init:
audiobookshelf-rs/
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── lib.rs
│   ├── api/
│   │   ├── mod.rs
│   │   ├── audiobooks.rs
│   │   └── sync.rs
│   ├── services/
│   │   ├── mod.rs
│   │   ├── scanner.rs
│   │   └── sync.rs
│   ├── models/
│   │   ├── mod.rs
│   │   ├── audiobook.rs
│   │   └── progress.rs
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── database.rs     # SQLite or sled wrapper
│   │   └── fs.rs           # File-related helpers
│   └── utils.rs
├── .env
├── Cargo.toml
└── README.md


## Test curl
curl localhost:3000/api/scan_files

curl -X POST http://localhost:3000/api/update_progress \
  -H 'content-type: application/json' \
  -d '{
    "user_id": 1,
    "book_id": 7,
    "file_id": 6,
    "progress_time_marker": 10000
  }' -i

curl localhost:3000/api/file_metadata/1

curl localhost:3000/api/get_progress/1/7/6

curl -X POST http://localhost:3000/api/create_user \
  -H "Content-Type: application/json" \
  -d '{"username": "valerie", "password": "mypassword"}'

  curl -X POST http://192.168.1.3:3000/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "valerie", "password": "mypassword"}'

http://192.168.1.3:3000/api/login valerie mypassword