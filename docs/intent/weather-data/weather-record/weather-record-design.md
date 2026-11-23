---
parent: weather-data
prefix: WX-REC
---

# Weather Record

## Context and Design Philosophy

This leaf owns the stored weather for one lawn: the PostgreSQL tables the source clients'
output is written to, the catalog of measured stations and the choice among them, the rule that
resolves one value per input per day from several sources, the first-run backfill, and the
query contracts every other component reads.

It sits between two neighbours. Weather sources (`WX-SRC`) fetch and normalize: each client
answers "what does this source say for this station or point over this span", in the record's
units. Data sync decides *when* to fetch. The record stores what it is handed, says what is
missing or due, and answers questions about the lawn.

Principles, from the parent:

- **Sources are kept apart; the choice is made on read.** Every stored row belongs to one
  source. No table holds a blended series, so a change of cut-off, a newly mapped sensor or a
  pinned station changes answers without rewriting history.
- **A contract returns the basis with the value.** A caller cannot obtain a number from the
  record without also obtaining what it rests on.
- **Missing stays missing.** A day no eligible source covers is absent from a daily contract,
  and a field is `None`, never zero and never carried forward.

## Inputs

The record resolves these inputs. The cut-offs are the parent's.

| Input | Grain | Station cut-off | Estimate source |
|---|---|---|---|
| Soil temperature at 5 and 10 cm (each its own input) | Hourly from a sensor or station; daily from the estimate | 50 mi | The soil estimate (`WX-EST`) |
| Soil temperature at 20, 50 and 100 cm | Hourly | 50 mi | None |
| Soil moisture at each depth | Hourly | 50 mi | None |
| Hourly air temperature and humidity | Hourly | 10 mi | The hourly analysis, approximate |
| Daily air maximum and minimum | Daily | No station | PRISM at the point, the NRCC grid behind it |
| Rain | Daily, with hourly amounts where the source has them | No station | PRISM daily at the point; the hourly analysis for days PRISM has not published |

## What Is Stored

| Table | One row per | Holds |
|---|---|---|
| `weather_station` | Station in a network's catalog | Network (USCRN, SCAN), the network's station id, name, state, latitude, longitude, elevation, which inputs it reports, first and last year of record |
| `weather_source` | Source in use for the lawn | Kind (sensor, station, grid, analysis, estimate), display name, the station or grid point it refers to, distance from the lawn in miles (stations), role (basis or calibration) |
| `weather_hourly` | Source and UTC hour | Air temperature, relative humidity, precipitation, soil temperature and soil moisture at 5 / 10 / 20 / 50 / 100 cm. Every measurement column is nullable |
| `weather_daily` | Source and day | Air maximum, minimum and mean, precipitation, soil temperature at 5 and 10 cm. Used only by sources that publish daily values: the grids, the soil estimate, and the calibration stations |
| `weather_sensor_reading` | Sensor source, input and instant | Readings as they arrive, kept 7 days |
| `weather_basis_plan` | Input and rank | The source at that rank, and whether the owner pinned it |
| `weather_fetch_state` | Source and product | The span held (`held_from`, `held_through`), the span wanted, last attempt, last success, last failure |

Rules that hold for every table:

- **Units are fixed at the door.** Temperatures are °F, precipitation mm, humidity percent,
  soil moisture a fraction from 0 to 1. Clients convert and drop sentinels before handing rows
  over; the record stores nothing it would have to interpret later.
- **Hourly rows are keyed by source and UTC hour; writes are upserts that replace the row.**
  Station networks revise recent hours after quality control, and a re-fetch must win,
  including where it blanks a value published earlier.
- **Daily rows keep the source's own day.** PRISM's day ends at 12:00 UTC. The record applies
  the mapping to lawn-local days on read (see "Daily sources and the lawn's day").
- **Daily values of an hourly source are computed on read**, by bucketing its hours into
  lawn-local days in SQL. No derived daily copy of a station or sensor exists to fall out of
  step with its hours or with a changed timezone.
- **A sensor reading becomes part of its hour.** Each arriving reading is stored as received
  and the sensor source's hourly row is recomputed from that hour's readings: the mean, or for
  rain the hour's total. The live reading uses the newest raw reading.

### How long each source is kept

