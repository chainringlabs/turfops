# Data Sync — EARS Specs

Design: [data-sync-design.md](data-sync-design.md). Prefix `SYNC`. Facets: `SCHED` (the fetch schedule), `REFRESH` (startup and the clock), `MERGE` (assembling the summary), `AGE` (how old a value may be and still be current), `FAIL` (failures and degradation), `CONN` (source status and `/health`), `MEMO` (the shared climate-record slot).

"The schedule" is the one background task that calls source clients and writes what they return to the weather record. "A fetch" is one call to one client for one product and span. "The summary" is the `EnvironmentalSummary` data sync returns. "Current data" is every product except backfill: the owner's sensors, the forecast, the stations' and the hourly analysis's newest hours, and the grids' and calibration stations' newest days. The record's contracts are `WX-REC-*`; the clients' are `WX-SRC-*`; the `/environmental` endpoints' contract is WX-VIEW-API-001.

## The fetch schedule

- [ ] **SYNC-SCHED-001**: The system shall run one background schedule, started after the database is reachable and migrated, that once a minute, and at once when woken, asks the weather record what is due and runs those fetches.
- [ ] **SYNC-SCHED-002**: The schedule shall run fetches one at a time, and shall write each fetch's rows and outcome to the weather record before starting the next.
- [ ] **SYNC-SCHED-003**: The schedule shall run due fetches in this order: the owner's sensors, the forecast, current hourly data, current daily data, then backfill.
- [ ] **SYNC-SCHED-004**: The schedule shall run backfill newest year first and, within a year, the lawn's own sources before calibration sources, and after each backfill fetch shall run any current data that has become due before the next backfill fetch.
- [ ] **SYNC-SCHED-005**: The schedule shall run hourly and daily fetches at a minute of the hour chosen at random when the installation first gets a location and kept thereafter.
- [ ] **SYNC-SCHED-006**: If a fetch fails, then the schedule shall log the client's detail, record the failure against that source and product, and try that product again after 5 minutes, then after 15 minutes, then once an hour until a fetch of it succeeds.
- [ ] **SYNC-SCHED-007**: A failed fetch shall not delay or cancel the fetches of any other source or product.
- [ ] **SYNC-SCHED-008**: When the lawn's location, a pinned station, a sensor mapping, the Home Assistant connection or the forecast provider changes, the system shall wake the schedule.
- [ ] **SYNC-SCHED-009**: When `POST /api/v1/environmental/refresh` is called, the system shall wake the schedule to fetch all current data regardless of cadence, unless it did so less than a minute ago, wait up to 20 seconds for those fetches, and return the summary.
- [ ] **SYNC-SCHED-010**: When the process is told to stop, the schedule shall start no further fetch and shall end once the fetch in progress finishes or times out.
- [ ] **SYNC-SCHED-011**: While the lawn has no location, the schedule shall make no outside request.
- [ ] **SYNC-SCHED-012**: The schedule shall hold no lock that a request for the summary needs while a fetch is in progress.

## Startup and the clock

