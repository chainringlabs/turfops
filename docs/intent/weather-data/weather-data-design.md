---
parent: high-level-design
prefix: WX
---

# Weather Data

## Problem

Every agronomic answer in TurfOps starts from the conditions at one lawn, and no single free
source describes them. Measured soil stations are sparse: about half the US population lives
more than 50 miles from one. Gridded air temperature covers every location but is daily and has
no soil. Hourly humidity for an arbitrary point exists only as an analysis. The owner may have
sensors of their own, with no history. A forecast covers only the future. These sources differ
in grain, units, lag, licence and failure mode, and their probes fail without notice.

Five feature components and three read-only views need these conditions. If each chose its own
source and opened its own reads, they would disagree about what "now", "today" and "the last
7 days" mean, and an estimate could be shown somewhere as if it were a reading.

## Approach

This sub-HLD groups the source clients, the stored record, the soil estimate, and the views
built directly on them. Five ideas hold across all of its children.

### One record, written by sources and read through contracts

TurfOps fetches every source itself and stores what it fetched in PostgreSQL: hourly
observations, daily grid values, the owner's sensor readings and the modeled soil series.
Feature components never call a source and never query the tables. They consume published
contracts (the live reading, the 7-day summary, daily disease inputs, the climate record and
the rest), so a change of source or basis reaches every consumer at once without any of them changing.

### The basis is chosen per input and per day

For each input (air temperature, soil temperature by depth, humidity, rain, soil moisture) and
each day, the record resolves one value from the best basis that has one:

1. the owner's sensor mapped to that input;
2. a measured station within that input's cut-off: USCRN, then SCAN;
3. the estimate for the location.

The cut-off is a property of the input, because a soil reading describes a wider area than an
air reading does:

| Input | Station cut-off | Estimate |
|---|---|---|
| Soil temperature, 5 cm and 10 cm | 50 miles | The modeled soil series |
| Soil moisture | 50 miles | None |
| Hourly air temperature and humidity | 10 miles | The hourly analysis, labeled approximate |
| Daily air maximum and minimum | No station | PRISM at the lawn's point |
| Rain | No station | PRISM daily at the lawn's point; the hourly analysis for days PRISM has not published |

The owner can pin a station for an input, which replaces the default choice for it.

A station whose soil probe is out for a week still supplies hourly air for that week while
soil temperature comes from the estimate. A sensor mapped yesterday supplies yesterday onward;
the years before it come from the next basis. Soil moisture stops at step 2: it has no
estimate.

### The basis is carried as data

Every value a contract returns carries what it rests on: the basis (sensor, station or
estimate), the source's name, and for a station its distance. Consumers pass it through; the
views and the MCP tools show it. A series that changes basis partway says where.

### Lag and gaps are carried as data

Stations trail real time by hours to two days, the newest local day is usually partial, the
grid's newest days are less settled than older ones, and the forecast starts and ends mid-day.
Contracts expose this instead of hiding it: the observation stamp, per-day hour coverage,
partial-day flags and `full_days()`. Missing values stay `None` all the way to the UI, and
sentinels are dropped rather than averaged in.

### One clock

"Today", every date gate and every year boundary use the lawn's local date: the app's `Clock`
(`backend/src/clock.rs`), in the timezone of the lawn's location; `TZ`, when set, overrides
it. Observations are stored in UTC. Hourly observations and forecast slots are
bucketed into lawn-local calendar days, daylight-saving aware, each by the lawn-local date of its own
time. A source that publishes only daily values is stored under that source's own day. No
source publishes a "today" of its own.

## Children

