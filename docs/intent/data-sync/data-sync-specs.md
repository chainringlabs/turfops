# Data Sync — EARS Specs

Design: [data-sync-design.md](data-sync-design.md). Prefix `SYNC`. Facets: `SCHED` (the fetch schedule), `REFRESH` (startup and the clock), `MERGE` (assembling the summary), `AGE` (how old a value may be and still be current), `FAIL` (failures and degradation), `CONN` (source status and `/health`), `MEMO` (the shared climate-record slot).

"The schedule" is the one background task that calls source clients and writes what they return to the weather record. "A fetch" is one call to one client for one product and span. "The summary" is the `EnvironmentalSummary` data sync returns. "Current data" is every product except backfill: the owner's sensors, the forecast, the stations' and the hourly analysis's newest hours, and the grids' and calibration stations' newest days. The record's contracts are `WX-REC-*`; the clients' are `WX-SRC-*`; the `/environmental` endpoints' contract is WX-VIEW-API-001.

## The fetch schedule

- [x] **SYNC-SCHED-001**: The system shall run one background schedule, started after the database is reachable and migrated, that once a minute, and at once when woken, asks the weather record what is due and runs those fetches.
- [x] **SYNC-SCHED-002**: The schedule shall run fetches one at a time, and shall write each fetch's rows and outcome to the weather record before starting the next.
- [x] **SYNC-SCHED-003**: The schedule shall run due fetches in this order: the owner's sensors, the forecast, current hourly data, current daily data, then backfill.
- [x] **SYNC-SCHED-004**: The schedule shall run backfill newest year first and, within a year, the lawn's own sources before calibration sources, and after each backfill fetch shall run any current data that has become due before the next backfill fetch.
- [x] **SYNC-SCHED-005**: The schedule shall run hourly and daily fetches at a minute of the hour chosen at random when the installation first gets a location and kept thereafter; a source's first fetch, and a retry after a failure, shall not wait for that minute.
- [x] **SYNC-SCHED-006**: If a fetch fails, then the schedule shall log the client's detail, record the failure against that source and product, and try that product again after 5 minutes, then after 15 minutes, then once an hour until a fetch of it succeeds.
- [x] **SYNC-SCHED-007**: A failed fetch shall not delay or cancel the fetches of any other source or product.
- [x] **SYNC-SCHED-008**: When the lawn's location, a pinned station, a sensor mapping, the Home Assistant connection or the forecast provider changes, the system shall wake the schedule.
- [x] **SYNC-SCHED-009**: When `POST /api/v1/environmental/refresh` is called, the system shall ask the schedule for a pass that reads the owner's sensors and fetches the forecast whatever their rhythm and runs every current fetch the weather record reports due without waiting for the installation's minute of the hour, unless a refresh was asked less than a minute ago, shall wait up to 20 seconds for that pass to finish, and shall return a summary assembled after it.
- [x] **SYNC-SCHED-010**: When the process is told to stop, the schedule shall start no further fetch and shall end once the fetch in progress finishes or times out.
- [x] **SYNC-SCHED-011**: While the lawn has no location, the schedule shall make no outside request.
- [x] **SYNC-SCHED-012**: The schedule shall hold no lock that a request for the summary needs while a fetch is in progress.
- [x] **SYNC-SCHED-013**: While the lawn has a location, a Home Assistant connection is stored and at least one Home Assistant sensor is mapped, the schedule shall read those sensors (WX-SRC-HA) at the start of a pass when 5 minutes or more have passed since the last read was attempted, whether it succeeded or failed, without waiting for the installation's minute of the hour, and shall store each reading returned (WX-REC-STORE-006).
- [x] **SYNC-SCHED-014**: While no Home Assistant connection is stored or no Home Assistant sensor is mapped, the schedule shall make no request to Home Assistant.
- [x] **SYNC-SCHED-015**: When the schedule reads the owner's Home Assistant sensors, it shall record against each sensor's source a success when a reading was returned or its reported time was unchanged, and otherwise a failure with a sentence naming the problem: Home Assistant did not answer, refused the token, reports the sensor as unavailable, has no such sensor, or reports it in a unit its input does not accept or at an implausible value.
- [x] **SYNC-SCHED-016**: If Home Assistant does not answer or refuses the token, then the schedule shall try the read again as SYNC-SCHED-006 describes for a failed fetch.
- [x] **SYNC-SCHED-017**: The schedule shall log at INFO the first Home Assistant read that succeeds after a start or after a failed read, with the number of sensors read and readings stored, and every other successful read at DEBUG.
- [x] **SYNC-SCHED-018**: Once a day the schedule shall delete sensor readings older than 7 days (WX-REC-STORE-007).
- [x] **SYNC-SCHED-019**: While the lawn has a location, the schedule shall fetch the forecast from the chosen provider for the lawn's location in the first pass after a start and in the first pass after the chosen provider or OpenWeatherMap's stored key changes, and otherwise hourly at the installation's minute of the hour.
- [x] **SYNC-SCHED-020**: The schedule shall hold the newest forecast it fetched and the forecast status (SYNC-CONN-007) in memory only, shall discard the forecast held when the chosen provider changes, and no page or rule shall fetch a forecast itself.
- [x] **SYNC-SCHED-021**: The schedule shall log at INFO the first forecast fetched after a start, after a failed forecast fetch or after a change of provider, naming the provider and the days it covers, and every other forecast fetched at DEBUG.
- [x] **SYNC-SCHED-022**: If a forecast fetch fails, then the schedule shall log the provider's detail, keep the forecast held, set the forecast status's failure to the owner sentence for the forecast being unavailable, and try the forecast again after 5 minutes, then after 15 minutes, then once an hour until a fetch succeeds.
- [x] **SYNC-SCHED-023**: If the chosen provider needs a key and none is stored, then the schedule shall make no forecast request and shall set the forecast status's failure to a sentence naming the provider and saying it needs its key.

