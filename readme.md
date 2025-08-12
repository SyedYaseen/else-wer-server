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
- Option to organize the books folder based on book/ series/ author data, so the user doesnt have to do it

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
    "book_id": 5,
    "file_id": 21,
    "progress_ms": 119720.00122070312,
    "complete": false
  }' -i



  <!-- 1 5 21 119720.00122070312 false -->

curl localhost:3000/api/file_metadata/1

curl localhost:3000/api/get_progress/1/7/6



http://192.168.1.3:3000/api/login valerie mypassword

curl localhost:3000/api/covers//app/static/covers/elder_race_[2021].jpg

User
curl -X POST http://localhost:3000/api/create_user \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOjIsInJvbGUiOiJ1c2VyIiwidXNlcm5hbWUiOiJ2YWxlcmllIiwiZXhwIjoxNzU0OTg5MjcyLCJpYXQiOjE3NTQ5MDI4NzJ9.maP1PrYux61oX7TxzSFJEC8UbIWLhEz7g8UCAE0vMOo" \
  -d '{"username": "valerie", "password": "mypassword", "is_admin": false}'

  curl -X POST http://192.168.1.3:3000/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "valerie", "password": "mypassword"}'


Admin
curl -X POST http://localhost:3000/api/create_user \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin", "is_admin": true}'

  curl -X POST http://192.168.1.3:3000/api/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin"}'

valerie
curl -X GET http://localhost:3000/api/hello \
-H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOjIsInJvbGUiOiJ1c2VyIiwidXNlcm5hbWUiOiJ2YWxlcmllIiwiZXhwIjoxNzU0OTg5MjcyLCJpYXQiOjE3NTQ5MDI4NzJ9.maP1PrYux61oX7TxzSFJEC8UbIWLhEz7g8UCAE0vMOo"

admin
curl -X GET http://localhost:3000/api/hello \
-H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOjEsInJvbGUiOiJhZG1pbiIsInVzZXJuYW1lIjoiYWRtaW4iLCJleHAiOjE3NTQ5OTI2NzYsImlhdCI6MTc1NDkwNjI3Nn0.vhTRmbui7hWIFc2BhADMc9YHP1FjcYkCpgBbR3J-dS8"

// Wont work because no admin access
curl -X POST http://localhost:3000/api/create_user \
-H "Content-Type: application/json" \
-H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOjIsInJvbGUiOiJ1c2VyIiwidXNlcm5hbWUiOiJ2YWxlcmllIiwiZXhwIjoxNzU0OTg5MjcyLCJpYXQiOjE3NTQ5MDI4NzJ9.maP1PrYux61oX7TxzSFJEC8UbIWLhEz7g8UCAE0vMOo" \
-d '{"username": "test1", "password": "mypassword", "is_admin": false}'



curl -X POST http://localhost:3000/api/create_user \
-H "Content-Type: application/json" \
-H "Authorization: Bearer eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJzdWIiOjEsInJvbGUiOiJhZG1pbiIsInVzZXJuYW1lIjoiYWRtaW4iLCJleHAiOjE3NTQ5OTI2NzYsImlhdCI6MTc1NDkwNjI3Nn0.vhTRmbui7hWIFc2BhADMc9YHP1FjcYkCpgBbR3J-dS8" \
-d '{"username": "test1", "password": "mypassword", "is_admin": false}'

cross run --release --target armv7-unknown-linux-gnueabihf
cross run --release --target aarch64-unknown-linux-gnu
scp target/armv7-unknown-linux-gnueabihf/release/rustybookshelf yaseen@192.168.1.12:/home/yaseen/rustybookshelf

Works
cross build --target armv7-unknown-linux-musleabihf --release
scp target/armv7-unknown-linux-musleabihf/release/rustybookshelf yaseen@192.168.1.12:/home/yaseen/
