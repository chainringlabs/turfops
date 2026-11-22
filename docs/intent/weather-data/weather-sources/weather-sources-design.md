---
parent: weather-data
prefix: WX-SRC
---

# Weather Sources

## Context and Design Philosophy

This leaf owns the clients that bring conditions into TurfOps: one per outside source. A client
knows one source's transport, format, units, sentinels, quality flags, publishing rhythm and
terms of use, and nothing about the lawn. It answers "what does this source say for this
station or point over this span" in the weather record's units. Data sync decides when a
client is called; the record stores what it returns and decides which source speaks.

The forecast is the one thing not stored in the record: this leaf also owns `WeatherForecast`
and its day-level helpers.

Principles:

- **Every required source is free and keyless** (HLD). A source that needs a key is optional
  and off without one.
- **Normalize at the door.** A client hands over °F, mm, percent and fractions, UTC instants,
  and no sentinels. Nothing downstream interprets a source's native format.
- **A polite guest.** Each client asks for the least the source can serve, no more often than
  the source publishes, identifies the application by name, and sends nothing about the owner
  beyond the coordinates a point query requires.
- **A source that fails says so and writes nothing.** A partial or unparseable response is an
  outage, never a short success.

## Sources

| Source | Client supplies | Transport | Grain at the source | Required |
|---|---|---|---|---|
| NOAA USCRN | Station catalog; hourly air, humidity, rain, soil temperature and moisture at 5 / 10 / 20 / 50 / 100 cm; daily soil and air for calibration stations | Keyless files over HTTPS: one `hourly02` and one `daily01` file per station and year, and a station list | Hourly and daily; °C, mm, % RH, fractional moisture | Yes |
| USDA SCAN | Station catalog; the same hourly inputs where a station has the sensors | Keyless AWDB REST API | Hourly; units named per element | Yes |
| PRISM daily grid | Daily air maximum, minimum and precipitation at a point | Keyless RCC-ACIS `GridData`, grid 21 | Daily, about 4 km, through yesterday; °F and inches | Yes |
| NRCC interpolated grid | The same, as the fallback for days PRISM lacks | RCC-ACIS `GridData`, grid 1 | Daily, through today | Yes |
| Open-Meteo | The hourly analysis (air, humidity, rain for past hours); the default forecast; the point's elevation; place search for setup | Keyless JSON APIs | Hourly; 93 past days and 16 forecast days | Yes |
| Owner sensors: Home Assistant | The mapped entities' states, and the list of entities for setup | `GET {HA_URL}/api/states` with a bearer token | Instantaneous | No |
| Owner sensors: posted readings | Readings sent to TurfOps by any hub or script | `POST /api/v1/sensor-readings` with a bearer token | Instantaneous | No |
| National Weather Service | An alternative forecast | Keyless `api.weather.gov` | Hourly, about 6.5 days; rain amounts about 3 days | No |
| OpenWeatherMap | An alternative forecast | `GET /data/2.5/forecast` with an API key | 3-hourly, 5 days | No |

## What Every Client Does

### Units, sentinels and plausibility

| Quantity | Stored as | Dropped when |
|---|---|---|
| Air and soil temperature | °F | The source's missing marker (USCRN −9999, −99), or outside −80 to 150 °F |
| Relative humidity | Percent | Missing marker, or outside 0 to 100 |
| Precipitation | mm | Missing marker, negative, or more than 300 mm in an hour |
| Soil moisture | Fraction 0 to 1 | Missing marker, or outside 0 to 1 |

A dropped value is `None` in its row; the rest of the row stands. A value the source itself
flags as failed quality control is dropped the same way.

### Time

Every instant is UTC. An hourly row is stamped with the start of the hour it describes. A
source that stamps the end of the hour (USCRN) or reports in a station's local standard time
(SCAN) is converted by its client. A daily row keeps the source's own day; the record maps it to the lawn's days.

### Requests

- **Identification.** Every request carries a `User-Agent` naming TurfOps and its version. It
  carries no owner name, address or contact detail.
- **Timeouts.** 5 s to connect and 30 s per request, 120 s for a multi-year grid request.
- **One request at a time per host**, with at least 1 s between requests to the same host and
  2 s for RCC-ACIS, which publishes no limits.
- **Conditional fetches.** A file already held is re-requested with `If-Modified-Since`; "not
  modified" is a success that writes nothing.
- **No retry inside a client.** A failure is returned; data sync owns when to try again.

### Cadence

Each client states how often its source has something new. Data sync schedules from this.

