# Deployment, Configuration & Operations — EARS Specs

Design: [deployment-design.md](deployment-design.md). Prefix `DEPLOY`. Facets: `IMAGE` (the Docker image), `COMPOSE` (the example Compose stack), `CONFIG` (environment configuration surface), `TZ` (the one "today" clock), `START` (startup sequence and migrations), `HTTP` (router assembly, middleware and the HTTP error contract), `HEALTH` (the health endpoint), `LOG` (logging and secret hygiene), `CI` (CI, release and supply chain).

"Credential" below means any of `DATABASE_PASSWORD` / `DB_PASSWORD`, `HA_TOKEN`, `OWM_API_KEY`, `OPENROUTER_API_KEY`, `MCP_TOKEN` and `SENSOR_TOKEN`, and any credential stored in the app. Which Home Assistant connection and forecast key the clients use is specified by Weather sources (`WX-SRC-CFG-002`, `WX-SRC-CFG-003`); how they are stored and imported, by Setup and sources (`SETUP-CRED-*`).

## Container image

- [x] **DEPLOY-IMAGE-001**: The Docker image shall be built in three stages (frontend build on Node, backend build on Rust, distroless runtime), with every base image pinned by digest.
- [x] **DEPLOY-IMAGE-002**: The runtime image shall add to its base only the release backend binary, the built SPA at `/app/static` and the timezone database at `/usr/share/zoneinfo`, with no shell or package manager.
- [x] **DEPLOY-IMAGE-003**: The runtime stage shall use the Chainguard `glibc-dynamic` base, which supplies the shared libraries the backend binary loads, and shall copy no shared library of its own.
- [x] **DEPLOY-IMAGE-004**: The image's backend build stage shall copy `docs/timing-windows.md` and `docs/agronomy-methods.md` to `/docs/` before compiling, so the MCP server's embedded methodology resources resolve, and `.dockerignore` shall leave `docs/*.md` in the build context.
- [x] **DEPLOY-IMAGE-005**: The runtime image shall run the backend as the non-root uid 65532, with the binary and static files owned by 65532:65532.
- [x] **DEPLOY-IMAGE-006**: The runtime image shall set `STATIC_DIR=/app/static`, expose port 3000 and use the backend binary as its entrypoint.
- [x] **DEPLOY-IMAGE-007**: The Docker build context shall exclude `.env` and every `.env.*` except `.env.example`, so no image contains an environment file.
- [x] **DEPLOY-IMAGE-008**: The app container (image or Compose `app` service) shall declare a health check that probes `GET /api/v1/health` and marks the container unhealthy when it does not answer, using a probe that works without a shell or curl in the distroless runtime.
- [x] **DEPLOY-IMAGE-009**: The runtime image shall declare `USER 65532:65532` explicitly, so the backend runs as that non-root uid even if the base image's default user changes.
- [x] **DEPLOY-IMAGE-010**: The image's frontend build stage shall install dependencies with `npm ci --ignore-scripts` (no package lifecycle scripts, matching the supply-chain scan), unless a dependency genuinely needs an install script, in which case the Dockerfile shall say which one and why.

## Compose stack

