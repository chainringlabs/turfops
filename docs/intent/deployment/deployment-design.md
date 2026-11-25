---
parent: high-level-design
prefix: DEPLOY
---

# Deployment, Configuration & Operations

## Context and Design Philosophy

TurfOps ships as one statically served binary in one container image that runs next to a PostgreSQL database, and those two are a complete installation: weather is fetched by the app and kept in that database, so nothing is mounted and no other service is needed. The repo provides the image, its configuration surface and an example Compose file, and each works for anyone who wants to run the project; the owner's own deployment is built separately and is out of scope here. This component owns everything between the source tree and a running, observable process: the image build, the example Compose stack, environment configuration, startup order, router assembly and middleware, the database pool and migration run, the HTTP error contract, the health endpoint, logging, and the CI / supply-chain gates that guard `main`.

Guiding principles:

- **Small, non-root, distroless runtime.** The runtime image carries the binary, the built SPA and the timezone database, and nothing else: no shell, no package manager, no library of its own.
- **The environment holds what must exist before the app starts; the app holds the rest.** The database connection, the listen address, tokens that guard a route, and the switches for optional services are environment variables with safe defaults. Choices about the lawn (its location, sources, sensors, forecast provider) are made in the app and stored in the database (setup and sources); `LAWN_*` variables and the Home Assistant and OpenWeatherMap credentials only seed a first run; no environment variable overrides a choice made in the app. Nothing optional can stop the app from starting. Only the database password is required (unset or empty refuses to start). A malformed `MCP_TOKEN`, `SENSOR_TOKEN` or `CORS_ALLOWED_ORIGIN` refuses to start rather than run weakly. Any other unusable value falls back to its default with a warning naming the setting.
- **Secrets never leave the process.** Config structs holding a credential redact it in `Debug`, error responses never echo internals, and `.env` files are never committed or copied into an image. A credential entered in the app is stored write-only (setup and sources). The OpenWeatherMap key, which must travel in the request URL, is stripped from every error its client builds.
- **One clock, in the lawn's timezone.** "Today" and every date gate in every component is the lawn's local date, read from the app's `Clock` (`backend/src/clock.rs`; no component reads `chrono::Local` or the system time to judge a date or an age). The zone is the one that follows from the lawn's location, and TurfOps has no timezone setting. A process on the wrong zone computes wrong dates, not just odd-looking ones, so startup logs the zone in force and where it came from.
- **Outbound HTTPS is the one thing the host must allow.** The app reaches NOAA, USDA, RCC-ACIS and Open-Meteo for weather; with none of them reachable it starts and serves the log, the shelf and whatever weather it already holds.
- **Graceful stop.** On SIGTERM the server finishes in-flight requests before exiting, so stopping or restarting the container does not cut a request off midway.
- **Gates before merge.** Format, clippy, tests, type check, lint and a supply-chain scan run on every PR; dependency updates arrive continuously through Renovate.

## Container Image (`Dockerfile`)

Three stages, every base image pinned by digest (Renovate maintains the digests).

| Stage | Base | Produces |
|---|---|---|
| `frontend-build` | `node:26-alpine` | `npm ci --ignore-scripts` then `npm run build` → `/app/frontend/dist` |
| `backend-build` | `rust:1.99-slim-trixie` | `cargo build --release` → `/app/target/release/turfops-backend` |
| runtime | `cgr.dev/chainguard/glibc-dynamic:latest@sha256:…` | `/app/turfops-backend`, `/app/static`, `/usr/share/zoneinfo` |

**No install scripts.** The frontend stage installs with `--ignore-scripts`, the same policy as the supply-chain scan, so no package's lifecycle script runs in the image build. `frontend/.npmrc` sets the same policy (`ignore-scripts=true`) for every other npm install, with `min-release-age=7` so npm resolves no version published in the last seven days; the stage copies it in before `npm ci`. No dependency in the lockfile declares an install script; one that genuinely needs it would be named in the Dockerfile with the reason.

**Native toolchain.** The build stage installs `tzdata` only so its files can be copied forward. No C++ compiler is needed.

**Dependency caching.** `Cargo.toml` / `Cargo.lock` are copied first and built against a placeholder `main.rs` (`cargo build --release 2>/dev/null || true`), so the dependency layer is reused until the manifests change. The real `src/` is then copied and `touch src/main.rs` forces the crate itself to rebuild.

**MCP resources.** `docs/timing-windows.md` and `docs/agronomy-methods.md` are copied to `/docs/` in the build stage. The MCP server embeds them with `include_str!("../../../docs/…")` from `backend/src/mcp/resources.rs`; from `/app/src/mcp/` that path resolves to `/docs/`, and from the repo checkout it resolves to the repo's `docs/`. Both copies must exist at compile time or the build fails. `.dockerignore`'s `*.md` is root-anchored, so it excludes only top-level Markdown and leaves `docs/*.md` in the context.