| Product | New data | Revisions to re-read |
|---|---|---|
| USCRN hourly, current year | Hourly | The last 3 days |
| USCRN hourly or daily, past years | Never; fetched once | None |
| USCRN daily for calibration stations, current year | Daily | The last 7 days |
| SCAN hourly | Hourly | The last 3 days |
| PRISM daily | Once a day | The last 7 days |
| NRCC daily | Once a day, only for days PRISM lacks | None |
| Hourly analysis | Hourly | The last 2 days |
| Forecast | Hourly | Replaced whole |
| Home Assistant entities | Every 5 minutes | None |
| Station catalogs | Monthly | Replaced whole |

### Errors

A transport failure, a non-success status, a body that does not parse, or a response missing
the span asked for is an outage of that source (`unavailable(Source::…, detail)`). The owner
sentence names the source and what depends on it; the detail (URL, status, body, parse error)
goes to the log only. A URL that carries a key is stripped from every error before it is
formatted, inside the client.

The Home Assistant and forecast clients expose a probe that makes their smallest real request,
so setup can test a connection the owner has just entered. Routine source status comes from
the outcome of scheduled fetches (data sync).

## The Clients

### USCRN

- **Catalog:** the network's station list (`stations.tsv`): WBAN number, state, location and
  name, latitude, longitude, elevation in feet, status, operation and network. The list also
  carries other networks; only rows whose network is USCRN, in the contiguous United States,
  are kept. Which depths a station reports is learned from its data.
- **Hourly (`hourly02`):** air temperature (the hour's average, with the hour's maximum and
  minimum), precipitation, relative humidity (dropped when its flag is not 0), and soil
  temperature and moisture at the five depths. The file's UTC time is the end of the observed
  hour, so each row is stamped one hour earlier.
- **Lag:** the network publishes hourly but can run two days behind; the yearly file and the
  network's hourly update feed end at the same hour.
- **Daily (`daily01`):** air maximum, minimum and mean, precipitation, and soil temperature at
  5 and 10 cm. Used for calibration stations only.
- **Span:** one station-year per request. A year the station did not operate is a 404, which
  is an empty success, not an outage.

### SCAN

- **Catalog:** the AWDB station list filtered to the SCAN network in the contiguous United
  States, with elevation in feet, the station's fixed UTC offset, its first date of record,
  and the elements each station reports with their depths and units.
- **Hourly:** air temperature, relative humidity, precipitation increment, and soil
  temperature and moisture at the sensor depths nearest 5, 10, 20, 50 and 100 cm (SCAN's
  standard depths are 2, 4, 8, 20 and 40 inches, which are those five). Units are read from
  the response and converted: temperatures arrive in °F, soil moisture and humidity in
  percent, precipitation in inches.
- **Span:** up to one station-year per request.

### PRISM and NRCC (RCC-ACIS)

- One request returns a point's daily maximum, minimum and precipitation for any span, so a
  15-year backfill of one point is one request.
- Temperatures arrive in °F; precipitation in inches, converted to mm. The missing marker
  (−999) drops the value.
- Asked for it, the response reports the grid cell's own latitude and longitude; the client
  returns them so the record can show where the value is for. It does not report the cell's
  elevation.
- The two grids are one client with a grid parameter. NRCC is requested only for days PRISM
  returned nothing.

### Open-Meteo

- **Hourly analysis:** past hours' air temperature, relative humidity and precipitation at the
  point, from the forecast API's past days (up to 92) at first start and hourly thereafter.
  Soil variables are never requested: the reanalysis soil scored worse than the soil estimate.
- **Point facts:** every response carries the model grid's elevation in metres, which the
  client returns in feet, the unit the station catalogs use.
- **Place search:** a name or postal code to a list of candidate places with coordinates, for
  setup. The typed text is sent to Open-Meteo and nothing else is.
- **Forecast:** see "Forecast" below.
- **Terms:** free for non-commercial use, data under CC BY 4.0, a daily request allowance the
  schedule above stays far inside. Attribution is shown wherever its values are.

### Owner sensors

One kind of source, two ways in. Either way a sensor is a named thing mapped to exactly one
input: air temperature, humidity, rain, or soil temperature or moisture at one depth. The
mapping is made in the app and stored there.

- **Home Assistant.** The client lists entities with their units and device classes so setup
  can offer them, and reads the mapped ones on schedule. A reading is the entity's state, its
  unit of measurement, and the time it was last reported: `last_reported`, which moves each
  time the device reports even when the value is unchanged, or `last_updated` where a Home
  Assistant version does not provide it. A state that is not a number
  (`unavailable`, `unknown`) is no reading. A reading whose reported time is unchanged since
  the previous read is not stored again.
