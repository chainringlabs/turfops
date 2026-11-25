---
parent: weather-data
prefix: WX-VIEW
---

# Environmental Views

## Context and Design Philosophy

This leaf owns the read-only views of conditions: the Environmental page (`/environmental`),
the `GET /api/v1/historical` time series behind its charts, the rendering of
`GET /api/v1/environmental` (current conditions), the `Gauge` and `TrendChart` components, the
way a basis is shown on every page, and the shape of the source status that `/health` reports.
It holds no agronomic logic. Each number shown comes from a weather-record contract or from
data sync's cached summary.

Principles:

- **Always labeled.** No reading, series or card appears without what it rests on. A
  measurement, an estimate and the owner's sensor never look alike.
- **One answer everywhere.** The page shows conditions. Questions that a feature component
  answers (pre-emergent, seeding, disease risk) are not restated here with other numbers.
- **Withhold rather than invent.** A missing value renders as `--`, an input with no source
  says so, and an empty series is "No data available". Lag is disclosed rather than hidden.

The 10 cm soil-temperature outlook shown on this page (`PredictionChart`, its crossing cards)
is owned by the soil-temperature-outlook leaf. Data sync owns `/environmental`'s refresh logic
and the `/health` endpoint itself. The Sources page, where the owner inspects and changes the
sources, belongs to setup and sources (`docs/intent/setup-sources/`).

## Showing a Basis

One component, `BasisLabel`, is used wherever a weather-dependent value appears, on this page
and on every other.

| Basis | Label | Mark |
|---|---|---|
| Station | "Measured · *station name*, *N* mi" | Solid dot |
| Owner's sensor | "Your sensor · *sensor name*" | Solid square |
| Estimate | "Estimated for your location" | Hollow dot |
| Estimate, approximate | "Estimated for your location · approximate" | Hollow dot |
| Period on more than one basis | The newest basis, followed by "· mixed" | As the newest |

- The mark and the words always appear together; a basis is never carried by color or line
  style alone (Web UI convention).
- Each label links to the Sources page.
- **In a chart**, measured stretches are drawn solid, estimated stretches dotted in the same
  hue, and each change of basis is a thin vertical marker. The chart's caption lists the
  spans in words, and the table view has a basis column. Dashed stays reserved for forecast.
- **An input with no source** (soil moisture with no station or probe; deep soil on an
  estimate) shows "No source for this location" with a link to the Sources page, not `--`.
  `--` means a source exists and has no value now.

## API Endpoints

| Method | Path | Behavior | Errors |
|---|---|---|---|
| GET | `/api/v1/environmental` | Data sync's summary (`backend/src/api/environmental.rs`) | None: a source that fails leaves its values empty or on the next basis |
| POST | `/api/v1/environmental/refresh` | Data sync's refresh: wakes the fetch schedule for the current data and returns the summary that results | same |
| GET | `/api/v1/historical?range=7d\|30d\|90d` | See below (`backend/src/api/historical.rs`) | 400 for a bad range; the weather record's outages (location not set, history still loading) as 503 with their own sentences |

### What the summary says about each value's basis

The summary keeps its numbers where the rules and pages already read them
(`current.soil_temp_10_f`, `soil_temp_7day_avg_f`, …) and says what each rests on beside them:

- **`current_bases`**: one entry per input a live reading can fill, in the order the record
  lists them: the input's request name (`soil_temp_10`), its **standing**, and, when it has
  a current value, its basis and the time it was observed (for a daily source, the day it is
  for). The standing is `InUse` (a current value from a source in the plan),
  `NoCurrentValue` (the plan has a source, none is current) or `NoSource` (the plan has none).
- **`week_bases`**: for each 7-day figure that has a value, keyed by the figure's field name,
  its basis and whether the week was `mixed`.

A page finds a gauge's label by the input it shows; `NoSource` is the "No source for this
location" case and `NoCurrentValue` the `--` case.

### `/historical`