| Child | Prefix | Owns |
|---|---|---|
| [Weather sources](weather-sources/weather-sources-design.md) | `WX-SRC` | One client per source: the USCRN and SCAN station networks, the PRISM grid (RCC-ACIS), the hourly analysis, the owner's sensors, and the forecast providers. What each fetch returns, its units, sentinels and quality flags, its publishing cadence, and its terms of use and attribution. `WeatherForecast` with local-date bucketing and the day-level helpers (`full_days()` and friends) |
| [Weather record](weather-record/weather-record-design.md) | `WX-REC` | The PostgreSQL schema for stored weather, the station catalog and distance lookup, per-input per-day basis resolution, the backfill, and the query contracts every consumer reads (`EnvironmentalReading` / `EnvironmentalSummary`, daily disease inputs, the climate record, GDD), each carrying its basis |
| [Soil estimate](soil-estimate/soil-estimate-design.md) | `WX-EST` | The modeled soil-temperature series for a location: the regression of soil on gridded air with a day-of-year term, its calibration on the five nearest USCRN stations, the depths it produces (5 cm and 10 cm), which threshold crossings it may date, and the scoring that keeps its published accuracy true |
| [Soil-temperature outlook](soil-temp-outlook/soil-temp-outlook-design.md) | `WX-OUTLOOK` | The 10 cm air → soil outlook, the direction-aware 10 cm thresholds, `GET /soil-temp-forecast`, `PredictionChart`, the dashboard `SoilTempForecastWidget`, and the `fit_model` / `predict_soil_temps` contract reused by timing windows |
| [Environmental views](environmental-views/environmental-views-design.md) | `WX-VIEW` | The Environmental page, `GET /historical`, the rendering of `/environmental`, the gauges and `TrendChart`, how a basis is shown, and the shape of source status. The Sources page (per input: the source in use, what stands behind it, freshness, history held, accuracy and attribution) belongs to setup and sources (`docs/intent/setup-sources/`), which reads the record's `status()` and basis plan |

Weather sources and the weather record are the foundation. Sources know nothing of the lawn:
each client answers "what does this source say for this station or point over this span". The
record knows nothing of HTTP: it stores what clients return and resolves the basis. The soil
estimate reads gridded air and neighbouring stations' soil from the record and writes its
series back to it as one more source. The outlook and the views consume the record's contracts
exactly as outside components do.

## Boundaries to Consumers

| Component | Consumes | Owned there, not here |
|---|---|---|
| Data sync | Each source client's fetch and stated cadence; the record's "what is missing or due" answer; `fetch_summary`, `fetch_gdd_ytd` | *When* to fetch: the schedule per source, the backfill run, catch-up after downtime, retry policy, the staleness limits on what is shown as current, merging into the cached `EnvironmentalSummary`, and `/health` itself |
| Timing windows | `fetch_climate_record`, `fit_model` / `continuous_air` / `anchored_outlook`, `WeatherForecast` | Windows, boundaries, climatology, the 5 cm outlook's depth and training window, what a window does with an estimated crossing |
| Disease risk | `fetch_daily_disease_inputs`, `ForecastPoint`s | Which days are usable, the dawn and afternoon hour sets, the forecast-day series, every model, how approximate inputs are disclosed |
| Agronomic rules | `EnvironmentalSummary` (incl. `gdd_base50_ytd`), the forecast helpers, `fetch_daily_gdd` behind the GDD endpoint | Rule thresholds, GDD accumulation, what a rule does when soil moisture is absent |
| Seasonal plan | `fetch_daily_soil_temp_averages` | Crossing analysis and its cache |
| Landscape plants | `rain_expected_within`, the 7-day rain total, soil moisture | The watering hold-back rule |
| MCP server | `/soil-temp-forecast` as `soil_temp_outlook`; `fetch_daily_disease_inputs` and `fetch_climate_record` in `weather_history`; connection status in `lawn_snapshot` | Tool shapes and trimming |
| Applications | The lawn's location (latitude, longitude and timezone), which the record resolves stations and the grid point from | The lawn profile that holds it |
| Setup and sources | The station catalog with distances, the basis plan, `status()`, and the writes that pin a station or map a sensor | The setup flow, the Sources page, how the location is entered |
| Web UI | The views' pages and widgets | Shell, routing, API client, shared styles and the status-palette conventions |
| Deployment | The settings each source client reads | The configuration surface, the image and the example Compose stack |

## Key Design Decisions

