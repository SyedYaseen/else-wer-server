# Handoff: public demo server for Google Play review

## Goal
Google Play reviewers must be able to sign in to the else-wer Android app. The app is only a
client, so reviewers need an else-wer server they can reach from the internet, running all the
time, with a non-admin reviewer account and public-domain audiobooks. You do this with
Docker plus a **Cloudflare Tunnel**, which gives HTTPS without opening router ports.

End state to report back:
- a public HTTPS URL, e.g. `https://elsewer-demo.<domain>`, serving the API
- reviewer username and password (non-admin, doesn't expire)
- default `admin/admin` password changed

## Context you won't find in the repo
- Reviewers type the URL into the app's "Server address" field. The app appends `/api`
  itself (`else-wer-app/app/login.tsx`), so give them the **bare origin, no `/api`, no
  trailing slash**.
- Use HTTPS. Android blocks cleartext `http://` by default.
- On first boot the server creates **`admin` / `admin`** (`src/services/startup.rs`,
  `ensure_admin_user`). This URL will be public, so **change that password before starting the tunnel**.
- The first boot also creates a "Default" library pointing at `/audiobooks` (inside the
  container), which is `AUDIOBOOKS_DIR` on the host.
- Content has to be **public domain** (LibriVox), because Google reviews the app with it.
- A quick tunnel (`cloudflared tunnel --url ...`) gets a random URL that **changes on restart**,
  so it's no good for Play. You need a **named tunnel** on a domain in Cloudflare.
- The owner doesn't want any commits made on their behalf. Leave changes uncommitted.

## Deployed on poochi (2026-09-15)
Status: live at `https://elsewer-demo.syedyaseen.dev`. External HTTPS checks pass (health, reviewer login, book
list, chapter stream 206, PWA). Still to do: the release-build phone test on mobile data and the Play Console entry.

On poochi, `deploy/docker/.env`, host :3030 and `else-wer:latest` belong to the **personal**
instance, so `make env-init && make docker-deploy` would rebuild and restart *that* one. The demo is a
separate compose project that lives outside the repo, in `~/elsewer-demo/`:
- `.env`: its own `JWT_SECRET`, project `elsewer-demo`, container `else-wer-demo`, `127.0.0.1:3031`
  (loopback only, because public traffic comes in through the tunnel). `data/`, `logs/` and `audiobooks/` also live here.
- `compose.demo.yml`: builds `else-wer:demo` from this checkout and adds a `cloudflared` container.
- `tunnel.env`: `TUNNEL_TOKEN` for a dashboard-managed tunnel (Zero Trust → Networks → Tunnels →
  `elsewer-demo`, public hostname `elsewer-demo.syedyaseen.dev` → HTTP `else-wer-demo:3000`).
- `CREDENTIALS` (mode 600): the admin and reviewer passwords.

```sh
cd ~/elsewer-demo
dc() { docker compose --project-directory . -f ~/projects/else-wer-server/deploy/docker/docker-compose.yml -f compose.demo.yml "$@"; }
dc up -d --build     # update the demo to the current checkout
dc ps; dc logs -f
```
The reviewer's username has a random suffix. That's because login lockout is keyed on peer IP + username
(`src/api/user.rs` `login`, `src/api/rate_limit.rs`), and behind the tunnel every request shares one
peer IP. With a guessable username, anyone could lock the reviewer out with 5 bad passwords.

## Steps

### 1. Server (Docker)
```bash
git clone <else-wer-server repo> && cd else-wer-server
make env-init                      # creates deploy/docker/.env with a fresh JWT_SECRET
mkdir -p audiobooks                # default AUDIOBOOKS_DIR = repo-root audiobooks/
```
Add 2–3 LibriVox books to `audiobooks/`, one folder per book (the scan is folder-based),
e.g. `audiobooks/Pride and Prejudice/*.mp3`. Get them from https://librivox.org (the
"Download" zip for each book).

```bash
make docker-deploy                 # = cd deploy/docker && docker compose up -d --build
curl -s http://localhost:3000/api/health
```