- **Posted readings.** `POST /api/v1/sensor-readings` accepts one or more readings: sensor
  name, value, unit, and optionally the time observed (default: now). The route exists only
  when `SENSOR_TOKEN` is set and requires it as a bearer token. A reading for a sensor that is
  not mapped, in a unit the input does not accept, outside the plausibility range, or dated
  more than 5 minutes in the future is rejected with a message naming the problem and the rest
  of the batch is stored. The answer is 200, listing what was stored and what was rejected,
  when anything was stored, and 400 when nothing was, so a sender that retries on an error
  never re-sends readings that were accepted.
- **Units** are converted from what the sensor declares: °C or °F; mm or inches; percent or
  fraction for soil moisture.
- **Rain** is mapped as one of two kinds. A *counter* reports a running total; the amount is
  the increase since the previous reading, and a decrease is a reset that counts as zero. An
  *amount* reports rain since its last reading and is used as is.

### Forecast

`WeatherForecast` (`backend/src/models/forecast.rs`) is what every provider returns:

- `hourly: Vec<ForecastPoint>`: one point per provider slot with the slot's length in hours,
  temperature, humidity, precipitation in mm for the slot, probability of precipitation, wind,
  gust and a condition. A quantity the provider does not forecast for a slot is `None`, never
  zero: NWS rain amounts end about three days out.
- `daily_summary: Vec<DailyForecast>`: points grouped by lawn-local date through the app's
  `Clock`, each with high, low, mean humidity, total precipitation, maximum probability, most
  frequent condition, mean wind, maximum gust and `hours_covered`, the sum of its slots'
  lengths.

| Provider | Slot | Horizon | Needs |
|---|---|---|---|
| Open-Meteo (default) | 1 h | 16 days | Nothing |
| National Weather Service | 1 h | About 6.5 days | Nothing |
| OpenWeatherMap | 3 h | 5 days | `OWM_API_KEY` |

The provider is chosen in the app. A provider chosen but not usable (no key) leaves the
forecast off with the reason shown; it does not fall back to another provider silently.

Day-level helpers, a contract consumed by rules, timing windows, the soil-temperature outlook
and landscape plants:

| Helper | Semantics |
|---|---|
| `DailyForecast::is_full_day` | `hours_covered >= 18` |
| `full_days()` | Only full days. Anything that needs a daily mean or judges a whole day must use this, because a forecast starts and ends mid-day |
| `next_hours(n)` | Points with `timestamp <= now + n h`. Points already past are not dropped, so the current slot counts |
| `next_days(today, n)` | Daily summaries with `date <= today + n`, partial days included. `today` is the caller's lawn-local date |
| `rain_expected_within(h, mm)` | `Some` when total precipitation in `next_hours(h)` ≥ `mm` or any slot's probability ≥ 0.5 |
| `max_temp_next_days(today, n)` | Highest daily high in `next_days(today, n)` |

## Settings Each Client Reads

| Setting | From |
|---|---|
| Home Assistant address and token | The app's stored connection (setup and sources) |
| OpenWeatherMap key | The app's stored key |
| Token for posted readings | `SENSOR_TOKEN`; the route is not mounted without it |
| Forecast provider, sensor mappings | The app's stored settings |
| The lawn's location | The lawn profile |

`HA_URL`, `HA_TOKEN` and `OWM_API_KEY` are read once, on a start that finds nothing stored for
them, and copied into the app's stored settings; after that they are ignored. A stored
credential is write-only: no response, log line or error ever carries it. The station and grid clients take
a station id or a point as an argument and read no setting at all.

## Failure and Degradation