| Source | Span held |
|---|---|
| The station chosen for an input | Hourly, 15 calendar years plus the current year |
| PRISM (and NRCC) at the lawn's point | Daily, the same span |
| The soil estimate | Daily, the same span |
| Calibration stations (the five nearest USCRN stations with soil) | Daily, the span `WX-EST` asks for |
| The hourly analysis | Hourly, from 93 days before the first start, then kept |
| A sensor | Hourly from its first reading, kept without limit |

Fifteen years is what the timing windows' typical dates read.

## The Station Catalog and the Basis Plan

The catalog is every USCRN and SCAN station in the contiguous United States. It is loaded from
the networks' own station lists at first start and refreshed monthly.

The **basis plan** is the ranked list of sources for each input. The record builds it whenever
the lawn's location is set or changes, a sensor is mapped or unmapped, the owner pins or unpins
a station, or the catalog refresh changes a station in use:

1. The owner's sensor mapped to the input, if any. A sensor mapped to air temperature also
   speaks for daily air, whose maximum, minimum and mean are taken from its hours.
2. For inputs with a station cut-off: the owner's pinned station if there is one. Otherwise the
   nearest USCRN station within the cut-off that reports the input, then the nearest SCAN
   station within the cut-off that reports it. A station more than 1,000 ft above or below the
   lawn is skipped unless pinned.
3. The input's estimate source, if it has one.

For hourly air a default station must report both temperature and humidity, because dew point
needs the two from one instrument. A grid, the hourly analysis and the soil estimate are
sources at the lawn's point: a lawn that moves gets new ones, and the old point's rows are not
read for the new location.

Distance is great-circle miles between the lawn and the station. The lawn's elevation is the
one the hourly analysis reports for the point. It is kept with the lawn profile, written by
the first analysis fetch. Until that is known the plan is built without
the elevation rule, and it is rebuilt when the elevation arrives. A pinned station replaces the default stations
for that input at any distance and is labeled with its true distance.

Without a location the plan is empty and every contract answers with a "location not set"
outage.

## Resolving a Value

Resolution answers: for this input on this lawn-local day, which source speaks?

**A complete day** goes to the highest-ranked source in the plan that *covers* it:

| Source grain | Covers a day when |
|---|---|
| Hourly | It has at least 12 valid hourly values of the input on that day |
| Daily | It has a value for that day |