- **Hourly series.** The weather record's `fetch_range(now − range, now)`, reversed to
  ascending. 7d keeps every hour. 30d and 90d group the hours into 6-hour and 24-hour time
  buckets (aligned to UTC, each point stamped at its bucket's start): precipitation is summed
  and the other series are averaged over the hours that reported them (`series()`). Series:
  10 cm soil temperature, air temperature, RH, 10 cm moisture, precipitation (values ≥ 0). A
  missing value is left out of its series, not zero-filled; a bucket with no value for a
  series is left out of that series. A day whose soil is on a daily basis contributes one
  point at local noon. Default range: 7d.
- **Basis.** The response carries the record's basis spans for each series.
- **GDD.** `fetch_daily_gdd` from Jan 1 of the range-start year to today, accumulated with a
  reset each Jan 1 (agronomic rules' `gdd::accumulate_daily_gdd`), then filtered to the range
  and stamped 12:00 UTC. "Today" and the year are the lawn's local date (parent decision):
  `gdd_window(&clock, start, now)` (`backend/src/api/historical.rs`) turns the range's instants
  into local dates with the app's `Clock`, giving Jan 1 of the local range-start year to
  accumulate from, the first date shown and the last date (the local today).

## Source Status

Source status is one entry per source in use, not a fixed set of keys:

| Field | Meaning |
|---|---|
| `id`, `name`, `kind` | The source as the record names it (sensor, station, grid, analysis, estimate) |
| `ok` | The source's most recent scheduled fetch succeeded; `null` while it has never been fetched |
| `last_success`, `last_failure`, `last_error` | From the record's `status()`; `last_error` is the owner sentence for a failure newer than the last success |
| `inputs` | The inputs the plan ranks the source for, as named in requests |
| `held_from`, `held_through`, `wanted_from` | The history held against the history wanted, for a fetched source |

The entries come from the record's `status()`. The forecast has no source in the record; its
status (data sync's forecast status) is reported beside the entries, not as one of them. Data
sync assembles both for `/health`, the dashboard's source-status line and MCP `lawn_snapshot`.
A source that is in the plan but the basis for nothing is listed with empty `inputs`.

## Environmental Page

`frontend/src/pages/Environmental.tsx`, route `/environmental`.

- **Loading.** Loads `/environmental` and `/soil-temp-forecast` together. A forecast failure
  becomes `null` and hides its section. The page polls every 30 s **only while the tab is
  visible**, pauses when hidden, and refetches when the tab becomes visible. "Refresh Now"
  calls the POST endpoint. Errors show in a banner as `describeError(e)` (Web UI's
  human-readable errors), and stale responses are discarded with an `AbortController`.
  `/historical` loads separately for the selected range; a failed load clears the trend charts
  and shows its own plain-language error in their place, so a range change never leaves the
  previous range's data under the new selector.
- **Header.** "Last updated" (`last_updated`), the soil trend, and "Soil observed …"
  (`soil_observed_at`, the source's own time for the soil values; for an estimate, the day it
  is for). When data sync marks the soil stale (`soil_stale`), a notice says the weather station has not
  published new soil readings since that date, that the network publishes late from time to
  time and there is nothing for the owner to do, and that readings and soil-based advice are
  paused until it does. It is a notice, not an error: nothing is broken on the owner's side.
  The dashboard shows the same notice.
- **Gauges.** Four `Gauge`s (`frontend/src/components/Gauge.tsx`, configs in
  `components/gaugeConfigs.ts`, `gaugeLevel`). The bar is green, turns amber at `warn` and red
  at `critical` — and, on a gauge with a dry side, amber below its dry `warn` and red below its
  dry `critical`. A warning or critical state is also said in words beside the value ("High",
  "Very high", "Dry", "Very dry"), and the numeric value is always shown (`--` when null). Each gauge carries its `BasisLabel`.

| Gauge | Range | Warn / critical | Dry warn / critical |
|---|---|---|---|
| Soil Temp (10 cm) °F | 30–100 | 75 / 85 | — |
| Ambient Temp °F | 0–110 | 85 / 95 | — |
| Humidity % | 0–100 | 80 / 90 | — |
| Soil Moisture (10 cm) % (fraction × 100) | 0–50 | 40 / 45 | below 15 / below 10 |

- **Summary.** Four 7-day summary cards (precipitation in inches), a soil profile table with
  one row per depth that has a source (5 and 10 cm always; 20, 50 and 100 cm only from a
  station or probe), each with its basis, and **Forecast** cards for the first 7
  `daily_summary` days with the rest behind "Show more" (high/low, RH, precipitation in
  inches or "not forecast", max `pop`), headed by the provider's name. A day the forecast covers for fewer than 24 hours
  (`hours_covered`) is marked "partial day".
- **Soil Temperature Forecast.** The outlook leaf's `PredictionChart` and crossing cards.
- **Historical Trends.** A 7d / 30d / 90d selector over six `TrendChart`s
  (`components/TrendChart.tsx`, a Recharts area chart, "No data available" when empty, an
  optional dashed red reference line). Reference lines (`TREND_REFERENCES` in
  `components/environmental/display.ts`): air at 85 °F "Heat stress", RH at 80 % "Humid (80%)"
  and moisture at 0.10 "Drought". Soil temperature, GDD and precipitation have none:
  soil-temperature and GDD milestones are the timing windows' and the outlook's questions, and
  disease risk comes from the per-disease models. Each chart shows its basis as above.
- **Attribution.** A footer names every source the page drew on, with the credit its terms
  require.

`frontend/src/utils/units.ts`: `mmToInches` and `formatInches` (25.4 mm per inch). The page is
the only place precipitation becomes inches.

## Failure and Degradation

| Failure | Effect |
|---|---|
| The record cannot answer `/historical` | 503. The trend section clears its charts and shows the record's sentence. Current conditions stay at data sync's last good summary |
| A source in use fails | Its status entry turns `ok: false`; values it supplied fall to the next basis or are withheld by the record and data sync, and the labels change with them |
| No source for an input | "No source for this location" in place of the gauge value, row or chart |
| Forecast unavailable | The Forecast cards disappear once data sync's reuse window expires |
| `/soil-temp-forecast` fails | Its section is hidden; the rest of the page renders |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Source status shape | A list with one entry per source in use | Fixed keys per kind of source | Which sources exist depends on the location and the owner's sensors; fixed keys cannot name two stations or an unknown number of sensors. |
| Basis presentation | One shared label: words plus a mark, linked to the Sources page; dotted line for estimated stretches | A legend per page; color per basis; a single page-level note | Every page shows weather-dependent values, and the HLD requires each to name its basis the same way. A page-level note cannot express a series that changes basis partway. |
| No source versus no value | Different text | `--` for both | "Nothing measures this here" is a permanent fact the owner can change on the Sources page; "no value right now" is an outage. |
| Environmental page refresh | 30 s polling, visibility-aware, plus a manual force-refresh | Push or SSE; no polling | Data sync's staleness limits keep polling cheap, and a hidden tab makes no calls. [inferred] |
| `/historical` point budget | Aggregate longer ranges into 6 h (30d) and 24 h (90d) time buckets: precipitation summed, other series averaged | Return every hour; keep every Nth row | Keeps charts responsive while rain totals stay true and spacing stays even when hours are missing. |
| Reference lines | Heat stress, Humid (80%), Drought only | Lines for pre-emergent, crabgrass and "disease risk" | One answer everywhere: those questions belong to timing windows and disease risk, at their own depths and models. |
| Lag disclosure | Soil observation time and the stale banner, as on the dashboard; partial forecast days labelled | `last_updated` only | `last_updated` is the app's clock; the source's lag is the fact the owner needs (lag carried as data). |
| Gauge warnings | A text label ("High", "Dry") beside the colored bar | Color alone | State is never carried by color alone (Web UI convention). |
| Moisture gauge | Warns wet (40 / 45 %) and dry (amber < 0.15, red < 0.10) | Wet only | Drought is the more common action; the thresholds are the irrigation rule's. |
| Forecast cards | The first 7 days, the rest on request | Every day the provider returns | The default provider returns 16 days; the far days are low-skill and would push the trends off the screen. |

## Open Questions & Future Decisions

### Deferred

1. **Whether an estimated "current" soil temperature belongs on a gauge.** It is a daily mean
   for yesterday or earlier, shown with its day. A gauge implies a live reading.

## References

- Parent: `docs/intent/weather-data/weather-data-design.md`.
- `docs/intent/weather-data/weather-record/`: `fetch_range`, `fetch_daily_gdd`, `status()`, the
  summary shape and the basis types.
- `docs/intent/weather-data/soil-temp-outlook/`: the chart and crossing cards on this page.
- `docs/intent/data-sync/`: refresh, `check_connections()`, `/health`.
- `docs/intent/web-ui/`: shell, shared styles, status-palette and table-view conventions.
- Code: `backend/src/api/{environmental,historical}.rs`, `backend/src/models/historical.rs`,
  `frontend/src/pages/Environmental.tsx`,
  `frontend/src/components/{Gauge,TrendChart}.tsx`, `frontend/src/components/gaugeConfigs.ts`,
  `frontend/src/components/environmental/display.ts`,
  `frontend/src/utils/units.ts`.