- [x] **DEPLOY-COMPOSE-001**: The example Compose stack (`docker-compose.yml`) shall run two services, `db` (`postgres:18-alpine` pinned by digest, database and user `turfops`, data on the named volume `turfops_data`) and `app` (built from the Dockerfile), both with `restart: unless-stopped`.
- [x] **DEPLOY-COMPOSE-002**: If `DB_PASSWORD` is unset or empty, then Compose shall refuse to start the stack; the one value shall feed both the `db` service's `POSTGRES_PASSWORD` and the `app` service's `DATABASE_PASSWORD`, written in the bare `${DB_PASSWORD:?}` form with no message (the message form trips the secret scanner).
- [x] **DEPLOY-COMPOSE-003**: The Compose `app` service shall start only after the `db` service's health check (`pg_isready -U turfops`, every 5 s, 5 retries) passes.
- [x] **DEPLOY-COMPOSE-005**: The Compose `app` service shall pass no `TZ` to the backend.
- [x] **DEPLOY-COMPOSE-006**: The Compose `app` service shall pass every optional setting as `${KEY:-}` or `${KEY:-default}`, so a variable the host leaves unset reaches the backend as an empty string or the stated default; it shall fix `SERVER_HOST=0.0.0.0`, `SERVER_PORT=3000` and `STATIC_DIR=/app/static`, and default `RUST_LOG` to `info`.
- [x] **DEPLOY-COMPOSE-007**: The Compose stack shall publish the app on host port 3000, and PostgreSQL on host port 5433 bound to the host's loopback interface only (`127.0.0.1:5433:5432`), so a local `cargo run` can use the Compose database but other machines on the network cannot reach it.
- [x] **DEPLOY-COMPOSE-009**: The Compose `db` service shall mount its named volume `turfops_data` where the PostgreSQL 18 image keeps its data (the volume root `/var/lib/postgresql`, under which the image stores the cluster in `18/docker`), so a fresh `docker compose up` initializes the database and keeps it across container recreation.
- [x] **DEPLOY-COMPOSE-010**: The Compose `app` service shall pass `LAWN_LATITUDE` and `LAWN_LONGITUDE` as `${KEY:-}` with no default value, so a host that sets neither leaves the lawn without a location rather than at 0,0.
- [x] **DEPLOY-COMPOSE-011**: The Compose `app` service shall mount no volume, and the stack of `db` and `app` shall be a complete installation that needs no other service and no file from the host.

## Configuration

- [x] **DEPLOY-CONFIG-001**: The backend shall read its configuration from environment variables once at startup, giving every setting a default except `DATABASE_PASSWORD`.
- [x] **DEPLOY-CONFIG-002**: If `DATABASE_PASSWORD` is unset or empty at startup, then the backend shall stop with a configuration error naming `DATABASE_PASSWORD` and exit non-zero.
- [x] **DEPLOY-CONFIG-003**: Where `OPENROUTER_API_KEY`, `MCP_TOKEN`, `SENSOR_TOKEN`, `HA_URL`, `HA_TOKEN`, `OWM_API_KEY` or `CORS_ALLOWED_ORIGIN` is empty or whitespace-only, the backend shall treat that setting as not set.
- [x] **DEPLOY-CONFIG-004**: The backend shall construct the OpenRouter client whenever `OPENROUTER_API_KEY` is non-blank, with its enabled flag from `OPENROUTER_ENABLED` (DEPLOY-CONFIG-013; on by default), the model from `OPENROUTER_MODEL` and the base URL from `OPENROUTER_BASE_URL` (default `https://openrouter.ai/api/v1`); a turned-off client answers with the assistant's "turned off" 503 instead of calling out.
- [x] **DEPLOY-CONFIG-007**: If `DATABASE_PORT`, `SERVER_PORT` or `LAWN_GRASS_TYPE` cannot be parsed, then the backend shall log a warning naming the value and use its default (5432, 3000, TallFescue).
- [x] **DEPLOY-CONFIG-008**: The backend shall size its PostgreSQL pool from `DB_MAX_CONNECTIONS`, defaulting to 10.
- [x] **DEPLOY-CONFIG-009**: When the backend starts with no lawn profile in the database, it shall create one from `LAWN_NAME`, `LAWN_GRASS_TYPE`, `LAWN_USDA_ZONE`, `LAWN_SOIL_TYPE`, `LAWN_SIZE_SQFT`, `LAWN_IRRIGATION_TYPE`, `LAWN_LATITUDE` and `LAWN_LONGITUDE`; once a profile exists, the backend shall ignore the `LAWN_*` settings except to give a location to a profile that has none (APP-PROF-015).
- [x] **DEPLOY-CONFIG-010**: Every configuration struct that holds a credential shall print it as `[REDACTED]` in its `Debug` output.
- [x] **DEPLOY-CONFIG-011**: Where the backend is built with the `dev-env` feature, it shall load a `.env` file before reading configuration; the default build shall read only the process environment.
- [x] **DEPLOY-CONFIG-012**: If any configuration setting is set to a non-empty value the backend cannot use (for example an unparseable `SERVER_HOST`, `DB_MAX_CONNECTIONS` or `LAWN_SIZE_SQFT`, an unknown `LAWN_SOIL_TYPE` or `LAWN_IRRIGATION_TYPE`, or an invalid `RUST_LOG` filter), then the backend shall log a warning naming the setting and the default it used, and shall still start.
- [x] **DEPLOY-CONFIG-013**: When `OPENROUTER_ENABLED` is set, the backend shall treat `true`, `1` or `yes`, in any letter case, as on and any other value as off; when unset, the flag shall be on.
- [x] **DEPLOY-CONFIG-014**: If `SENSOR_TOKEN` is set to fewer than 32 characters, then the backend shall stop with a configuration error naming `SENSOR_TOKEN` and exit non-zero.
- [x] **DEPLOY-CONFIG-015**: When the backend starts with `DATALAKE_ROOT`, `WEATHER_SILVER_PATH`, `WEATHER_GOLD_PATH`, `NOAA_STATION_WBANNO` or `HA_TEMPERATURE_UNIT` set, it shall log one line for each naming it and saying it no longer has any effect, and shall start.
- [x] **DEPLOY-CONFIG-016**: The backend shall read `HA_URL`, `HA_TOKEN` and `OWM_API_KEY` only to seed the app's stored settings on a first run (SETUP-CRED-002), and no environment variable shall override a location, source, sensor, provider or credential stored in the app.
- [x] **DEPLOY-CONFIG-017**: When the backend starts with `OWM_ENABLED`, `OWM_LATITUDE` or `OWM_LONGITUDE` set, it shall log one line for each naming it and saying it no longer has any effect because the forecast is fetched from the provider chosen in the app for the lawn's location, shall take nothing from it, and shall start.

