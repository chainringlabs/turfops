---
parent: high-level-design
prefix: SYNC
---

# Data Sync

## Context and Design Philosophy

Data Sync keeps the weather record current and turns it into the one picture of present conditions that everything else reasons from. It owns three things: the **fetch schedule** (the background task that calls each source client when its data is due and writes the result to the weather record), the **summary** (`EnvironmentalSummary`: current reading, 7-day aggregates, trend, GDD year-to-date, forecast, each with its basis), and the application's shared runtime state (`AppState`), including the memo of the 15-year climate record and the source status shown on the dashboard and the health endpoint.

Four HLD commitments shape it:

- **The record keeps itself current.** Sources are fetched on their own publishing rhythm whether or not anyone is looking, so a page never waits on a download and the owner's sensors build a history between visits.
- **Polite fetching.** Nothing is requested more often than its source publishes, a failing source is retried on a lengthening interval, and one installation's whole traffic to outside hosts is on the order of a hundred requests a day.
- **Graceful degradation.** Losing one source removes only what depends on it. A failed fetch is logged and absorbed; the record keeps answering from what it holds, and the summary falls to the next basis or leaves a field empty.
- **Withhold rather than invent.** A value too old to describe the present is not presented as current, whatever its source.

Weather sources own what a fetch returns and how often a source has something new. The weather record owns what is stored, which fetches are due, and which source speaks for an input. Data Sync owns *when* a due fetch runs, what happens when it fails, how old a value may be and still be called current, and the assembly of the summary.

## Boundaries