**A day a source has not reached yet is left empty, not filled from below.** Stations trail
real time by a day or two. A source is *pending* for a day when its newest value is within its
age limit (data sync's, by kind of source) and falls on that day or an earlier one. While a
higher-ranked source is pending for a day, no lower-ranked source supplies that day: the input
is empty there. A lower-ranked source fills a day only when every source above it either
covers too little of a day it has already passed, or is past its age limit. Only a sensor or a
station is waited for in this way. A grid or the hourly analysis that is late is filled from
the next estimate at once, which is what the NRCC grid behind PRISM is for. Rain is the other
exception (below).

**The day in progress and the live reading** go, per input, to the highest-ranked source whose
newest value is within the age limit data sync sets for that kind of source. Under that limit
each input takes its newest non-null value, so a probe that skips an hour does not blank the
reading. When the source that speaks is daily, the current value is its newest day's value,
stamped with that day.

Three refinements:

- **Air temperature and humidity are one input for the disease contract.** Dew point and humid
  hours need both from the same instrument, so a day's disease inputs come from the highest
  ranked source that has both for 12 hours. A sensor that measures only one of them still
  supplies the live reading and the 7-day mean for what it measures.
- **An estimate inside a measured series is anchored.** Where the soil estimate fills days
  between or after a station's or sensor's values, the record uses the estimate's anchored form
  for those days, which `WX-EST` defines, so a change of basis is not a step in the series.
  This holds for every contract and for the live reading. So that a day gets the same value
  whatever span it is asked in, the record looks for the measured days around a gap up to a
  year either side of the span; a gap longer than that is anchored from the edge in reach.
- **Rain's newest days.** A lawn day PRISM has not yet published takes its total from the
  hourly analysis and is replaced when PRISM arrives. Rain is never held back for a pending
  source: a rain sensor that reports only when it rains would otherwise leave every dry day
  empty.

### Daily sources and the lawn's day

| Source value | Lawn-local day it is assigned to |
|---|---|
| PRISM or NRCC maximum and minimum for day *D* | *D* |
| PRISM precipitation for day *D* | *D* − 1 |
| Soil estimate for day *D* | *D* |
| Calibration station daily values | Not resolved for the lawn; read by `WX-EST` only |

PRISM's day *D* runs from 12:00 UTC on *D* − 1 to 12:00 UTC on *D*, so most of its rain fell on
the lawn's previous day.

## Basis in the Contracts

```
Basis      { kind: Sensor | Station | Estimate, source: name, distance_miles: Option, approximate: bool }
BasisSpan  { input, from: date, to: date, basis }
```

- A value from the hourly analysis is `Estimate` with `approximate: true`. A value from PRISM
  or the soil estimate is `Estimate`. The source name says which.
- A **series** contract returns its rows plus `basis: Vec<BasisSpan>`: for each input in the
  rows, the series' successive days on the same basis collapsed into one span. A series on one
  basis throughout has one span per input; a seam is the boundary between two spans. A day
  with no value does not end a span.
- The **live reading** carries one `Basis` per filled input.
- A **scalar** over a period (a 7-day mean, the year-to-date GDD) carries the basis of the
  newest day in the period, and `mixed: true` when the period used more than one.

## Query Contracts

Names and shapes are the ones consumers already use. Each gains its basis as above.

| Contract | Returns | Resolved from | Consumer |
|---|---|---|---|
| `fetch_latest` | `Option<EnvironmentalReading>`: each input's current value and its stamp | Live-reading rule, per input | data sync (via `fetch_summary`) |
| `fetch_range(start, end)` | Hourly `EnvironmentalReading`s, newest first | Per day, each input from the source that covers the day; a day on a daily basis yields one reading at local noon | `fetch_summary`, `/historical` |
| `fetch_summary(now)` | `EnvironmentalSummary`: `current`; 7-day means of 10 cm soil, air and humidity; 7-day rain total; soil trend | The 168 hours ending at `now`. A day on a daily basis contributes its value for each of its hours that fall inside those 168 | data sync |
| `fetch_daily_soil_temp_averages` | `(date, soil_temp_10_f)` | Resolved 10 cm soil per complete day | seasonal plan |
| `fetch_daily_paired_averages` | `(date, air_mean_f, soil_temp_10_f)` for days with both | Daily air mean and resolved 10 cm soil | soil-temperature outlook |
| `fetch_daily_gdd(start, end)` | `(date, max_f, min_f, gdd50)` | Resolved daily maximum and minimum; `gdd50 = max((max + min) / 2 − 50, 0)` | `/historical`, the GDD endpoint |
| `fetch_gdd_ytd(today)` | Sum of `gdd50` from Jan 1 of `today`'s year; `None` with no days | As above | data sync |
| `fetch_daily_disease_inputs(start)` | One `DailyWeather` per local day: usable hours, air mean / min / max, humidity mean, hours at RH ≥ 90 %, leaf-wetness hours, rain total, peak dew point, dawn and afternoon coverage; °C | Hours of the day's air-and-humidity source; the rain total from the rain basis; wet hours from the same hourly source's precipitation | disease risk, MCP `weather_history` |
| `fetch_climate_record(start)` | One `ClimateDay` per local day: `soil_temp_5_f`, `air_min_f`, `air_avg_f`, `gdd50` | Resolved 5 cm soil; resolved daily air; a day missing one side keeps the other with `None` | timing windows, MCP |
| `due(now)` | The fetches that are due: source, product and span | `weather_fetch_state` against each client's cadence | data sync |
| `preview(point)` | The basis plan the record would build for a point, and the stations near it with distance, elevation difference and inputs reported; writes nothing | The catalog and the cut-offs | setup and sources |
| `status()` | Per source: last success, last failure, span held, backfill progress | `weather_fetch_state` | data sync (`/health`), environmental views |

Details consumers rely on:

- **Daily air mean** is the mean of the day's hours from a sensor, and `(max + min) / 2` from
  the grid.
- **Trend** compares the mean 10 cm soil temperature of the 24 hours ending at the newest
  value with the 24 before: more than +2 °F is `Rising`, less than −2 °F is `Falling`,
  otherwise `Stable`. On a daily basis it compares the newest day with the day before. It is
  `Unknown` when either period has no value, and when the two periods rest on different
  sources: a change of source is a seam, not warming or cooling.
- **Current rain comes from an hourly source only.** For every other input a daily source's
  newest day stands as the current value. A day's rain total does not: an earlier day's rain
  is not falling now. The day in progress takes its rain from a sensor or the hourly analysis.
- **A disease day's rain total is zero where the rain basis has nothing for it.** The models
  read it as a number, and the lake they were built on did the same.
- **The last day of a daily contract is usually partial** on an hourly basis. Whether a day is
  usable stays with the consumer, as does `hours_covered`.
- **No contract publishes a "today".** Callers pass the lawn's date from the app's `Clock`.

## Backfill and Keeping Current

`weather_fetch_state` records, per source and product, the span wanted and the span held. The
difference is what `due(now)` reports, one calendar year at a time and newest first, so the
current season fills before 2012 does and progress can be shown in years. Every current fetch
is reported before any backfill. A source new to the plan gets one current fetch first: the
year so far for a station, the last 30 days for a grid, the whole 93 days for the analysis.
The NRCC grid is asked only for the days after PRISM's newest, and holds no history. A fetch that fails leaves the state unchanged apart from its failure stamp; the same span
is due again. Writes are upserts, so a repeated or overlapping fetch is harmless and a backfill
interrupted by a restart resumes where the held span ends.

Recent spans are re-fetched to pick up revisions: a station's last 3 days on each fetch, and
PRISM's last 7 days daily.

Until every source in an input's plan holds its wanted span, contracts that read long history (`fetch_climate_record`,
`fetch_daily_soil_temp_averages`, `fetch_daily_paired_averages`) answer with a "history still
loading" outage naming the progress. Waiting for every source keeps typical dates from appearing on the estimate and
then moving when a station's history lands. Contracts over recent days answer as soon as
their own window is held.

