# Standalone weather: build status

Where the build of the standalone weather product stands, what is left, and what is waiting
on a decision. The research behind the design is in
[standalone-weather-sources.md](standalone-weather-sources.md); the design and specs are under
`docs/intent/`. This file is the place to start when picking the work up.

Status as of 2026-11-25: slice 7 is complete (#374 to #380); slice 8 is under way: the forecast on the schedule (#381) is merged, and the summary from the record is in review.

## The goal

TurfOps fetches its own weather into PostgreSQL and no longer depends on the Dagster weather
lake. Every input is measured where a source is near enough and estimated where it is not, and
every value says which. No data service needs a key or a payment.

## What is built

Everything below is merged to `main`. None of it changes what a page shows: the pages, rules
and MCP tools still read the lake through the old path until the cutover (slice 8).

| Slice | What | PRs |
|---|---|---|
| 1 | Soil-estimate model and the scoring tool that publishes its accuracy | #340 |
| 2 | The lawn's location and timezone; storage for the weather record | #341, #342 |
| 3 | Source clients: USCRN, SCAN, PRISM and NRCC, Open-Meteo, National Weather Service; the forecast model with slot length and empty quantities | #344, #346, #347 |
| 4 | The weather record: basis plan, resolution, every query contract, what is due, source status | #348 to #352 |
| 5 | The soil estimate wired in: calibration choice and data, fitting, the series, anchoring | #353 to #356 |
| 6 | The fetch schedule, started at boot, with its logging | #357, #358, #360 |
| 7, backend | Setup storage; place search, preview and station list; pins, forecast provider and the one-step apply; the Sources read; Home Assistant connection and entity list; sensor mapping with its check; preview elevation; reading mapped Home Assistant sensors every 5 minutes; posted sensor readings behind `SENSOR_TOKEN`, with unique sensor names | #362, #363, #364, #367, #368, #370, #374, this PR |

Also merged along the way: the notice for late station data (#361) and the `url` security
bump (#345).

## What runs in production today

- **The fetch schedule runs.** An install whose lawn has a location downloads its station,
  the grids, the hourly analysis and its calibration data into PostgreSQL, and keeps them
  current. On each start it logs `Weather schedule running … history_fetches_waiting=N`.
- **Connections and sensors are imported once.** On the first start after #367 and #368,
  `HA_URL` with `HA_TOKEN`, and `OWM_API_KEY`, are copied into the database, and the entities
  in `HA_TEMPERATURE_ENTITY` and `HA_HUMIDITY_ENTITY` are mapped as sensors.
- **Mapped Home Assistant sensors are read every 5 minutes** into the record, each with its own
  status. The first good read after a start logs `Home Assistant sensors read sensors=N stored=N`.
  Sensor readings older than a week are deleted once a day.
- **Posted readings** are taken at `POST /api/v1/sensor-readings` when `SENSOR_TOKEN` is set.
  The deployment on `fridge` does not set it, so the route is not mounted there.
- **The setup and sources endpoints exist** and nothing in the UI calls them.

## What is left

### Slice 7, remaining

1. **The UI**, in four PRs, each usable once merged:
   1. The Sources page at `/sources`, read-only, with the shared basis label (#377).
   2. Setup steps 1 and 2 (location, what we found), opened from the Sources page's Change (#378).
   3. Setup step 3 (your sensors), with map and unmap from the Sources page (#379).
   4. Setup steps 4 and 5 (your lawn, loading history) (this PR).
   The basis label on every other weather page, and the "Tell TurfOps where your lawn is"
   notice (SETUP-FLOW-001), wait for the cutover (slice 8): until those pages read the record
   and the clock follows the location, the notice would hide content that works and its UTC
   sentence would be untrue. Specs: SETUP-FLOW, SETUP-LOC, SETUP-PLAN, SETUP-HA, SETUP-MAP,
   SETUP-PAGE; WX-VIEW-BASIS.

### Slice 8, the cutover

The first slice that changes what the owner sees. Five PRs:

1. **The forecast from the chosen provider**, fetched on the schedule, with its status (#381, merged).
2. **The summary from the record**, with basis labels on the dashboard and the Environmental
   page, refresh through the schedule, and source status in place of the connection flags
   (#382, merged).
3. The daily series, in two parts: (a) `/gdd` and the seasonal plan's 10 cm crossings from
   the record, with the crossing cache tables dropped (#383, merged); (b) the 10 cm outlook
   from the record, with its estimated method (#384, merged). Plant watering reads the
   summary (PR 2).
4. Timing windows, disease risk and `/historical`, in three parts: (a) `/historical` from the
   record, with basis-aware trend charts and the Environmental page's source credits
   (#387); (b) disease risk from the record, approximate tiers marked (in review, stacked
   on 4a); (c) timing windows from the record.
5. The clock follows the location, the no-location notice, MCP results without coordinates,
   MCP tools on the record, and the retired-settings lines.

What each covers:

- **The summary from the record.** Data sync assembles `EnvironmentalSummary` from
  `fetch_summary`, carrying each value's basis, with the age limits and the one-minute memo.
  The record already returns its own `RecordSummary`; the page model has no place for a basis
  yet. Specs: SYNC-MERGE, SYNC-AGE, SYNC-FAIL, SYNC-CONN, SYNC-MEMO.
- **The forecast from the chosen provider**, fetched on the schedule. Specs: WX-SRC-FC-001,
  004; SYNC-FAIL-006.
- **The refresh endpoint** wakes the schedule. Spec: SYNC-SCHED-009.
- **The no-location notice** on every weather page (SETUP-FLOW-001), and the basis label
  there (WX-VIEW-BASIS).
- **Every consumer reads the record:** timing windows, seasonal plan, soil outlook, disease
  risk, the rules, landscape plants, `/historical`, `/gdd`, the MCP tools.
- **The clock follows the location.** `TZ` stops being read. Specs: DEPLOY-TZ.
- **MCP results carry no coordinates.** The `lawn_profile` tool returns them today.
- **Retired settings** log one "no effect" line each.

### Slice 9, removal

DuckDB, the lake reader, the cache tables, the Compose mount, and the documents that describe
them: `CLAUDE.md`, the README, `docs/agronomy-methods.md`, `docs/timing-windows.md`,
`docs/arrows/index.yaml`.

## Waiting on a decision

Nothing.

## Decisions already made

| Decision | Chosen |
|---|---|
| Elevation during a setup preview | Fetched from the hourly analysis at preview time, so the 1,000 ft guard applies to a preview as it will to the saved plan. Decided 2026-11-23. |
| Years of station history | 15 complete years plus the current one, hourly. Reviewed on 2026-11-22 and kept. |
| A forecast day with no rain amount | The irrigation rule keeps its existing specs (probability alone never cancels it; it needs three days that carry an amount). Rain delay, plant watering, the application window and disease leaf wetness follow the table in PR #347. |
| Timezone licence | A scoped cargo-deny exception for `tzf-dist` (ODbL) with attribution. |
| Credentials | The environment seeds the app once; the stored copy is then the one the app manages. Stored unencrypted in the database. |

## Known gaps, not yet addressed

- **A depth with no model blocks the long contracts.** When fewer than three stations qualify
  to calibrate a depth, no estimate is written, the estimate source never registers a span,
  and "history still loading" does not clear for that depth. Unlikely in the contiguous
  United States, where the five nearest are taken at any distance.
- **SCAN's hourly precipitation** has not been checked for which hour it belongs to.
- **Home Assistant's `last_reported`** is taken from memory of its API, not from a live
  instance. The client falls back to `last_updated`.
- **The disease tier thresholds** have not been re-run on hourly-analysis inputs.
- **The anchoring rules** are unscored.

## Operations notes

- **Renovate PR #366 (lock file maintenance) must not be merged before 2026-11-27.** It moves
  `url` back to 2.5.8, which has RUSTSEC-2026-0592, because 2.5.9 is inside the 7-day release
  wait. Renovate rebuilds it once 2.5.9 is old enough.
- **The deployment's `TZ`.** The TurfOps container had none, so the app's clock was on UTC and
  date-gated advice rolled over in the early evening. `TZ: "{{ timezone }}"` was added
  to it in the home-builder repo on 2026-11-23 (PR #413 there); once that is deployed the
  startup log reads `Clock: TZ=America/New_York`. It stays until the cutover makes the clock follow
  the location, after which `TZ` is not read.
- **Confirming the backfill** on a running install: the startup log line, or

  ```sql
  SELECT s.key, f.product, f.held_from, f.held_through, f.last_success, f.last_error
  FROM weather_fetch_state f JOIN weather_source s ON s.id = f.source_id
  ORDER BY s.key;
  ```

## How to check the work

- `cd backend && cargo test`: the suite with no database (1,184 tests).
- `cd backend && DATABASE_URL=postgres://postgres@127.0.0.1:5434/postgres cargo test --features db-tests`:
  everything, against PostgreSQL (1,490 tests). The container is
  `docker run -d --name turfops-testdb -p 127.0.0.1:5434:5432 -e POSTGRES_HOST_AUTH_METHOD=trust postgres:18`.
- `cd backend && cargo test live_ -- --ignored --test-threads=1`: one real request to each
  outside source.
- `cd backend && DATABASE_URL=… cargo test --features db-tests live_schedule -- --ignored`:
  the schedule's first passes for a new lawn against the real services. Run it before any
  change to the plan, the fetcher or the schedule: it caught a fault no stubbed test could.
- `node scripts/coherence-check.mjs`: 1,006 specs implemented, 205 open, 11 deferred.