## Timezone ("today")

- [x] **DEPLOY-TZ-001**: The runtime image shall ship the timezone database at `/usr/share/zoneinfo`, so the host-default clock (`Clock::System`, which reads `chrono::Local`) has zone data.
- [x] **DEPLOY-TZ-002**: The runtime image shall set no `TZ` of its own; the app clock's zone shall be the lawn profile's timezone, or the host's default zone while the profile has no location, and the backend shall read no timezone setting of its own.
- [x] **DEPLOY-TZ-003**: When the backend starts, and whenever the app clock is replaced, it shall log at INFO the zone in force, its UTC offset, the resulting local date and time, and whether the zone came from the lawn's location or the host default.
- [x] **DEPLOY-TZ-006**: When `GET /api/v1/today` is called, the system shall return `today`, the lawn's local date from the app clock as YYYY-MM-DD, and `timezone`, the IANA zone name (null when the host's default zone applies), without touching the database or any data source.
- [x] **DEPLOY-TZ-007**: The app clock shall supply "now" (an instant) and "today" (that instant's local date in the clock's zone), together as one reading when a computation needs both, and shall offer a frozen variant stopped at a given instant and zone that configuration never produces, so every date-, age- and expiry-dependent judgment can run at a chosen moment.
- [x] **DEPLOY-TZ-008**: When the lawn's location changes, the system shall replace the app clock with one in the new location's timezone, so every request that starts afterwards uses it.

## Startup and migrations

- [x] **DEPLOY-START-001**: When the backend starts, it shall initialize logging, then read configuration, then connect to PostgreSQL and run migrations, then ensure a lawn profile exists and build the app clock, then construct the source and OpenRouter clients, then assemble the router and bind `SERVER_HOST:SERVER_PORT`, then start the fetch schedule.
- [x] **DEPLOY-START-002**: When the backend starts, it shall apply every pending migration embedded in the binary (timestamped, forward-only `.sql` files from `backend/src/db/migrations/`) before binding the listener, so a binary always runs against the schema it ships with.
- [x] **DEPLOY-START-003**: If PostgreSQL is still unreachable when the startup connection retries (DEPLOY-START-007) run out, or a migration fails at startup, then the backend shall exit non-zero without binding the listener.
- [x] **DEPLOY-START-004**: The backend shall bind its listener without waiting on any weather source, Home Assistant or OpenRouter, so an unreachable integration never prevents startup.
- [x] **DEPLOY-START-005**: The backend shall issue database queries through sqlx's runtime API (no compile-time-checked query macros or offline query metadata), so the image and CI build without a database.
- [x] **DEPLOY-START-006**: When the running backend receives SIGTERM, it shall stop accepting new connections, let in-flight requests finish, tell the fetch schedule to stop (SYNC-SCHED-010), and then exit.
- [x] **DEPLOY-START-007**: If PostgreSQL is unreachable at startup, then the backend shall retry the connection with increasing backoff for about one minute, logging each failed attempt at WARN, before giving up.