When the plan changes, sources that left it stop being fetched and new ones start with an empty
held span. A source no longer in the plan is deleted with its rows once every fetched source in
the plan holds its wanted span. Sensors and calibration sources are never deleted this way.

The station catalog is due for refresh when a network has no stations or was last read 30 days
ago. The record answers that on its own too, so setup can load the catalog before the lawn has
a location or a plan.

## Failure and Degradation

| Failure | Effect |
|---|---|
| No location | Every contract: "location not set" outage |
| A source cannot be reached | Nothing is written; held data keeps answering. Data sync's age limits decide when the live reading falls to the next basis or is withheld |
| A station's probe drops hours | Days with at least 12 valid hours stay measured; the others fall to the next source in the plan for that input, and the basis spans show it |
| A station stops reporting | Its days end. Days after its last one stay empty until it is past its age limit, then come from the next source. For soil that is the anchored estimate; for soil moisture, nothing |
| PRISM unavailable | Daily air and rain come from the NRCC grid and the hourly analysis for the days missing, replaced when PRISM returns |
| The hourly analysis unavailable, no station within 10 mi, no sensor | No hourly air or humidity: disease inputs stop at the last day held; daily air, soil and rain are unaffected |
| Backfill incomplete | Long-history contracts: "history still loading" outage |
| The database cannot be read | The request fails as any database error does; there is no separate weather store to fall back to |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| When the basis is chosen | On read, from per-source rows | A resolved table written at ingestion | A resolved table must be rebuilt whenever a sensor is mapped, a station pinned, a cut-off changed or a late revision arrives, and it loses which source a value came from unless it duplicates it. Reads are small: 15 years of one station is about 130,000 hourly rows. |
| Hourly table shape | One wide row per source and hour, nullable columns | One row per source, hour and input | Every consumer reads several inputs of the same hour together, and the per-sensor fallback is a column-wise newest-non-null. A long table multiplies rows fifteenfold for no query it makes easier. |
| Daily values of hourly sources | Aggregated on read into lawn-local days | A stored daily rollup | The rollup depends on the lawn's timezone and on every late revision; computing it removes a second copy that can disagree. The climate record is already memoized for an hour by its consumer. |
| History of the chosen station | Hourly for the whole 15 years | The network's daily file for history and hourly only for recent years | One coverage rule (12 valid hours) for every day, old or new. The daily file hides how many hours its mean rests on, and the dropout investigation shows that matters. |
| Coverage rule | 12 valid hourly values of the input per local day | Any value; 18 hours; the whole day | The rule the timing windows and seasonal plan already rest on; it rejects a day known only by its afternoon. |
| Station choice | Nearest USCRN within the cut-off, then nearest SCAN, one of each | Nearest of either network; several stations averaged | USCRN is the network the cut-off and every accuracy figure were measured on; SCAN is unscored and fills its gaps. Averaging stations manufactures a series no instrument recorded. |
| Elevation guard | Skip a station more than 1,000 ft above or below the lawn | Distance alone | The 50-mile figure was measured on pairs within 1,000 ft of each other; in mountains a near station can sit in a different climate. |
| Pinned station | Replaces the default for that input at any distance, labeled with its distance | No override; an override limited to the cut-off | The owner may know a station across a ridge is the wrong one, or that a farther one matches their lawn. The label keeps the claim honest. |
| Where units are converted | In the source client, before storage | Store native units and convert in each contract | One conversion per value, at the only place that knows the source's units and sentinels. |
| PRISM rain's day | Assigned to the lawn's previous day | Left on PRISM's day | Against 133 gauges PRISM's daily amounts match best shifted one day, as its 12:00 UTC day boundary predicts. |
| PRISM temperature's day | Left on PRISM's day | Shift the maximum to the previous day | Freeze dates and GDD were scored unshifted (1 \| 9 and 2 \| 5 days). The minimum falls near dawn of the labeled day. |
| Sensor readings | Stored raw for 7 days and folded into hourly rows | Hourly rows only; raw kept forever | The live reading needs the newest instant, the models need hours. A sensor reporting each minute would otherwise write half a million rows a year per input. |
| Location change | Rebuild the plan; delete the old point's station and grid rows once their replacements are backfilled; keep sensor rows | Keep every location's data | The record describes one lawn. Old points would be dead weight no contract reads. A sensor is the owner's and travels with them. |
| Seven-day figures | The 168 hours ending now | The 7 local days ending now | A rule reading the rain total in the morning should see a full week, not six days and a morning. |
| A revised value that is now blank | Replaces the held value | Keep the earlier value | The network withdrew the reading; keeping it would preserve a value its own quality control rejected. |
| A day the station has not reported yet | Empty until the station reports it or passes its age limit | Fill it from the estimate at once | The estimate would be shown for a day or two and then replaced by the measurement, so a crossing dated on Monday could move on Wednesday, and one surface could call a day estimated while another showed the station as current. |
| Long history while loading | A "history still loading" outage | Answer from the years held so far | Typical dates from three of fifteen years would be shown with the confidence of fifteen (HLD tenet: withhold rather than invent). |
| Disease inputs in °C | Kept in °C at the contract, converted from stored °F | Change the contract to °F | The disease models' published constants are in °C; the contract keeps the shape its consumer was verified on. |