Decisions specific to one child live in that child's Decisions table.

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Where weather is kept | PostgreSQL tables written by the app | Parquet files maintained outside the app and read in place; parquet written by the app and queried with embedded DuckDB | HLD: an installation is the app and its database. The record mixes hourly station rows, a daily grid, sensor readings and a modeled series, each with a source tag; a relational table holds that directly. |
| Consumer contracts | Kept in shape; each gains a basis | New contracts per basis; consumers choosing their own source | Five components already degrade correctly on these shapes. Resolving the basis in one place is the only way every surface shows the same one (HLD tenet: one answer everywhere). |
| Grain of basis resolution | Per input and per day | Per installation; per input for the whole record | Probes fail for days to months while the rest of a station reports; a sensor has no past. A coarser grain throws away good measurements or invents a history. |
| Station networks | USCRN, then SCAN; no other network | Any state mesonet; airport observations | USCRN and SCAN are keyless, national and documented, and both measure soil. State mesonets are a patchwork of terms and formats, several forbidding automated use. |
| Station cut-off | Per input: 50 miles for soil, 10 miles for hourly air and humidity, none for daily air and rain | One cut-off of 50 miles for every input; the nearest station at any distance | Across USCRN pairs a neighbour's soil dates hold to about 60 miles. Its air does not: from 5 to 60 miles a neighbour's last spring freeze misses by 15 to 19 days at the 80th percentile, PRISM at the point by 4 to 9. The 10-mile figure for hourly air rests on 8 stations and is the least certain of the three. |
| Rain | PRISM daily precipitation at the point, with the hourly analysis for the newest day | The hourly analysis alone; a station's gauge within a cut-off | Against 133 gauges over two years PRISM's 7-day total was off by 4.5 mm and caught 88 % of weeks of 25 mm or more (15 % false alarms); the hourly analysis 8.2 mm, 75 % and 27 %. A station gauge by distance was not scored. |
| What the estimate rests on | PRISM daily air from RCC-ACIS, with the NRCC interpolated grid as its fallback | Reanalysis air and soil (Open-Meteo archive, ERA5-Land); the NRCC grid first | Scored at 133 USCRN stations: PRISM dates first fall freeze 1 \| 9 days (median \| 80th percentile), the NRCC grid 2 \| 11 to 4 \| 19, reanalysis 8 \| 30. PRISM's recent days score as well as its settled ones from four days old. |
| Hourly air and humidity without a station | An hourly gridded analysis (Open-Meteo), labeled approximate | The nearest airport's observations (IEM); none, with disease risk withheld | Against measured stations both were off by about 3 humid hours a day, and airports miss in both directions with no way to know which. One keyless request covers any point. Withholding would leave half the country without disease risk. |
| The owner's sensors | One "owner sensor" source with two ways in: Home Assistant entities read on a schedule, and readings posted to TurfOps by anything else. Each sensor is mapped to one input | Home Assistant only; a plug-in per sensor brand | The product stands alone (HLD tenet), so a sensor must not require Home Assistant; a posted reading with an input name is the smallest thing every hub and script can produce. |
| A sensor's missing past | Comes from the next basis, undisclosed offset and all, with the change of basis marked | Back-filling the sensor's history from the estimate with a fitted offset | A fitted offset needs a season of overlap to mean anything; marking the seam is honest from the first day. |
| Clock for "today" and day buckets | The lawn's local date (the app's `Clock`, in the location's timezone) for every date gate, year boundary and day bucket; daily-only sources keep their own day | The UTC date; each station's local date; the forecast provider's offset | HLD decision: every agronomic date is a lawn-local calendar day, and one configured clock gives every component the same day. The UTC date rolls over hours early every evening. |
| Precipitation units | Millimetres in every backend contract; only the UI converts to inches (`frontend/src/utils/units.ts`) | Inches in the API | One unit across the sources, the forecast, the models and the rules. |
| Shape of this node | A sub-HLD over five leaves | One leaf; sources and record as one leaf; the estimate inside the outlook | Sources change when a provider does; the record changes when a contract or the schema does; the estimate is a model with published accuracy to defend; the outlook is a forecast model with its own thresholds; the views are UI. All five share the one-record, basis-as-data and lag-as-data intent held here. |

## Open Questions

1. **Whether a station's gauge beats PRISM at short range.** Rain from a station a few miles
   away was not scored against PRISM at the lawn's point.
2. **Whether a sensor and the estimate should be reconciled** once a season of overlap exists.
3. **RCC-ACIS publishes no terms of use or rate limits.** The grid client must be conservative
   and the fallback grid is served by the same host.

## References

- `docs/high-level-design.md`: tenets, the "Weather data access" component, the storage, basis
  and "today" decisions.
- [docs/plans: standalone-weather-sources](../../plans/standalone-weather-sources.md): the
  source survey, station coverage and every accuracy figure quoted here.
- `docs/agronomy-methods.md`: data and staleness rules, thresholds (served over MCP).
- `docs/timing-windows.md`: the 5 cm outlook built on this node's regression.
- [docs/plans: uscrn-soil-dropout-investigation](../../plans/uscrn-soil-dropout-investigation.md):
  how USCRN soil probes fail.
- Sibling designs: `docs/intent/data-sync/`, `timing-windows/`, `disease-risk/`,
  `agronomic-rules/`, `seasonal-plan/`, `landscape-plants/`, `applications/`, `deployment/`,
  `mcp-server/`, `web-ui/`.
- NOAA USCRN `hourly02` product; USDA NRCS AWDB REST API (SCAN); RCC-ACIS GridData; Open-Meteo.
