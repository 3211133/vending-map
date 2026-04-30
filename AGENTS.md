# Repository Guidelines

## Project Structure & Module Organization
`backend/` contains the Rust API service. Keep HTTP entrypoints in `backend/src/handlers/`, data structs in `backend/src/models/`, database access in `backend/src/repositories/`, and external integration logic in `backend/src/services/`. `frontend/` is a static Leaflet client with `index.html`, `js/app.js`, and `css/style.css`. SQL schema changes live in `migrations/` as ordered files such as `004_create_import_logs.sql`. Docker assets are in `docker/`.

## Build, Test, and Development Commands
Run backend commands from `backend/`.

- `cargo run`: starts the API on `SERVER_HOST:SERVER_PORT` after loading `.env`.
- `cargo check`: fast compile pass for day-to-day validation.
- `cargo test`: runs Rust tests; add tests with new backend behavior.
- `cargo fmt`: applies standard Rust formatting.
- `cargo clippy --all-targets --all-features -D warnings`: catches lint issues before review.

The frontend is static. Serve `frontend/` with any simple file server, for example `python3 -m http.server 3000` from `frontend/`, then proxy `/api` to the backend or use the provided nginx config in `docker/nginx.conf`.

## Coding Style & Naming Conventions
Use Rust 2021 defaults: 4-space indentation, `snake_case` for functions and modules, `PascalCase` for structs and enums, and one responsibility per module. Keep handler functions thin and push SQL into repositories. Match existing frontend style: plain JavaScript, `const` by default, camelCase function names, and descriptive DOM id names such as `importButton`.

## Testing Guidelines
There is no committed test suite yet, so new backend features should add focused unit or integration tests near the changed code or under `backend/tests/`. Prefer names that describe behavior, for example `create_vote_rejects_duplicate_daily_vote`. At minimum, run `cargo test`, `cargo check`, and a manual `/health` smoke test against a local database before opening a PR.

## Commit & Pull Request Guidelines
Current history uses short imperative subjects with an optional prefix before a colon, for example `Initial: add DESIGN.md and directory structure`. Follow that pattern and keep commits scoped to one change. PRs should include a concise description, linked issue if available, setup or migration notes, and screenshots or short recordings for frontend changes.

## Configuration & Data Notes
The backend requires `DATABASE_URL` and `DAILY_SECRET`; `SERVER_HOST`, `SERVER_PORT`, `RUST_LOG`, and `OVERPASS_API_URL` are optional. Never commit secrets or local `.env` files. When changing schema, add a new numbered migration instead of editing an applied one.