## Open Questions & Future Decisions

### Deferred

1. **A scalar's basis when the period is mixed.** `mixed: true` says that it happened, not how
   much of the period each basis supplied. The spans are available from the series contracts.
2. **PRISM's maximum on the lawn's day.** Left unshifted on the evidence of freeze dates and
   GDD; a daily-maximum consumer (heat stress on observed days) would want it scored.
3. **Training the soil outlook on an estimated soil series.** `fetch_daily_paired_averages`
   returns pairs whose soil side may itself be modeled from air. The pairs carry their basis;
   what the outlook does with estimated pairs belongs to `WX-OUTLOOK` and `WX-EST`.
4. **Catalog staleness.** A station that closes between monthly refreshes simply stops
   reporting and its days fall to the next source; the plan is not rebuilt until the refresh.
5. **Hourly rain from a station.** The station's gauge is stored with its other hourly columns
   but is in no plan, because rain by station distance was not scored.

## References

- Parent: `docs/intent/weather-data/weather-data-design.md` (the per-input cut-offs, basis as
  data, one clock, consumer boundaries).
- Siblings: `weather-sources/` (clients, units, cadence), the soil estimate (`WX-EST`: the
  series, its anchored form, calibration), `soil-temp-outlook/`, `environmental-views/`.
- `docs/intent/data-sync/`: the fetch schedule, age limits on the live reading, `/health`.
- `docs/intent/applications/`: the lawn profile, which holds the location.
- `docs/intent/deployment/`: configuration, migrations, the outage vocabulary in `error.rs`.
- [docs/plans: standalone-weather-sources](../../../plans/standalone-weather-sources.md): every
  accuracy figure quoted here.
- [docs/plans: uscrn-soil-dropout-investigation](../../../plans/uscrn-soil-dropout-investigation.md).