| Neighbor (HLD component) | Direction | Contract at the boundary |
|---|---|---|
| Weather data access: sources | consumed | Each client's fetch for a station or point and span, returning rows in the record's units or an outage; each client's stated cadence |
| Weather data access: record | consumed | `due(now)` → the fetches that are due; the write of a fetch's rows and outcome; `fetch_summary(now)`, `fetch_gdd_ytd(today)`; `status()` |
| Agronomic rules, Recommendation feed, Disease risk, Timing windows, Seasonal plan, Soil-temperature outlook, MCP server | consumers | Receive a cloned `EnvironmentalSummary` from `summary()`. Fields may be `None` at any time; consumers must degrade, not fail. They read the record's other contracts directly, not through Data Sync |
| Timing windows | consumer of `AppState.climate_cache` | Owns what the climate record contains, the accessor and the data note shown when the record comes from the last good read. Data Sync owns the slot and its memo contract |
| Setup and sources (web UI) | consumer and trigger | Reads source status and backfill progress; a change it makes to the location, a pinned station or a sensor mapping wakes the schedule |
| Web UI | consumer | Renders `last_updated`, the stale-soil banner (from the summary's `soil_stale` flag; the UI applies no age limit of its own), the source status, and the Environmental page's refresh button |

## Runtime State (`AppState`)

`AppState` (`backend/src/state.rs`) is the Axum state cloned into every handler and the MCP server. All members are cheap clones.

| Field | Purpose |
|---|---|
| `pool` | The application database, which holds the weather record |
| `rules_engine` | Stateless rules, built once |
| `sync` | The summary service: the memoized summary and the latest forecast |
| `schedule` | A handle to the fetch schedule: wake it, read what it is doing |
| `openrouter` | LLM client when configured |
| `climate_cache` | 1 h memo of the multi-year climate record, with when its read completed |
| `mcp_enabled` | Whether `/mcp` is mounted; reported by the health check |
| `clock` | The lawn's clock, in the timezone of the lawn's location; every handler's "today". Replaced when the location's timezone changes |

### Climate-record memo contract

The memo is a single slot shared by every caller of `api::timing::climate_record` (timing windows, the seasonal plan's timing activities, and the MCP `weather_history` tool).

- **Key:** none; one record per process. The slot is emptied when the basis plan changes.
- **TTL:** 1 h, measured from when the read completed.
- **Hit:** checked under the read lock; a fresh entry is returned as a shared `Arc`.
- **Miss:** the record is read with no lock held; the result is stored under a short write lock. Concurrent misses each read and the last writer wins.
- **Failure:** a failed re-read is not memoized and leaves the slot unchanged. While the slot's record was read less than 24 h ago the accessor serves it, marked as coming from the last good read, and Timing windows adds a data note. Only when the slot is empty or its record is 24 h old or more does the error reach the caller.
- **Lifetime:** in memory only, empty at startup.

## The Fetch Schedule

One background task, started once the database is reachable and the migrations have run.

**The loop.** Once a minute, and at once when woken, the task asks the record what is due and runs those fetches. Each fetch is one call to a source client; its rows and its outcome are written to the record before the next begins. The task holds no lock on the summary while it fetches.

**Order.** Due fetches run in this order, so the present is never queued behind history:

1. The owner's sensors (Home Assistant entities).
2. The forecast.
3. Current hourly data: the stations in the basis plan and the hourly analysis.
4. Current daily data: the grid at the lawn's point, then at the calibration points; calibration stations.
5. Backfill, newest year first; within a year, the lawn's own sources before calibration sources.

After each backfill fetch the task returns to the top of the list, so backfill never delays a current fetch by more than one request.

**Rhythm.** A product is due when its source's cadence says there may be something new: hourly for stations, the analysis and the forecast; daily for the grids; every 5 minutes for Home Assistant. Hourly and daily fetches run at a minute offset chosen at random when the installation is first set up and kept thereafter, so installations do not all call a public host at the top of the hour.

**Failure.** A failed fetch is logged with the client's detail and recorded against its source and product. That product is next tried after 5 minutes, then 15, then every hour, until one succeeds. Other products of the same host are unaffected unless they fail themselves. The failure never reaches a page as an error: pages see it as source status and, in time, as an aging value.

**Catch-up.** After downtime the record's held spans are simply behind; `due(now)` reports the gap and the loop fills it in the order above. No separate catch-up mode exists.

**Waking.** A change to the location, a pinned station, a sensor mapping or the forecast provider wakes the task so the new plan starts filling at once. `POST /environmental/refresh` wakes it too, asking for every current product regardless of rhythm, at most once a minute.

**Stopping.** On shutdown the task finishes the fetch in progress or abandons it at the client's timeout, and starts no other. An interrupted backfill resumes from the record's held spans at next start.

**Nothing to do.** With no location set the plan is empty, nothing is due, and the task makes no requests.

**One process.** The schedule assumes a single running instance. A second instance against the same database would repeat fetches; the record's upserts make that harmless but it doubles the traffic, and it is not a supported arrangement.

### What a day costs

For a lawn with a soil station, steady state:

| Host | Requests a day |
|---|---|
| NOAA (one or two station files hourly, five calibration files daily) | About 30 to 55, most answered "not modified" |
| RCC-ACIS (the lawn's point and five calibration points) | About 6 |
| Open-Meteo (analysis and forecast together) | 24 |
| USDA (a SCAN station, where one is in the plan) | 24 |
| Home Assistant (local) | 288 |

The first start adds the backfill: about 15 hourly files per basis station, 15 daily files per calibration station, and one request per grid point.

## The Summary

`summary()` returns the `EnvironmentalSummary`. It is assembled from the record and memoized for one minute; within that minute callers receive the stored copy with no database read. A change of basis plan and a completed current fetch both discard the memo.

Assembly, in order:

1. **The record's summary.** `fetch_summary(now)` gives the live reading (each input's newest value from the source that speaks for it, with its basis and stamp), the 7-day means of 10 cm soil, air and humidity, the 7-day rain total and the soil trend.
2. **Age limits.** The record offers each input's best source; Data Sync says how old that source's newest value may be and still be current:

   | Kind of source | Current while no older than |
   |---|---|
   | Owner's sensor | 1 hour |
   | Hourly analysis | 3 hours |
   | Station | 72 hours |
   | Daily estimate (soil estimate, grid) | 72 hours after the end of the day it is for |

   An input whose best source is past its limit takes the next source in the plan that is within its own limit. With none, the input is withheld: its current value is `None`, and the 7-day figures built on it are `None` (the soil trend `Unknown`).
3. **Soil stamp and flag.** `soil_observed_at` is the stamp of the 10 cm soil value in use. `soil_stale` is true when soil temperature is withheld for age; every stale-soil banner reads it.
4. **GDD.** `fetch_gdd_ytd(clock.today())` → `gdd_base50_ytd`.
5. **Forecast.** The newest forecast the schedule fetched, attached only while it is less than 6 hours old.
6. **Stamp.** `last_updated` is the time of assembly.

A read of the record that fails leaves the previous summary in place for up to 5 minutes, after which the summary is returned with the weather fields empty.

### Field provenance

| Summary field | From | Notes |
|---|---|---|
| `current.soil_temp_*_f`, `current.soil_moisture_*` | The record's live reading | Per input: sensor, then station within 50 miles, then (5 and 10 cm temperature only) the estimate. Withheld past the age limit |
| `current.ambient_temp_f`, `current.humidity_percent` | The record's live reading | Sensor, then station within 10 miles, then the hourly analysis, marked approximate |
| `current.precipitation_mm` | The record's live reading | The newest hour from a rain sensor or the hourly analysis |
| Basis of each `current` field | The record | Carried through unchanged |
| `soil_observed_at`, `soil_stale` | Step 3 | |
| `soil_temp_7day_avg_f` (10 cm), `ambient_temp_7day_avg_f`, `humidity_7day_avg`, `precipitation_7day_total_mm`, `soil_temp_trend` | The record, the 7 local days ending now | Each with its basis; `None` when its input is withheld |
| `gdd_base50_ytd` | The record's resolved daily air, Jan 1 to today on the lawn's calendar | |
| `forecast` | The forecast client, via the schedule | At most an hour old when healthy; dropped at 6 hours |
| `last_updated` | Time of assembly | Not an observation time |

### API endpoints owned

| Method | Path | Behavior | Response |
|---|---|---|---|
| GET | `/api/v1/environmental` | `summary()` | `EnvironmentalSummary` |
| POST | `/api/v1/environmental/refresh` | Wakes the schedule for every current product (at most once a minute), waits up to 20 s for those fetches, then `summary()` | `EnvironmentalSummary` |
| GET | `/api/v1/health` | A 2 s database ping, source status, backfill progress, whether `/mcp` is mounted. `status` is `ok` or `degraded` on the database alone | See "Source Status" |

Neither environmental endpoint returns a source error.

## Source Status

Status is one entry per source in use, in the shape the environmental views define: id, name, kind, `ok`, last success, last failure, and the inputs it is the basis for.

- **`ok` is the outcome of the source's most recent scheduled fetch.** The schedule already calls every source at least hourly, so no separate probe runs. A source never yet fetched is reported as pending.
- **Backfill progress** is reported per source as the span held against the span wanted.
- Callers: `GET /api/v1/health`, `GET /api/v1/dashboard`, the Sources page and MCP `lawn_snapshot`. All read the record's `status()` and the schedule's forecast outcome; none makes an outside request.

## Failure and Degradation

| Failure | Effect on the summary | Recovery |
|---|---|---|
| A station stops reporting | Its inputs stay current for 72 h, then fall to the next basis (the estimate for soil temperature; nothing for soil moisture). `soil_stale` only if no basis is left | Automatic when it resumes |
| The grid cannot be reached | Daily air, rain and the estimate stay current for 72 h after their last day, then are withheld | Retried on the lengthening interval |
| The hourly analysis cannot be reached | Approximate air and humidity withheld after 3 h where no sensor or near station supplies them | As above |
| Home Assistant unreachable or an entity not numeric | That sensor's inputs fall to the next basis after 1 h | Next 5-minute read |
| The forecast provider fails | The last forecast is kept while under 6 h old | As above |
| The database cannot be read | The previous summary for up to 5 minutes, then empty weather fields | Next request |
| Backfill incomplete | The summary is unaffected: it needs only recent days | The schedule continues |
| No location set | Empty weather fields; the app directs the owner to setup | Setting a location |

## Concurrency

- **The schedule and the summary do not share a lock.** The schedule writes to the record through the pool. The summary service holds its memo behind a read-write lock that is taken for writing only to store a newly assembled summary, never across a database read or a fetch.
- **Simultaneous requests after the memo expires** may each assemble a summary; the last one stored wins. Assembly is a handful of local queries.
- **Fetches run one at a time.** The schedule is sequential, which is also what keeps it to one request at a time per host.
- **Consumers that read the record** (timing windows, disease risk, `/historical`, `/gdd`, the seasonal plan) use the pool directly and wait on nothing here.

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| When to fetch | A background schedule on each source's own rhythm | Fetch inside the request that needs the data; fetch on a single fixed interval | HLD decision: the models need an unbroken record, a first page after a quiet spell must not wait on downloads, and the owner's sensors need a history between visits. |
| Where the schedule runs | A task inside the application process | A separate ingester container; cron calling an endpoint | HLD: an installation is the app and its database. A second container or an external scheduler is one more thing a new owner must run. |
| Order of work | Sensors, forecast, current hourly, current daily, then backfill newest first, re-checking after each backfill request | Finish the backfill first; run everything in parallel | A first run backfills for minutes; today's conditions and this season's days should appear in the first of them. Parallel fetches would break one-at-a-time per host. |
| Fetch time within the hour | A random minute offset fixed per installation | The top of the hour | Many installations calling a public file server in the same second is the load pattern a polite client avoids. |
| Retry after a failure | 5 minutes, 15 minutes, then hourly, per source and product | Retry every loop; a fixed 5 minutes; give up after N tries | A short first retry rides out a blip; hourly is no more than the normal rhythm, so a long outage costs the host nothing extra and recovery is noticed within the hour. |
| Summary assembly | Read from the record on request, memoized one minute | Rebuilt by the schedule after each fetch and held in memory; no memo | The record is local and the queries small, so a request-time read is always consistent with the record. One minute spares a dashboard load's several handlers from repeating it. |
| Age limits | 1 h sensor, 3 h hourly analysis, 72 h station and daily estimate | One limit for everything; limits inside the record | A sensor is at the lawn and reports every few minutes, so an hour of silence is a fault; a station legitimately trails by a day or more. How old "current" may be is a judgment about presentation, which belongs with the summary. |
| Past the age limit | Fall to the next basis in the plan, then withhold | Withhold at once | A station down for a week should hand soil temperature to the estimate, labeled as such, not blank the lawn's advice (HLD: measured where near, estimated where not). |
| 7-day figures when an input is withheld | Withheld with it | Computed from whatever days exist | Rules read them as this week's weather. |
| Forecast reuse | The last forecast while under 6 h old | Drop on the first failure; reuse indefinitely | One failed fetch should not silence every forecast rule, but a forecast older than a few hours no longer describes the coming days. |
| Source status | The outcome of the last scheduled fetch | A separate probe per source, reused for 5 minutes | Every source is already called at least hourly; a probe would be an extra request that says the same thing. |
| Manual refresh | Wakes the schedule, at most once a minute | Fetch directly in the request; remove the button | The owner who has just fixed a sensor wants to see it. Routing it through the schedule keeps one-at-a-time per host; the floor keeps a held-down button from becoming traffic. |
| More than one instance | Not supported; harmless if it happens | A database lock electing one scheduler | A self-hosted single-owner app runs one process. The lock is machinery for a case the product does not have. |
| Climate-record memo | One in-memory slot, 1 h TTL, emptied when the plan changes | No memo; a stored copy | The 15-year resolve is the most expensive read and is requested by several pages; the current season must stay live. |
| Climate record when a re-read fails | Serve the last memoized record for up to 24 h, with a data note | Fail the caller once the memo expires | Fifteen years of history changes little in a day, while failing would blank the Timing page during a brief database problem. |
| Stale-soil threshold for display | Backend sends `soil_stale`; banners apply no limit of their own | Each banner compares the stamp with its own limit | One limit in one place: the banner and the withholding cannot drift apart. |

## Open Questions & Future Decisions

### Deferred

1. **Sub-hourly station data.** USCRN publishes 5-minute soil values; the schedule fetches hourly files only.
2. **A fetch that always fails for one station-year** (a corrupt file upstream) is retried hourly without end. Nothing marks a span as unobtainable.
3. **Refresh waits up to 20 s.** A slow source makes the Refresh button slow; the page could instead return at once and poll.
4. **No single-flight for the climate record or the summary.** Concurrent misses each read.
5. **The clock can change while a request is in flight** when the owner changes the location; that request finishes on the old calendar.

## References

- `docs/high-level-design.md`: the scheduled-fetch decision, polite fetching, graceful degradation.
- `docs/intent/weather-data/weather-record/`: `due`, `status`, the summary contracts, the basis plan.
- `docs/intent/weather-data/weather-sources/`: the clients and their cadences.
- `docs/intent/weather-data/environmental-views/`: the source-status shape and the banners.
- `docs/intent/timing-windows/`: the climate record's content and its data note.
- Code: `backend/src/logic/data_sync.rs` and `backend/src/logic/data_sync/` (the summary service and its age rules), `backend/src/state.rs`, `backend/src/api/{environmental,health}.rs`, `backend/src/api/timing/memo.rs`, `backend/src/models/environmental.rs`.