**The clock's zone.** The `Clock` is built after the database is reachable: in the lawn profile's `timezone` (an IANA name the profile derives from the location, resolved in the zone database compiled into the binary by `chrono-tz`), or, while the profile has no location, in the host's default zone (`Clock::System`). It lives in `AppState` behind a shared handle and is replaced whenever the owner changes the location. `/usr/share/zoneinfo` is copied from the build stage for `chrono::Local`, which `Clock::System` reads.

**One reading of the clock.** `Clock::now()` is the current instant and `Clock::today()` its local date; `Clock::moment()` takes both at once (`Moment { now, today }`), so a computation that needs "now" and "today" judges both on the same reading. Every age, staleness, expiry and date gate reads the app's `Clock`. `Clock::Frozen { at, zone }` is a clock stopped at one instant in one zone: neither a location nor the host default ever produces it; tests use it to run a whole request, feed included, on a chosen day. Infrastructure timestamps (`created_at`, cache ages) keep the real time. The image sets no `TZ` and the example Compose passes none: the zone comes from the location. `TZ` is not a TurfOps setting; like any process, the backend's host-default zone follows it, which matters only until a location is set. The clock dates window days, freeze nights, fungicide residual windows, the nitrogen budget's year, plan and calendar dates, and the local calendar days that hourly observations and forecast slots are bucketed into. Until a location is set the clock is the host's zone (UTC in the image). Nothing weather-dependent is shown in that state; the application log's follow-up reminders and its "this date is in the future" warning are judged on that zone, and the setup notice says dates use UTC until the location is set. The backend makes the clock visible instead of trusting it:

- At startup `main.rs` logs at INFO the zone, its offset, the local date and time, and whether it comes from the lawn's location or the host default (`Clock::describe`). The same line is logged when the clock is replaced.
- `GET /api/v1/today` (`backend/src/api/today.rs`) exposes the clock to the web UI as `{"today": "YYYY-MM-DD", "timezone": <IANA name or null>}`, with no database or data-source access, so forms judge "future" by the lawn's date rather than the viewer's (DEPLOY-TZ-006). It is its own route, not part of `/health`, which stays an operations probe.

**Runtime.** Binary and static files are copied `--chown=65532:65532`, and the image declares `USER 65532:65532` explicitly, so the process stays non-root even if the base image's default user changes. `ENV STATIC_DIR=/app/static`, `EXPOSE 3000`, `ENTRYPOINT ["./turfops-backend"]`.

**Health check.** The runtime has no shell or curl, so the image's `HEALTHCHECK` runs the binary itself: `turfops-backend --healthcheck` sends `GET /api/v1/health` to `127.0.0.1:$SERVER_PORT` with a 3 s timeout and exits 0 only on a 200 (`backend/src/health_probe.rs`). Interval 30 s, timeout 5 s, 3 retries; the 90 s start period covers the startup wait for PostgreSQL plus migrations. Living in the image, it applies to any deployment of it, not only Compose.

**Release profile** (`backend/Cargo.toml`): `strip = true`, `lto = true`, `codegen-units = 1`.

## Example Compose Stack (`docker-compose.yml`)

`docker-compose.yml` is an example for anyone who wants to try the project: it builds the image from source and runs it next to PostgreSQL 18. Those two services are the whole installation: no volume is mounted into the app. It assumes nothing about a particular host beyond the variables below.

| Service | Image / build | Ports | Notes |
|---|---|---|---|
| `db` | `postgres:18-alpine@sha256:…` | `127.0.0.1:5433:5432` | `POSTGRES_DB` / `POSTGRES_USER` = `turfops`; `POSTGRES_PASSWORD: ${DB_PASSWORD:?}`; volume `turfops_data:/var/lib/postgresql`; healthcheck `pg_isready -U turfops` every 5 s, 5 retries |
| `app` | `build: .` (Dockerfile) | `3000:3000` | `depends_on: db: condition: service_healthy`; no volumes |

Both services `restart: unless-stopped`. Host port 5433 exposes Postgres for a local `cargo run` against the Compose database (`README.md`, Local development); it is bound to the loopback interface, so other machines on the network cannot reach the database.

**Postgres data volume.** The PostgreSQL 18 image keeps its cluster in a version-specific directory (`/var/lib/postgresql/18/docker`) under the volume root `/var/lib/postgresql`. The named volume is mounted at that root, so a fresh `docker compose up` initializes the database and the data survives recreating the container.

**Required password.** `DB_PASSWORD` is read with `${DB_PASSWORD:?}`: Compose refuses to start when it is unset or empty, and the same value feeds both `POSTGRES_PASSWORD` and the app's `DATABASE_PASSWORD`. The bare `:?` form (no message) is deliberate: the ggshield pre-commit hook flags `${VAR:?message}` as a hard-coded password.

**Env passthrough.** Every optional key is passed as `${KEY:-}` or `${KEY:-default}`, so an unset host variable reaches the app as an **empty string**. Values come from the shell environment or from a `.env` file beside the Compose file, which Compose reads on its own and `.gitignore` keeps out of the repo. The app's "empty = not configured" rule exists for exactly this. `SERVER_HOST`, `SERVER_PORT` and `STATIC_DIR` are fixed in the file; `RUST_LOG` defaults to `info`.