## Startup and the clock

- [x] **SYNC-REFRESH-007**: When the server starts, the system shall build its source clients and begin serving requests without waiting for any outside source, so a source that is down does not delay startup.
- [x] **SYNC-REFRESH-009**: Data sync shall take "now" from the app clock for every judgment it makes (which fetch may be retried, how old a value or a forecast is, where the summary's 7-day window ends), never from the system time.

## Assembling the summary

- [x] **SYNC-MERGE-003**: Data sync shall stamp the summary's `last_updated` with the time the summary was assembled, leaving observation times only on the values they belong to and in `soil_observed_at`.
- [x] **SYNC-MERGE-006**: Data sync shall set the summary's `soil_stale` flag to true when the 10 cm soil temperature is withheld because every source in its plan is past its age limit, and to false otherwise, so every web UI stale-soil banner reads this flag and applies no limit of its own.
- [x] **SYNC-MERGE-007**: Data sync shall assemble the summary from the weather record's `fetch_summary(now)` and `fetch_gdd_ytd(today)` and the newest forecast the schedule fetched, carrying each value's basis through unchanged.
- [x] **SYNC-MERGE-008**: Data sync shall reuse an assembled summary for requests made within one minute of its assembly, and shall discard it when the basis plan changes or a fetch of current data completes.
- [x] **SYNC-MERGE-009**: Data sync shall set `soil_observed_at` to the observation time of the 10 cm soil temperature in use or, while 10 cm soil temperature is withheld for age, of the newest 10 cm value any source in its plan holds, and shall leave it empty when no source holds one.
- [x] **SYNC-MERGE-010**: Data sync shall carry into the summary, beside its numbers, each current value's basis and observation time and each 7-day figure's basis and whether it is mixed (WX-VIEW-API-007), marking an input whose basis plan has no source.
- [x] **SYNC-MERGE-011**: Data sync shall make no outside request: the summary shall read only the weather record and the forecast the schedule holds.

## How old a value may be

- [x] **SYNC-AGE-001**: Data sync shall treat an input's source as current while its newest value is no older than: for the owner's sensor, 1 hour for air temperature and humidity, 6 hours for soil temperature and soil moisture, and without limit for rain; 3 hours for the hourly analysis; 72 hours for a station; and 72 hours after the end of the day it is for, for a daily estimate.
- [x] **SYNC-AGE-002**: While the highest-ranked source for an input is past its age limit, the summary shall take that input from the next source in the plan that is within its own limit.
- [x] **SYNC-AGE-003**: While no source in an input's plan is within its age limit, the summary shall leave that input's current value empty and leave empty the 7-day figure built on it, reporting the soil trend as `Unknown` when the input is 10 cm soil temperature.
- [x] **SYNC-AGE-004**: The summary shall never carry a current value from a source past its age limit.

## Failures and degradation

- [x] **SYNC-FAIL-001**: If any source fails, then data sync shall still return a summary, so no caller (including `GET /api/v1/environmental` and `POST /api/v1/environmental/refresh`) receives a data-source error from it.
- [x] **SYNC-FAIL-006**: If a forecast fetch fails or the chosen provider cannot be used, then data sync shall serve the last forecast fetched in the summary only while it was fetched less than 6 hours ago, and no forecast otherwise.
- [x] **SYNC-FAIL-007**: The summary data sync returns shall never carry a forecast fetched 6 or more hours ago.
- [x] **SYNC-FAIL-010**: If the weather record cannot be read while assembling the summary, then data sync shall return the previous summary while it was assembled less than 5 minutes ago, and otherwise a summary whose weather fields are empty.
- [x] **SYNC-FAIL-011**: While the lawn has no location, the summary's weather fields shall be empty.

## Source status and health

- [x] **SYNC-CONN-001**: When data sync reports source status, it shall report one entry per source in use in the shape of WX-VIEW-STATUS-001, taking whether a source is up from the outcome of its most recent scheduled fetch, and reporting a source never yet fetched as pending.
- [x] **SYNC-CONN-002**: Reporting source status shall make no outside request and shall leave the summary unchanged.
- [x] **SYNC-CONN-003**: When `GET /api/v1/health` is called, the system shall respond with `status`, `version`, `database`, the source status under `sources` with each source's history held against wanted, the forecast status under `forecast`, and `mcp` (`enabled` when `/mcp` is mounted, else `disabled`).
- [x] **SYNC-CONN-004**: `GET /api/v1/health` shall report `status` `ok` and `database` true when the database answers `SELECT 1` within 2 seconds, and `degraded` and false otherwise, and shall not let an external source's outage change `status`.
- [x] **SYNC-CONN-005**: `GET /api/v1/dashboard` (as `sources`, with the forecast status as `forecast_status`) and the MCP `lawn_snapshot` tool (as `data_sources`, an object holding `sources` and `forecast`) shall include the source status.
- [x] **SYNC-CONN-007**: The forecast status shall give the chosen provider, when the forecast held was fetched (empty when none is held), and, while the latest forecast fetch failed or the chosen provider cannot be used, the owner sentence saying why (empty otherwise), and reporting it shall make no outside request.
- [x] **SYNC-CONN-008**: `GET /api/v1/health`, `GET /api/v1/dashboard` and the MCP `lawn_snapshot` tool shall report the forecast status beside the source status (SYNC-CONN-003, SYNC-CONN-005), and none of them shall carry an OpenWeatherMap or Home Assistant flag or probe a source; while the database does not answer, `GET /api/v1/health` shall report `forecast` as empty.

## Climate-record slot

- [x] **SYNC-MEMO-001**: The system shall keep one in-process climate-record slot, empty at startup and emptied when the basis plan changes, shared by every caller of the climate record (timing windows, the seasonal plan's timing activities, the MCP `weather_history` tool), measuring a record's age from when its read completed (the one-hour memo itself is TIME-ENG-038).
- [x] **SYNC-MEMO-002**: If re-reading the climate record fails, then the system shall leave the slot unchanged and serve the last memoized record while its read completed less than 24 hours ago, marked as from the last good read so the Timing page shows a data note saying so, and return the error to the caller only when no such record exists.
