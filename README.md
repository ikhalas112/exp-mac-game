# Snake Game

Simple desktop Snake game built with Rust, Dioxus Desktop, and a Rust Axum backend.

## Project Structure

```text
.
├── crates
│   ├── desktop   # Dioxus desktop app
│   ├── server    # Axum backend API
│   └── shared    # Shared game logic, constants, and API types
└── Cargo.toml    # Rust workspace
```

## Requirements

- Rust toolchain with Cargo
- Docker and Docker Compose for Postgres
- macOS desktop runtime support for Dioxus/Wry

## Run

Start Postgres first:

```bash
docker compose up -d
```

Create a local server env file:

```bash
cp crates/server/.env.example crates/server/.env
```

Server database settings are read from `crates/server/.env`:

```env
DATABASE_HOST=127.0.0.1
DATABASE_PORT=5432
POSTGRES_USER=snake
POSTGRES_PASSWORD=snake
POSTGRES_DB=snake
```

The server reads `POSTGRES_USER`, `POSTGRES_PASSWORD`, and `POSTGRES_DB` as the database username, password, and database name. `DATABASE_USER`, `DATABASE_PASSWORD`, and `DATABASE_NAME` can be set as server-only aliases if needed. `DATABASE_URL` can also be set as an override.

Open two terminals from the project root.

Terminal 1: start the backend.

```bash
cargo run -p snake-server
```

The backend runs migrations on startup and listens on:

```text
http://127.0.0.1:3001
```

Terminal 2: start the desktop app.

```bash
cargo run -p snake-desktop
```

The desktop app loads configuration from `crates/desktop/.env` first, then from a root `.env` if present. Create a local env file from the example:

```bash
cp crates/desktop/.env.example crates/desktop/.env
```

Supported desktop env values:

```env
BACKEND_URL=http://127.0.0.1:3001
PLAYER_NAME=Player
```

`BACKEND_URL` defaults to `http://127.0.0.1:3001`. `PLAYER_NAME` is used only as the default value in the player name field.

## Gameplay

- Enter a player name and press `Start`.
- Use arrow keys or `WASD` to move.
- Press `Space` to start or restart.
- Eat food to gain `10` points.
- The game ends when the snake hits a wall or itself.
- Scores are submitted to the backend when the game ends.
- The backend stores only each player's all-time high score.
- The leaderboard shows the top 10 scores from Postgres.

## Backend API

The backend stores users and all-time high scores in Postgres. Each user has at most one row in `scores`.

### Health Check

```http
GET /health
```

Returns:

```json
{ "status": "ok" }
```

### Get Or Create User

```http
POST /users
Content-Type: application/json

{
  "name": "Player"
}
```

Returns the existing user when the name already exists, otherwise creates a new user.

### List Scores

```http
GET /scores
```

Returns the top 10 scores, sorted from highest to lowest.

### Submit Score

```http
POST /scores
Content-Type: application/json

{
  "user_id": "00000000-0000-0000-0000-000000000000",
  "score": 120
}
```

Stores the score only when it is the user's first score or higher than their saved all-time high. Returns the updated top 10 scores.

## Development

Format Rust code:

```bash
cargo fmt --all
```

Check the full workspace:

```bash
cargo check --workspace
```

Run tests:

```bash
cargo test --workspace
```

## Build Clickable Desktop App

The desktop app still needs the backend and Postgres running. For a local packaged build, start the database and backend first:

```bash
docker compose up -d
cargo run -p snake-server
```

Then build the desktop app on the target operating system. Dioxus desktop bundles are native-platform builds, so build macOS packages on macOS and Windows packages on Windows.

### macOS

Install the Dioxus CLI if needed:

```bash
cargo install dioxus-cli
```

Build a clickable `.app` bundle:

```bash
dx bundle --desktop --package snake-desktop
```

The generated app will be under:

```text
target/dx/snake-desktop/bundle/macos/macos/
```

Open the generated `.app` from Finder, or run it from Terminal with:

```bash
open target/dx/snake-desktop/bundle/macos/macos/*.app
```

For a plain release binary instead of a `.app` bundle:

```bash
cargo build -p snake-desktop --release
```

The binary is:

```text
target/release/snake-desktop
```

### Windows

Install Rust and the Dioxus CLI in PowerShell:

```powershell
cargo install dioxus-cli
```

Build a clickable Windows desktop bundle:

```powershell
dx bundle --desktop --package snake-desktop
```

The generated Windows bundle will be under:

```text
target\dx\snake-desktop\bundle\windows\
```

Open the generated `.exe` or installer from File Explorer.

For a plain release binary instead of a bundled installer:

```powershell
cargo build -p snake-desktop --release
```

The binary is:

```text
target\release\snake-desktop.exe
```

### Runtime Config For Built Apps

The desktop app reads `BACKEND_URL` from environment variables. If the backend is running on another host or port, set it before launching the app:

macOS:

```bash
BACKEND_URL=http://127.0.0.1:3001 open target/dx/snake-desktop/bundle/macos/macos/*.app
```

Windows PowerShell:

```powershell
$env:BACKEND_URL="http://127.0.0.1:3001"
.\target\release\snake-desktop.exe
```

## Important Notes

- `docker-compose.yml` runs only Postgres; backend and desktop are still separate Cargo commands.
- The desktop app requires backend connectivity to create or load a player before the game starts.
- Game rules and shared API structs live in `crates/shared`.
- The backend and desktop app are separate commands by design.

R2_ACCESS_KEY_ID=1cf28587c68569adfe1aad0e15ccf836
R2_SECRET_ACCESS_KEY=6bd29afa57155bd707575585aec76cc1a70f7fd1df6192bbe538f0fb3586917a
R2_BUCKET_NAME=democlient
R2_ENDPOINT=https://592194e15345507ed46142845f1a3235.r2.cloudflarestorage.com

• ตัวอย่าง tag สำหรับ env dev ใช้ suffix -dev หรือ -alpha ได้ เช่น:

git tag v0.1.0-dev
git push origin v0.1.0-dev

ถ้าต้องการ build number:

git tag v1.2.2-sit
git push origin v1.2.2-sit

หรือแบบ annotated tag:

git tag -a v0.1.0-dev.1 -m "Release v0.1.0 dev build 1"
git push origin v0.1.0-dev.1

Workflow จะ map เป็น:

v0.1.0-dev -> env=dev, channel=alpha
v0.1.0-dev.1 -> env=dev, channel=alpha