## Router, middleware and error contract

- [x] **DEPLOY-HTTP-001**: The backend shall apply a 1 MiB request-body limit to every `/api/v1` route (the CORS policy is UI-SRV-004 to 006; `/mcp` mounting and its token rules are MCP-CFG-001 to 005; `/api/v1/sensor-readings` mounting and its token rule are WX-SRC-POST-001 to 003).
- [x] **DEPLOY-HTTP-004**: Where `STATIC_DIR/index.html` exists at startup, the backend shall serve `STATIC_DIR` as the router fallback, answering any unmatched path with `index.html` (client-side routing); otherwise it shall run API-only and log a warning.
- [x] **DEPLOY-HTTP-005**: The backend shall render every API handler error as JSON `{"error": "<message>"}` with status 404 for not-found, 400 for invalid data, 503 for an unavailable data source, 409 for a conflict with an existing record, and 500 for every other error.
- [x] **DEPLOY-HTTP-006**: When an API write violates an application-log duplicate guard (`applications_unique_turf` / `applications_unique_plant`), the backend shall answer 409 with "An identical entry (same type, product and date) is already logged. Edit the existing entry, or change the product name if this was a different product."
- [x] **DEPLOY-HTTP-007**: If an API handler fails with a configuration, database, HTTP-client, I/O or JSON error, then the backend shall log the error's details at ERROR and keep them out of the response body.
- [x] **DEPLOY-HTTP-008**: When the backend answers 500, the `error` message shall be plain language that says something went wrong on the server and what to do (try again; the details are in the server log).
- [x] **DEPLOY-HTTP-009**: When the backend answers 503 for an unavailable data source, the `error` message shall be written for the owner, naming the source that is unavailable, and shall carry no upstream response body, request URL, credential or internal error text; those details shall go to the log.
- [x] **DEPLOY-HTTP-010**: If an `/api/v1` request cannot be extracted (malformed JSON body, a path id that is not a number, a body over 1 MiB), then the backend shall answer with the JSON `{"error"}` shape and a plain-language message saying what was wrong with the request, not the framework's plain-text rejection.
- [x] **DEPLOY-HTTP-011**: If an API write violates a unique index other than the application-log duplicate guards (`applications_unique_turf` / `applications_unique_plant`), then the backend shall answer 409 with a generic plain-language message that the record conflicts with one that already exists, never the duplicate-application text.
- [x] **DEPLOY-HTTP-012**: When a weather contract cannot answer because the lawn has no location, the backend shall answer 503 with "TurfOps doesn't know where your lawn is yet, so weather, timing and disease information are unavailable. Set the location in Setup." and `reason` `location_not_set`
- [x] **DEPLOY-HTTP-013**: When a weather contract cannot answer because history is still loading, the backend shall answer 503 with a sentence giving the years held and wanted and saying that current conditions and the forecast are already available.

## Health endpoint

- [x] **DEPLOY-HEALTH-001**: `GET /api/v1/health` shall always answer 200 with `{status, version, database, sources, forecast, mcp}`.
- [x] **DEPLOY-HEALTH-002**: When `GET /api/v1/health` is called, the backend shall report `database` true when `SELECT 1` succeeds within 2 s, and `status` `ok` when it does, otherwise `degraded`.
- [x] **DEPLOY-HEALTH-003**: `GET /api/v1/health` shall report `sources` as data sync's source status and `forecast` as its forecast status (SYNC-CONN-003), `mcp` as `enabled` or `disabled` by whether `/mcp` is mounted, and `version` as the backend crate version.

## Logging

- [x] **DEPLOY-LOG-001**: The backend shall write text logs to stdout, filtered by `RUST_LOG` (default `info`; an invalid filter falls back to `info`), with logging initialized before configuration is read so configuration warnings are logged.
- [ ] **DEPLOY-LOG-002**: When the backend starts, it shall log at INFO whether the lawn has a location, which integrations are configured, any credential it imported from the environment on this start (never its value), the OpenRouter model, whether `/mcp` and `/api/v1/sensor-readings` are mounted, the CORS origin and the bind address.
- [x] **DEPLOY-LOG-003**: No log line and no HTTP response body produced by the backend shall contain a credential.
- [x] **DEPLOY-LOG-004**: The fetch schedule shall log each failed fetch at WARN with the client's detail and each completed backfill year at INFO.