### 2. Lock down admin and create the reviewer (still local only)
```bash
B=http://localhost:3000/api
TOKEN=$(curl -s -X POST $B/login -H 'content-type: application/json' \
  -d '{"username":"admin","password":"admin"}' | jq -r '.token // .access_token')
# check the login response shape if TOKEN is empty: see src/api/user.rs `login`

ADMIN_PW=$(openssl rand -base64 18)
curl -s -X PUT $B/user/change_password -H "authorization: Bearer $TOKEN" \
  -H 'content-type: application/json' -d "{\"username\":\"admin\",\"new_password\":\"$ADMIN_PW\"}"

REV_PW=$(openssl rand -base64 15)
curl -s -X POST $B/create_user -H "authorization: Bearer $TOKEN" -H 'content-type: application/json' \
  -d "{\"username\":\"playreviewer\",\"password\":\"$REV_PW\",\"is_admin\":false,\"can_organize\":false}"

curl -s -X POST $B/libraries/1/scan -H "authorization: Bearer $TOKEN"   # index the books
echo "admin: $ADMIN_PW   playreviewer: $REV_PW"   # save both in a password manager
```
If a payload is rejected, read the handler in `src/api/user.rs` / `src/api/libraries.rs`
for the exact fields. You can also do all of this in the PWA at http://localhost:3000.

Check: log in as `playreviewer` in the PWA and play a chapter.

### 3. Cloudflare named tunnel
```bash
# install cloudflared (Arch: pacman -S cloudflared; Debian: Cloudflare's apt repo)
cloudflared tunnel login                         # browser auth, pick the domain
cloudflared tunnel create elsewer-demo
cloudflared tunnel route dns elsewer-demo elsewer-demo.<domain>
```
`~/.cloudflared/config.yml`:
```yaml
tunnel: elsewer-demo
credentials-file: /root/.cloudflared/<TUNNEL_ID>.json   # path printed by `create`
ingress:
  - hostname: elsewer-demo.<domain>
    service: http://localhost:3000
  - service: http_status:404
```
Make it survive reboots:
```bash
sudo cloudflared --config ~/.cloudflared/config.yml service install
sudo systemctl enable --now cloudflared
```
The container already has `restart: unless-stopped`. Make sure Docker itself starts on
boot (`systemctl enable docker`).

In Cloudflare, don't put Cloudflare Access or "Under Attack" mode on this hostname.
Reviewers can't get past a challenge page, and the app can't either.

### 4. Verify from outside
```bash
curl -s https://elsewer-demo.<domain>/api/health
curl -s -X POST https://elsewer-demo.<domain>/api/login -H 'content-type: application/json' \
  -d "{\"username\":\"playreviewer\",\"password\":\"$REV_PW\"}"
```
Test from a phone on mobile data, not Wi-Fi, with the **release** app build: server
`https://elsewer-demo.<domain>`, reviewer credentials, play a book, seek, move to the next chapter.

### 5. Hand back to the owner
Report the URL, reviewer username and reviewer password. The owner pastes them into Play
Console → App content → App access:
- Username: `playreviewer` / Password: `<REV_PW>`
- Any other information:
  > This app is a client for a self-hosted audiobook server. On the sign-in screen, enter
  > https://elsewer-demo.<domain> in the "Server address" field, then enter the username and
  > password above and tap "Sign in". The demo server is online 24/7 and contains
  > public-domain audiobooks from LibriVox. No other verification is required.

## TODO
- [x] 1. Docker server running, LibriVox books in `audiobooks/` (poochi: `~/elsewer-demo`, 3 books / 61 files)
- [x] 2. admin password changed, reviewer created (non-admin; poochi: `reviewer-cb6f2b`), library scanned
- [x] 3. named Cloudflare tunnel + DNS + cloudflared (poochi: dashboard-managed tunnel, `cloudflared` container in
      `~/elsewer-demo`, tunnel `634c7aac-34ce-4100-80dd-4876b4c32415`)
- [ ] 4. external checks: curl over HTTPS ✅ (2026-09-15) + phone on mobile data with the release build (pending)
- [x] 5. URL and credentials reported to the owner (`~/elsewer-demo/CREDENTIALS`)

## Ongoing
Play uses these credentials **for every update review**. Keep the box, tunnel and account
up. If the URL or password changes, update Play Console *before* submitting the next build.