**Pre-built image.** CI publishes the image to `ghcr.io/chainringlabs/turfops`; the README shows replacing `build:` with `image:` to run it without building.

## Configuration (`backend/src/config.rs`)

`Config::from_env()` reads everything once at startup. Two variables are read outside it: `STATIC_DIR` (`main.rs`) and `DB_MAX_CONNECTIONS` (`db/pool.rs`). "Compose" is the value the Compose file passes when the host leaves the variable unset.

| Variable | App default | Compose | Empty / invalid value |
|---|---|---|---|
| `DATABASE_HOST` / `_PORT` / `_NAME` / `_USER` | `localhost` / `5432` / `turfops` / `turfops` | `db` / `5432` / `turfops` / `turfops` | bad port → warn, 5432 |
| `DATABASE_PASSWORD` | **required** | `${DB_PASSWORD:?}` | unset or empty → `Config` error naming it, process exits |
| `DB_MAX_CONNECTIONS` | 10 | not passed | unparseable → warn, 10 |
| `SERVER_HOST` / `SERVER_PORT` | `0.0.0.0` / `3000` | fixed `0.0.0.0` / `3000` | bad host → warn, `0.0.0.0`; bad port → warn, 3000 |
| `STATIC_DIR` | `./static` | `/app/static` | missing `index.html` → API-only mode (warn) |
| `CORS_ALLOWED_ORIGIN` | unset → `http://localhost:<port>` | `""` | empty = unset; `*` = permissive (warn); an unparseable origin **panics** at startup |
| `HA_URL` / `HA_TOKEN` | unset | `""` / `""` | first run only: both set and no connection ever stored → imported as the Home Assistant connection (setup and sources) |
| `HA_TEMPERATURE_ENTITY` / `HA_HUMIDITY_ENTITY` | unset | `""` / `""` | first run only: become sensor mappings when no sensor is mapped (setup and sources) |
| `OWM_API_KEY` | unset | `""` | first run only: set and no key ever stored → imported as the OpenWeatherMap key; does not select that provider |
| `SENSOR_TOKEN` | unset | `""` | trimmed; empty = the posted-readings route is not mounted; set but < 32 chars → `Config` error, process exits |
| `OPENROUTER_ENABLED` | on | `true` | `true`, `1` or `yes` in any letter case → on; any other value → off |
| `OPENROUTER_API_KEY` | unset | `""` | empty/whitespace = not configured |
| `OPENROUTER_MODEL` | `anthropic/claude-haiku-4.5` | same | not validated |
| `OPENROUTER_BASE_URL` | `https://openrouter.ai/api/v1` | not passed | — |
| `MCP_TOKEN` | unset | `""` | trimmed; empty = `/mcp` not mounted; set but < 32 chars → `Config` error, process exits (`mcp/config.rs`) |
| `LAWN_NAME`, `LAWN_GRASS_TYPE`, `LAWN_USDA_ZONE` | `Main Lawn`, `TallFescue`, `7a` | same | unknown grass → warn, TallFescue |
| `LAWN_SOIL_TYPE`, `LAWN_SIZE_SQFT`, `LAWN_IRRIGATION_TYPE` | none | `""` | unknown / unparseable → warn, none |
| `LAWN_LATITUDE` / `LAWN_LONGITUDE` | none | `""` / `""` | seed the lawn's location (applications); either missing, unparseable or outside the contiguous United States → warn, no location. |
| `RUST_LOG` | `info` | `info` | invalid filter → warn, `info` |

Every fallback caused by an unusable non-empty value logs a warning naming the setting and the default used, so a typo shows in the startup log instead of quietly changing behavior. An empty value is "not set", not invalid, and takes the default without a warning.

`LAWN_*` values only seed the lawn profile on first start (`ensure_default_profile`); once a profile exists they are ignored and the profile is edited in the app. The one exception is the location, which is also seeded into an existing profile that has none.

**Retired settings.** `DATALAKE_ROOT`, `WEATHER_SILVER_PATH`, `WEATHER_GOLD_PATH`, `NOAA_STATION_WBANNO`, `HA_TEMPERATURE_UNIT`, `OWM_ENABLED`, `OWM_LATITUDE` and `OWM_LONGITUDE` are no longer read. When one is set, startup logs one line naming it and saying it has no effect, so an old Compose file explains itself; the forecast's two say it is fetched for the lawn's location.

**Secret redaction.** `HomeAssistantConfig`, `OpenWeatherMapConfig`, `OpenRouterConfig`, `DatabaseConfig` and `McpConfig` implement `Debug` by hand with the credential printed as `[REDACTED]`.

**Local development.** `cargo run --features dev-env` loads a `.env` via `dotenvy`; the default build has no dotenv dependency and reads only the process environment.