- [ ] **SYNC-REFRESH-007**: When the server starts, the system shall build its source clients and begin serving requests without waiting for any outside source, so a source that is down does not delay startup.
- [x] **SYNC-REFRESH-009**: Data sync shall take "now" from the app clock for every judgment it makes (which fetch may be retried, how old a value or a forecast is, where the summary's 7-day window ends), never from the system time.

## Assembling the summary

- [ ] **SYNC-MERGE-003**: Data sync shall stamp the summary's `last_updated` with the time the summary was assembled, leaving observation times only on the values they belong to and in `soil_observed_at`.
- [ ] **SYNC-MERGE-006**: Data sync shall set the summary's `soil_stale` flag to true when the 10 cm soil temperature is withheld because every source in its plan is past its age limit, and to false otherwise, so every web UI stale-soil banner reads this flag and applies no limit of its own.
- [ ] **SYNC-MERGE-007**: Data sync shall assemble the summary from the weather record's `fetch_summary(now)` and `fetch_gdd_ytd(today)` and the newest forecast the schedule fetched, carrying each value's basis through unchanged.
- [ ] **SYNC-MERGE-008**: Data sync shall reuse an assembled summary for requests made within one minute of its assembly, and shall discard it when the basis plan changes or a fetch of current data completes.
- [ ] **SYNC-MERGE-009**: Data sync shall set `soil_observed_at` to the observation time of the 10 cm soil temperature in use, and leave it empty when there is none.

## How old a value may be

- [ ] **SYNC-AGE-001**: Data sync shall treat an input's source as current while its newest value is no older than: for the owner's sensor, 1 hour for air temperature and humidity, 6 hours for soil temperature and soil moisture, and without limit for rain; 3 hours for the hourly analysis; 72 hours for a station; and 72 hours after the end of the day it is for, for a daily estimate.
- [ ] **SYNC-AGE-002**: While the highest-ranked source for an input is past its age limit, the summary shall take that input from the next source in the plan that is within its own limit.
- [ ] **SYNC-AGE-003**: While no source in an input's plan is within its age limit, the summary shall leave that input's current value empty and leave empty the 7-day figure built on it, reporting the soil trend as `Unknown` when the input is 10 cm soil temperature.
- [ ] **SYNC-AGE-004**: The summary shall never carry a current value from a source past its age limit.

## Failures and degradation

- [ ] **SYNC-FAIL-001**: If any source fails, then data sync shall still return a summary, so no caller (including `GET /api/v1/environmental` and `POST /api/v1/environmental/refresh`) receives a data-source error from it.
- [ ] **SYNC-FAIL-006**: If a forecast fetch fails or the chosen provider cannot be used, then data sync shall serve the last forecast fetched in the summary only while it was fetched less than 6 hours ago, and no forecast otherwise.
- [x] **SYNC-FAIL-007**: The summary data sync returns shall never carry a forecast fetched 6 or more hours ago.
- [ ] **SYNC-FAIL-010**: If the weather record cannot be read while assembling the summary, then data sync shall return the previous summary while it was assembled less than 5 minutes ago, and otherwise a summary whose weather fields are empty.
- [ ] **SYNC-FAIL-011**: While the lawn has no location, the summary's weather fields shall be empty.

## Source status and health

- [ ] **SYNC-CONN-001**: When data sync reports source status, it shall report one entry per source in use in the shape of WX-VIEW-STATUS-001, taking whether a source is up from the outcome of its most recent scheduled fetch, and reporting a source never yet fetched as pending.
- [ ] **SYNC-CONN-002**: Reporting source status shall make no outside request and shall leave the summary unchanged.
- [ ] **SYNC-CONN-003**: When `GET /api/v1/health` is called, the system shall respond with `status`, `version`, `database`, the source status under `sources` with each source's history held against wanted, and `mcp` (`enabled` when `/mcp` is mounted, else `disabled`).
- [x] **SYNC-CONN-004**: `GET /api/v1/health` shall report `status` `ok` and `database` true when the database answers `SELECT 1` within 2 seconds, and `degraded` and false otherwise, and shall not let an external source's outage change `status`.
- [ ] **SYNC-CONN-005**: `GET /api/v1/dashboard` (as `sources`) and the MCP `lawn_snapshot` tool (as `data_sources`) shall include the source status.

## Climate-record slot

- [ ] **SYNC-MEMO-001**: The system shall keep one in-process climate-record slot, empty at startup and emptied when the basis plan changes, shared by every caller of the climate record (timing windows, the seasonal plan's timing activities, the MCP `weather_history` tool), measuring a record's age from when its read completed (the one-hour memo itself is TIME-ENG-038).
- [ ] **SYNC-MEMO-002**: If re-reading the climate record fails, then the system shall leave the slot unchanged and serve the last memoized record while its read completed less than 24 hours ago, marked as from the last good read so the Timing page shows a data note saying so, and return the error to the caller only when no such record exists.