| Failure | What the client returns |
|---|---|
| Host unreachable, timeout, 5xx | An outage of that source |
| 404 for a station-year file | Empty success |
| Body parses but has a hole inside the span asked for | An outage; nothing partial is returned. Missing days at the end of a grid span are not yet published, not a hole |
| A value is a sentinel, flagged or implausible | That value `None`, the row kept |
| Home Assistant entity not numeric | No reading for that sensor; the others are read |
| Home Assistant unreachable | An outage of the owner's sensors (Home Assistant) |
| Forecast provider fails | A forecast outage; data sync keeps the previous forecast within its reuse limit |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Shape of a client | One per source, each returning rows in the record's units | One generic "provider" interface with per-source configuration | The sources share almost nothing: yearly fixed-width files, a REST API with per-element units, a grid POST. A common return type is the only abstraction that holds. |
| USCRN access | The per-station yearly files | The network's update feed of all stations per hour | A lawn needs one to six stations. The yearly file serves backfill and updates with one parser, is no staler than the feed, and a conditional request makes an unchanged file free. |
| Station history product | `hourly02` for basis stations, `daily01` for calibration stations | Hourly for all; daily for all | The record's coverage rule needs hours for a basis station. Calibration needs only daily soil, at a fortieth of the download. |
| Hourly analysis provider | Open-Meteo's forecast API past days | Open-Meteo's reanalysis archive; airport observations (IEM) | Against measured stations the forecast API's past hours counted humid hours within 2.9 h a day and agreed 84 % of the time, the archive 3.4 h and 80 %, a nearby airport 3.1 to 4.0 h. One keyless request serves any point. |
| Soil from Open-Meteo | Never requested | Use it where no station is near | Its rising 55 °F crossing scored 4 \| 21 days with a 4-day early bias against 2 \| 13 for the soil estimate. |
| Default forecast | Open-Meteo | National Weather Service; OpenWeatherMap | Keyless, hourly, 16 days, with humidity and rain amounts throughout. NWS is shorter and its rain amounts stop at about 3 days; OpenWeatherMap needs a key (HLD). |
| Forecast slot length | Carried on each point; `hours_covered` is the sum | Assume 3 h | Providers differ, and a day's coverage must mean the same thing for each. |
| A quantity a provider lacks | `None` | Zero | "No rain forecast" and "rain not forecast this far out" lead to different advice. |
| Provider not usable | Forecast off, reason shown | Fall back to the default provider | The owner chose a provider; answering from another without saying so breaks "every answer names its basis". |
| Posted readings | A bearer-guarded route, mounted only with `SENSOR_TOKEN` | Open to the local network; MQTT; a plug-in per hub | The product must not require Home Assistant (HLD tenet: the product stands alone), and an HTTP POST is what every hub and script can make. A write route with no guard would let anything on the network alter the lawn's record. |
| Home Assistant reading time | The entity's `last_reported`, else `last_updated` | The server clock at read time; `last_updated` alone | A stuck sensor otherwise looks current forever. `last_updated` moves only when the value changes, so a rain counter in a dry week or a steady soil probe would look dead. |
| Home Assistant units | Read from the entity | A unit setting per entity | The entity already declares it; a setting can only disagree with it. |
| Rain from a sensor | Declared as counter or amount at mapping | Guess from the readings | The two look alike until a reset or a dry week; a wrong guess doubles or zeroes the rain. |
| Retries | None inside a client | Retry with back-off in each client | One place (data sync) knows the schedule and what else is due; a client that retries hides an outage and multiplies requests on a struggling host. |
| Identification | Application name and version only | Include a contact address, as some services request | The owner's details are private and are not sent to any service (HLD). |
| Key in errors | The URL is stripped inside the client | Rely on callers not to log it | OpenWeatherMap's key travels in the query string; the client is the only place that can guarantee it is never formatted. |
| Full-day cut-off | 18 of 24 hours | 24 hours; any coverage | Allows a small shortfall while excluding evening-only days. |

## Open Questions & Future Decisions

### Deferred

1. **RCC-ACIS publishes no terms or limits** and serves both grids. If it withdraws service,
   daily air and rain have no scored replacement: Open-Meteo's archive dated first fall
   freeze 8 | 30 days.
2. **The newest day's rain** comes from the hourly analysis, which caught 58 % of wet weeks in
   testing against PRISM's 88 %. It is replaced within a day or two.
3. **SCAN was not scored against USCRN.** Its sensors and siting differ; it is used only where
   no USCRN station is within the cut-off.
4. **Open-Meteo's free tier is for non-commercial use.** A hosted or commercial TurfOps would
   need its paid tier or another hourly analysis.
5. **Hobby soil-moisture probes report uncalibrated percentages.** The client converts units;
   it cannot tell a calibrated volumetric reading from a raw one, and the irrigation thresholds
   assume the former.
6. **Posted readings are not rate-limited.**

## References

- Parent: `docs/intent/weather-data/weather-data-design.md`.
- Siblings: `weather-record/` (what is stored and resolved), `soil-estimate/`,
  `environmental-views/` (connection status).
- `docs/intent/data-sync/`: the fetch schedule, retries, forecast reuse.
- `docs/intent/deployment/`: the environment settings and the outage vocabulary.
- [docs/plans: standalone-weather-sources](../../../plans/standalone-weather-sources.md): the
  source survey and every accuracy figure quoted here.
- NOAA USCRN `hourly02` and `daily01` product documentation; USDA NRCS AWDB REST API;
  RCC-ACIS web services; Open-Meteo API documentation; api.weather.gov; OpenWeatherMap
  5-day forecast.