## CI, release and supply chain

- [x] **DEPLOY-CI-001**: When code is pushed to `main` or a pull request targets `main`, CI shall run `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test --features db-tests` (against a PostgreSQL 18 service) on the backend, and `npm ci` and `npm run lint` on the frontend (Node 26).
- [x] **DEPLOY-CI-002**: When code is pushed to `main` or a pull request targets `main`, CI shall run the organization's shared supply-chain scan against `frontend/package-lock.json` and `backend/Cargo.lock`: frontend install with `--ignore-scripts`, `npm audit signatures`, a Socket scan of `./frontend`, and an OSV scan of both lockfiles.
- [x] **DEPLOY-CI-003**: Where the `SOCKET_SECURITY_API_KEY` secret is absent (bot-authored pull requests), the supply-chain scan shall skip the Socket step and still run its other checks.
- [x] **DEPLOY-CI-004**: When a pull request targets `main`, the image workflow shall build the Docker image (including the frontend production build) after the supply-chain scan passes, without pushing it.
- [x] **DEPLOY-CI-005**: When code is pushed to `main` or a `v*` tag is pushed, the image workflow shall push the image to `ghcr.io/chainringlabs/turfops` tagged by commit sha, by semver for tags and `latest` for `main`, sign it keylessly with cosign, and attach an SPDX SBOM attestation and a build-provenance attestation.
- [x] **DEPLOY-CI-006**: Every Monday at 06:00 UTC, and on manual dispatch, the retention workflow shall keep `latest` and the 5 newest tagged image versions in GHCR and delete untagged manifests.
- [x] **DEPLOY-CI-007**: Renovate, configured in `renovate.json5`, shall extend the shared preset `github>chainringlabs/renovate-config`, under which it shall pin Docker images and GitHub Actions by digest; group minor and patch updates and auto-merge them once the release is 7 days old; raise digest updates at any time; label major updates `major-update` and leave them for review; keep at most 3 open PRs; run lock-file maintenance Mondays before 4 am under the same 7-day age without auto-merging it; and label vulnerability fixes `security`, opening them immediately and auto-merging them once 7 days old.
- [x] **DEPLOY-CI-008**: Every change shall reach `main` through a pull request: the organization's ruleset shall require one (no approvals), require the GitGuardian check on an up-to-date branch, and forbid force-pushes and deletion; the repository's own `main` protection shall require the `Backend (Rust)`, `Frontend (React/TypeScript)`, `Supply chain / Scan` and `image / Build` jobs and shall not repeat the GitGuardian check, and the repository shall delete each head branch once merged; the owner merges every pull request except the non-major dependency updates Renovate auto-merges under DEPLOY-CI-007 once their checks pass. `scripts/github-protection.sh` shall state the repository's settings and check them against GitHub (`check`) or apply them (`apply`), and `check` shall also fail when the organization's rules are not in force on `main`.
- [x] **DEPLOY-CI-009**: The supply-chain scan's advisory ignore lists (`backend/deny.toml` for `cargo-deny`, `backend/osv-scanner.toml` for OSV) shall name an advisory only while its affected crate is in `backend/Cargo.lock`, each with a written justification.
- [x] **DEPLOY-CI-010**: The backend's PostgreSQL-backed tests shall be compiled only with the `db-tests` Cargo feature, so `cargo test` without it needs no database; with it, each such test shall run against a fresh database with every migration applied, on the server `DATABASE_URL` names.
- [x] **DEPLOY-CI-011**: When a pull request targets `main`, the supply-chain scan shall fail only on vulnerabilities the pull request introduces, by comparing OSV scans of its lockfiles with the base branch's; when code is pushed to `main`, and every Monday, the scan shall fail on any known vulnerability in either lockfile and shall also run `cargo-deny`'s advisory check, which additionally fails on unmaintained crates and yanked releases.
- [x] **DEPLOY-CI-012**: On every run of the supply-chain scan, `cargo-deny` shall check the backend's crates against `backend/deny.toml`: licences restricted to an explicit allow-list, no wildcard version requirements, and no crate from a registry or git source other than crates.io.