## Startup Sequence (`backend/src/main.rs`)

1. (`dev-env` only) load `.env`.
2. Initialize `tracing_subscriber::fmt` with `RUST_LOG` (default `info`), so config warnings are logged.
3. `Config::from_env()`.
4. `create_pool`: connect to Postgres, retrying with increasing backoff for about a minute while it is unreachable (each failed attempt logged at WARN), then run all pending migrations (below).
5. `ensure_default_profile`: insert a profile from `LAWN_*` when the database has none; seed the location into a profile that lacks one. Build the `Clock` and log it.
6. Construct the source clients and the summary service, and turn `HA_TEMPERATURE_ENTITY` / `HA_HUMIDITY_ENTITY` into sensor mappings when none exist. No network I/O happens here.
7. Construct the OpenRouter client whenever a key is configured, turned off or not (a turned-off client answers with the "turned off" 503, so the owner learns which switch to flip); log the model and whether it is enabled at INFO.
8. Build `AppState` (pool, summary service, schedule handle, clock, OpenRouter, `mcp_enabled`).
9. Assemble the router (below), bind `SERVER_HOST:SERVER_PORT`, start the fetch schedule (data sync), and `axum::serve` with graceful shutdown on SIGTERM.

| Startup failure | Behavior |
|---|---|
| `DATABASE_PASSWORD` unset or empty; `MCP_TOKEN` or `SENSOR_TOKEN` too short | `Config` error returned from `main`; exit non-zero |
| Postgres unreachable | retried after 2, 4, 8, 16 and 30 s (about a minute, `db::pool::startup_delays`, each failure logged at WARN), so a database that starts a little later (a host reboot, or a run without Compose's `depends_on`) is picked up; still unreachable → error returned from `main`, exit non-zero. Only reachability failures are retried (I/O, pool timeout, TLS, Postgres `57P03` still starting): a wrong password or database name fails at once |
| Migration fails | error returned from `main`; exit non-zero (no retry: a failing migration fails the same way again) |
| Invalid `CORS_ALLOWED_ORIGIN` | panic in `build_cors_layer` |
| Port already bound | `TcpListener::bind` error; exit non-zero |
| Any weather source, Home Assistant or OpenRouter unreachable | no effect at startup; the schedule retries, and features degrade per source |

**Shutdown.** On SIGTERM (or Ctrl-C) the server stops accepting new connections, lets in-flight requests finish, tells the fetch schedule to stop after the fetch in progress, then exits 0 (`axum::serve(...).with_graceful_shutdown`, `main.rs::shutdown_signal`), so `docker stop` or a Compose restart does not cut a request off midway.

## Router Assembly and Middleware

The REST routes (30 paths, 41 method handlers under `/api/v1`, listed in `main.rs` and owned by each feature LLD) are built first, then wrapped, in order, with:

1. `RequestBodyLimitLayer(1 MiB)`.
2. `api::json_errors::with_json_errors` (`backend/src/api/json_errors.rs`): a catch-all route `/api/v1/{*path}` that answers JSON 404 "There is no such API endpoint.", and a `map_response` middleware that rewrites any 4xx/5xx response whose body is not JSON (axum's rejections and the 413 from the body limit) into the `{"error"}` shape (see HTTP Error Contract).
3. The CORS layer (`build_cors_layer`): unset → allow only `http://localhost:<SERVER_PORT>`; `*` → `CorsLayer::permissive()`; otherwise that one origin. Methods GET/POST/PUT/PATCH/DELETE, any header.

After the layers, two things are added that the layers therefore do **not** cover:

- **`/api/v1/sensor-readings`** (weather sources): mounted only when `SENSOR_TOKEN` is set, guarded by that bearer token. It is called by hubs and scripts, not the SPA.
- **`/mcp`** (MCP server): merged only when `MCP_TOKEN` is set, guarded by its own bearer middleware. It is server-to-server, so it sits outside CORS. Its internals belong to the MCP server LLD.
- **SPA fallback** (Web UI): when `STATIC_DIR/index.html` exists at startup, `ServeDir(STATIC_DIR)` with `index.html` as the not-found service becomes the router fallback, so any unmatched path outside `/api/v1/` returns the SPA shell; an unknown `/api/v1/...` path has already been answered by the JSON 404 route. Without `index.html` the app runs API-only and logs a warning.

## Database Pool and Migrations (`backend/src/db/pool.rs`)

- `PgPoolOptions::max_connections(DB_MAX_CONNECTIONS or 10).connect_with(...)`, built from discrete `DATABASE_*` fields rather than a URL, over rustls TLS-capable sqlx.
- Migrations live in `backend/src/db/migrations/` as timestamped, forward-only `.sql` files (`YYYYMMDDHHMMSS_name.sql`). `sqlx::migrate!` embeds them at compile time, and `create_pool` runs every pending one at each startup before the server binds. A failure surfaces as `TurfOpsError::Config("Migration failed: …")` and stops startup.
- Each feature LLD owns its tables' schema. This component owns the mechanism: the ordering, startup application, and the rule that a migration ships in the same binary as the code that needs it.
- Queries use sqlx's runtime API (no `query!` macros, no `.sqlx` offline data), so the image and CI build without a database.
- Tests that need PostgreSQL sit behind the `db-tests` Cargo feature. Each is a `#[sqlx::test]` that gets a fresh database, with every migration applied, on the server `DATABASE_URL` names. Plain `cargo test` compiles them out and needs no database.

## HTTP Error Contract (`backend/src/error.rs`)

All handler errors are `TurfOpsError` and render as JSON `{"error": "<message>"}`.

| Variant | Status | Message to client | Logged |
|---|---|---|---|
| `NotFound(msg)` | 404 | `msg` | no |
| `InvalidData(msg)` | 400 | `msg` | no |
| `Unavailable { origin, outage }` | 503 | a fixed sentence for the source and the outage (below) | WARN with the detail, for `Down(detail)` |
| `Database` violating `applications_unique_turf` / `applications_unique_plant` | 409 | fixed duplicate-application text | no |
| `Database` violating any other unique index | 409 | generic plain-language conflict text (the record conflicts with one that already exists) | no |
| `Config(msg)`, `Internal(msg)` | 500 | `SERVER_ERROR_MESSAGE`: "Something went wrong on the server. Try again; the details are in the server log." | ERROR |
| `Database` (other), `Http`, `Io`, `Json` | 500 | `SERVER_ERROR_MESSAGE` | ERROR |

`owner_message()` builds the `error` text; `IntoResponse` picks the status and does the logging. `Internal` marks the server's own data being inconsistent (a stored profile row without an id, for example).

**Data-source outages.** `Source` names a source as the owner knows it: `WeatherRecord` (the stored weather as a whole), `SoilStation`, `Grid` (daily air and rain), `HourlyAnalysis`, `OwnerSensors` (Home Assistant or posted readings), `Forecast`, `PlanAssistant` (OpenRouter for plant care plans), `ProductAssistant` (OpenRouter answering for a shelf product being added), `ProfileRefresh` (OpenRouter regenerating a shelf product's profile) and `SoilOutlook` (the 10 cm projection). `Outage` says why: `NotConfigured`, `Disabled`, `Down(detail)`, `NotEnoughData`, `LocationNotSet` or `HistoryLoading { held, wanted }`. Sources build these through `TurfOpsError::unavailable(source, detail)` and its siblings; `for_products()` re-labels a `PlanAssistant` outage as `ProductAssistant` and `for_refresh()` as `ProfileRefresh`. Each source and outage pair has one sentence saying what is unavailable, what it affects and what to do:

| Outage | Sentence |
|---|---|
| `LocationNotSet` | "TurfOps doesn't know where your lawn is yet, so weather, timing and disease information are unavailable. Set the location in Setup." |
| `HistoryLoading` | "Weather history is still loading (*held* of *wanted* years), so typical dates aren't available yet. Current conditions and the forecast already are." |
| `Down` for a weather source | Names the source in the owner's words and what rests on it, for example "The soil station (*name*) can't be reached, so soil readings are coming from the estimate." |

A `Down` detail (an upstream status and body, a raw model output) is logged and never part of the response. A source failing in the background is not a request error at all: it appears as source status (data sync).

The duplicate-application text tells the owner how to fix the entry (edit the existing one, or rename the product), so it is used only for the two indexes it describes; the constraint name decides which message applies, and any other unique index gets the generic text rather than advice that does not fit. Requests that fail before a handler runs get the same shape through `with_json_errors`, one plain sentence per status: 400 (malformed JSON, a non-numeric `{id}`) "The request couldn't be read. Check the form and try again.", 405 "That action isn't supported here.", 413 (a body over 1 MiB) "That request is too large to accept.", 415 "The request must be sent as JSON.", 422 "Some fields are missing or not valid. Check the form and try again.", any 5xx the server-error sentence. A response that is already JSON passes through unchanged.

## Health Endpoint (`backend/src/api/health.rs`)

`GET /api/v1/health` always answers **200** with `{status, version, database, sources, mcp}`. `database` is `SELECT 1` with a 2 s timeout; `status` is `ok` when that succeeds, else `degraded`. `sources` is data sync's source status: one entry per source in use, with backfill progress. `mcp` is `enabled` or `disabled`. `version` is `CARGO_PKG_VERSION`. The image's `HEALTHCHECK` consumes only the 200.

## Logging and Operations

- `tracing` text output to stdout; `RUST_LOG` sets the filter. Startup logs whether a location is set, which integrations are configured, any credential imported from the environment on this start (never its value), the OpenRouter model, whether `/mcp` is mounted, the CORS origin, and the bind address, so a misconfiguration shows on every restart.
- Startup also logs the resolved timezone, offset and local date and time, and warns on every setting that fell back to a default.
- Shipping stdout to a log store is the deployment's concern; the backend assumes nothing about it.
- The container needs outbound HTTPS to the weather hosts and no mounted volume. The fetch schedule logs each failed fetch at WARN with the client's detail and each backfill year at INFO.
- No log line may contain a credential. The OpenWeatherMap key travels in the request URL's query string, so its client formats every reqwest error with `without_url()` (`request_error`, `backend/src/datasources/openweathermap.rs`).

## CI, Release and Supply Chain (`.github/workflows/`, `renovate.json5`, `scripts/github-protection.sh`)

| Workflow | Trigger | Gates / output |
|---|---|---|
| `ci.yml` | push / PR to `main` | Backend: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --features db-tests` against a `postgres:18` service container (trust auth, job-local). Frontend (Node 26): `npm ci`, `npx tsc -b`, `npm run lint`, `npm test` (Vitest). Calls the shared supply-chain scan; on the weekly schedule (Mondays 13:00 UTC) only the scan runs. |
| supply-chain scan | called by `ci.yml` and `docker-publish.yml` | The organization's shared workflow (`chainringlabs/ci`, pinned by commit), given this repository's lockfile folders and Node major: `npm ci --ignore-scripts`; `npm audit signatures`; Socket scan of `./frontend` (skipped when `SOCKET_SECURITY_API_KEY` is absent, as on bot PRs); OSV on both lockfiles. On pull requests OSV compares against the base branch and fails only on what the pull request introduces; on pushes to `main` and the weekly run it fails on any known vulnerability and `cargo-deny`'s advisory check also runs. `cargo-deny` checks licences, bans and sources against `backend/deny.toml` on every event. |
| `docker-publish.yml` | push / PR to `main`, `v*` tags | Runs the supply-chain scan, then calls the organization's shared publish workflow (`chainringlabs/ci`, pinned by commit; check `image / Build`), which builds the image (GHA cache). PRs build without pushing, which is what exercises `npm run build`. Otherwise pushes to GHCR (`latest` on main, semver, `sha`), signs keylessly with cosign, attaches an SPDX SBOM and build-provenance attestations. The signature names the shared workflow as signer and this repository as source. |
| `ghcr-retention.yml` | Mondays 06:00 UTC / manual | Calls the organization's shared retention workflow (`chainringlabs/ci`, pinned by commit). Keeps `latest` plus the 5 newest tagged versions; deletes untagged manifests. |

**Advisory ignores.** `backend/deny.toml` (cargo-deny) and `backend/osv-scanner.toml` (OSV) name an advisory only while its crate is in `Cargo.lock`, each with a written justification; an ignore whose crate has left the lockfile is deleted, so it cannot silently hide a future advisory of the same ID. The OSV list can be the longer one: OSV reads the whole lockfile, while cargo-deny judges only the crates this build compiles, so an advisory against a locked but never-compiled crate needs an OSV entry alone.

**Renovate.** `renovate.json5` extends the shared preset `github>chainringlabs/renovate-config`, which holds the policy for every repo. Renovate runs off-hours on weekdays and anytime on weekends (America/New_York), at most 3 open PRs, creating them immediately and rebasing when behind. Docker images and GitHub Actions are digest-pinned. Minor and patch updates are grouped, wait until the release is 7 days old and auto-merge; digest updates are raised at any time and auto-merge (only Docker Hub reports a release date, so digests from other registries cannot be aged); majors are labeled `major-update` and wait for review. Weekly lock-file maintenance runs Monday before 4 am under the same 7-day age and is merged by the owner. Vulnerability fixes are labeled `security`, open immediately and auto-merge once 7 days old.

**Branch policy.** All changes reach `main` through a PR. The owner merges them, except the non-major dependency updates Renovate auto-merges once their checks pass. The protection on `main` has two layers. The `chainringlabs` organization's ruleset applies to the default branch of every repository in it: a PR is required (no approvals, since the owner is the only reviewer), the GitGuardian check must pass on an up-to-date branch, and force-pushes and deleting the branch are forbidden. The repository's own protection adds what is particular to this project: the Backend, Frontend, Supply-chain scan and image-build jobs are required checks, and the repository deletes each head branch on merge. The repository's settings live in `scripts/github-protection.sh`, which states them once and either checks them against the GitHub API (`check`, the default) or applies them (`apply`); `check` also confirms the organization's rules are in force on `main`, which the script cannot apply. It needs a `gh` login with admin rights on the repository, so it runs by hand rather than in CI. **Renovate's config** is `renovate.json5` (JSON5 allows the comments that carry its `@spec` annotation).

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Branch protection for the owner | Organization admins may bypass the organization's ruleset and the repository's protection does not bind admins (`enforce_admins` off); the Backend, Frontend, Supply-chain scan and image-build jobs are required checks beside the organization's GitGuardian check | Enforce for admins; require GitGuardian only | Renovate auto-merges dependency updates, and with only GitGuardian required a red build or a failed supply-chain scan would not stop one. The owner keeps an emergency path to `main` as an admin and judges a red check case by case. GitGuardian stays required because a leaked secret cannot be undone. |
| Where the GitGuardian check is required | In the organization's ruleset only | Also in the repository's own protection | The rule is the same for every repository, so it is stated once where a new repository gets it without setup; a second copy here could only drift from it. |
| Scan mode on pull requests | OSV comparison: fail only on vulnerabilities the pull request introduces; full scan with `cargo-deny`'s advisory check on `main` and weekly | Full scan on every pull request | A full scan on pull requests blocks unrelated work, including automatic dependency updates, the day an advisory lands against a dependency already on `main`. The weekly run keeps `main`'s own state visible. |
| Rust dependency policy tool | `cargo-deny` (advisories, licences, bans, sources) | `cargo audit` (advisories only) | Both read the RustSec database, so advisory coverage is the same; `cargo-deny` adds a licence allow-list and source restrictions, and it is the one tool every Rust repository in the organization uses. |
| Where the scan logic lives | The organization's shared workflow (`chainringlabs/ci`), pinned by commit | A copy in this repository | One reviewed scan for every repository; a change reaches this one only when its pin is raised. The ignore lists stay here. |
| Runtime base | Chainguard `glibc-dynamic` distroless | Debian slim; Alpine/musl; scratch | No shell or package manager, non-root by default, small CVE surface; glibc avoids musl build friction with the TLS stack. [inferred] |
| What an installation is | The app image and PostgreSQL; weather fetched by the app | A mounted weather lake built by a separate pipeline; a second ingester container | HLD: the product stands alone. A mount or a second service is something a new owner must build before the first page works. |
| Where settings live | Environment for what must exist before start; the app and its database for choices about the lawn | Everything in the environment | HLD decision. A station or a sensor mapping is a choice among things the app has to find and show, and must change without a restart. |
| Retired settings | Ignored, with one startup line naming each that is still set | Refuse to start; ignore silently | An upgrade must not fail on an old Compose file, and the owner should learn the setting no longer does anything. |
| Posted-readings route | Mounted only with `SENSOR_TOKEN`, minimum 32 characters, startup refused when shorter | Always mounted; no minimum | The same rule as `MCP_TOKEN`: a write route is either properly guarded or absent. |
| Timezone | The zone of the lawn's location, held in a replaceable `Clock`; no timezone setting; `/usr/share/zoneinfo` shipped for the host-default case before a location exists | `TZ` as a required setting; `TZ` as an override; a timezone field in the app; compute all dates in UTC; the viewer's browser date | Every agronomic date is a lawn-local calendar day, and one clock gives every component the same day (HLD). The zone follows from the location, so any separate setting can only agree with it or contradict it. |
| Required DB password syntax | `${DB_PASSWORD:?}` | `${DB_PASSWORD:?message}`; a default password | A missing password must stop Compose; the message form trips the ggshield secret scanner. |
| Optional keys | Empty or whitespace = not configured | Treat empty as a value; separate `*_ENABLED` only | Compose passes unset `${KEY:-}` as `""`; treating it as configured would start clients with blank credentials. [inferred] |
| Container health check | The binary probes its own `/api/v1/health` (`--healthcheck`), declared in the image | `curl` in the runtime; a Compose-only check; no check | The distroless runtime has no shell or curl and should stay that way; an image-level check covers every deployment of the image, not just the example Compose stack. |
| What the startup retry waits out | Only reachability failures | Every connect error | A wrong password or database name will not fix itself; waiting a minute on it only delays the error the owner needs to see. |
| Config loading | `config::load::from_vars(lookup)` returns the config and its warnings; `from_env` passes the process environment and logs them | Reading `std::env` directly | Tests can feed any combination of settings without touching the process environment, which parallel tests share. |
| Malformed security config | Refuse to start (`MCP_TOKEN` < 32, bad CORS origin) | Warn and continue | A short token or wrong origin would silently weaken an open endpoint (MCP plan, `docs/plans/ai-assistant.md`). |
| Other malformed values | Warn, naming the setting and the default, then fall back | Refuse to start; fall back silently | Keeps the app up on a typo in a non-security setting, while the warning makes the typo visible on the next restart. |
| Empty `DATABASE_PASSWORD` | Refuse to start | Connect with a blank password | A blank password is never intended; failing at startup with the setting's name beats an authentication error from Postgres, and runs without Compose's `:?` guard get the same protection. |
| `*_ENABLED` flags | `true`, `1`, `yes`, case-insensitive | Exact `true` only | `TRUE` or `1` turning a feature off is a surprising failure; these are the common spellings of "on". |
| Role of `docker-compose.yml` | An example any user can run to try the project | The definition of the owner's deployment | Keeps the repo usable by anyone; an environment-specific deploy belongs with that environment, not in the repo. |
| Example Postgres port | Published on `127.0.0.1:5433` only | All interfaces; not published | Local `cargo run` against the Compose database keeps working without exposing the database to the network. |
| Postgres connection at startup | Retry with backoff for about a minute, then exit | Fail on the first error; retry forever | Rides out a database that starts a little later without relying on a restart policy, while a wrong host or password still surfaces as an exit within a minute. |
| npm lifecycle scripts in the image build | `npm ci --ignore-scripts` | Run install scripts | Matches the supply-chain scan's policy; install scripts are a common malware vector and no dependency needs one. |
| Migrations | Embedded, run at every startup before bind | Separate migration job; manual `sqlx migrate run` | One binary is always in step with its schema; single-instance deploy has no migration race. [inferred] |
| Query style | sqlx runtime queries | `query!` compile-time macros | Builds need no live DB or offline metadata in CI or Docker. [inferred] |
| Error exposure | Every `error` is written for the owner: 4xx carry the handler's message, a 503 a fixed sentence per source and outage, a 500 one generic sentence; upstream and internal details only in logs | Echo all error text; pass the source's own error string through | Hides DB, upstream and internal details from clients, and the owner learns what is unavailable and what to do instead of reading a provider's response body. |
| Unique-violation message | Duplicate-application text only for the two application indexes; generic conflict text otherwise | One message for every unique violation | The duplicate text gives fix-it advice specific to the application log, which would mislead for any other record. |
| Health semantics | Always 200 with `ok` / `degraded` | 503 when the DB is down | Clients get a readable status body; integrations are reported, not fatal. [inferred] |
| Release integrity | cosign keyless signing, SBOM, provenance on every pushed image | Unsigned images | Verifiable supply chain for the deployed image. [inferred] |
| Socket scan invocation | `--org grecolabs`, self-skip without the secret | Interactive org selection; required secret | Without `--org` the CLI prompts and hangs CI; bot PRs have no secrets, so a required secret would fail them outright. |
| Dependency currency | Renovate auto-merges grouped minor/patch after a soak; majors reviewed | Manual updates; auto-merge everything | Stays current without unreviewed breaking changes. [inferred] |

## Open Questions & Future Decisions

### Resolved
1. ✅ The image contains no `.env`: `.dockerignore` excludes `.env` / `.env.*`, and only `backend/src`, the Cargo manifests, `frontend/` and two docs are copied.
2. ✅ `/mcp` is unmounted, not merely unauthenticated, when `MCP_TOKEN` is empty.

### Deferred
1. **No panic hook.** There is none beyond the default. `build_cors_layer` (`main.rs`) and the OWM client builder (`datasources/openweathermap.rs`) use `expect`, so a bad CORS origin ends startup in a panic, not in the `Config` error every other refused setting produces.
2. **Settings outside the loader.** `STATIC_DIR` is read in `main.rs`, not in `config::load`.
3. **`OPENROUTER_MODEL` is not validated.** An id OpenRouter does not know surfaces only when the assistant is first called (the default, `anthropic/claude-haiku-4.5`, is valid).
4. **CI does not build the frontend.** `ci.yml` type-checks, lints and runs the Vitest suite; only the PR image build in `docker-publish.yml` runs `npm run build`.
5. **`.dockerignore` patterns are root-anchored.** `target/`, `src/` and `*.md` match only top-level paths, so `backend/target` (tens of GB locally) is not excluded. BuildKit transfers only paths the Dockerfile copies, which keeps this harmless today. Widening `*.md` to `**/*.md` would break the MCP docs copy.
6. **`main.rs` is 353 lines,** over the 300-line limit; the route table is the natural split (for example `api::router()`).
7. **No start-up smoke test of the built image.** The PR image build proves the image builds, not that the binary loads and starts on the runtime base. A smoke run of the built image would close this.
7. **The example stack's database user is fixed.** The `db` service always creates `turfops`, while `app` takes `${DATABASE_USER:-turfops}`; a `.env` that sets `DATABASE_USER` (Compose reads `.env` automatically) makes the app authenticate as a user the database never created.

## References

- `Dockerfile`, `docker-compose.yml`, `.dockerignore`, `.gitignore`, `renovate.json5`, `scripts/github-protection.sh`
- `backend/src/main.rs`, `backend/src/config.rs`, `backend/src/error.rs`, `backend/src/api/json_errors.rs`, `backend/src/db/pool.rs`, `backend/src/db/migrations/`, `backend/src/api/health.rs`, `backend/src/mcp/config.rs`, `backend/src/mcp/resources.rs`
- `backend/Cargo.toml`, `backend/deny.toml`, `backend/osv-scanner.toml`, `backend/.env.example`
- `.github/workflows/ci.yml`, `docker-publish.yml`, `ghcr-retention.yml`
- `README.md` (Environment Variables, Docker Build, local development)
- `docs/plans/ai-assistant.md` (MCP token and mounting rules)
- Boundaries: Weather data access (source clients, the record's migrations), Data sync (the fetch schedule, source status), Setup and sources (stored settings and credentials), Applications (the lawn profile and its location), MCP server (`/mcp` router and auth), Web UI (SPA served from `STATIC_DIR`)
