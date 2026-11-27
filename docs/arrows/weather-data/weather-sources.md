# Arrow: weather-sources

Weather Sources — leaf segment `WX-SRC` under the `weather-data` sub-HLD.

## Status

**PARTIAL** — audited 2026-11-27 (git SHA `24a3baf`): 77 of 80 active specs implemented; 3 marked `[ ]` (3 of them already cited in code).

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Weather data access"
- [docs/intent/weather-data/weather-data-design.md](../../intent/weather-data/weather-data-design.md) (parent sub-HLD)

### LLD
- [docs/intent/weather-data/weather-sources/weather-sources-design.md](../../intent/weather-data/weather-sources/weather-sources-design.md)

### EARS
- [docs/intent/weather-data/weather-sources/weather-sources-specs.md](../../intent/weather-data/weather-sources/weather-sources-specs.md) (80 specs, prefix `WX-SRC`)

### Related docs
- [docs/plans/standalone-weather-sources.md](../../plans/standalone-weather-sources.md)

### Tests
- [backend/src/api/sensor_readings/config_tests.rs](../../../backend/src/api/sensor_readings/config_tests.rs)
- [backend/src/api/sensor_readings/db_tests.rs](../../../backend/src/api/sensor_readings/db_tests.rs)
- [backend/src/api/sources/write_db_tests.rs](../../../backend/src/api/sources/write_db_tests.rs)
- [backend/src/datasources/grid/tests.rs](../../../backend/src/datasources/grid/tests.rs)
- [backend/src/datasources/homeassistant/connection_tests.rs](../../../backend/src/datasources/homeassistant/connection_tests.rs)
- [backend/src/datasources/nws/tests.rs](../../../backend/src/datasources/nws/tests.rs)
- [backend/src/datasources/open_meteo/forecast_tests.rs](../../../backend/src/datasources/open_meteo/forecast_tests.rs)
- [backend/src/datasources/open_meteo/places_tests.rs](../../../backend/src/datasources/open_meteo/places_tests.rs)
- [backend/src/datasources/open_meteo/tests.rs](../../../backend/src/datasources/open_meteo/tests.rs)
- [backend/src/datasources/openweathermap/fetch_tests.rs](../../../backend/src/datasources/openweathermap/fetch_tests.rs)
- [backend/src/datasources/openweathermap/tests.rs](../../../backend/src/datasources/openweathermap/tests.rs)
- [backend/src/datasources/outside/tests.rs](../../../backend/src/datasources/outside/tests.rs)
- [backend/src/datasources/scan/tests.rs](../../../backend/src/datasources/scan/tests.rs)
- [backend/src/datasources/sensor_units_tests.rs](../../../backend/src/datasources/sensor_units_tests.rs)
- [backend/src/datasources/uscrn/parse_tests.rs](../../../backend/src/datasources/uscrn/parse_tests.rs)
- [backend/src/datasources/uscrn/tests.rs](../../../backend/src/datasources/uscrn/tests.rs)
- [backend/src/db/setup_db_tests.rs](../../../backend/src/db/setup_db_tests.rs)
- [backend/src/logic/weather_record/due_tests.rs](../../../backend/src/logic/weather_record/due_tests.rs)
- [backend/src/logic/weather_sync/forecast_db_tests.rs](../../../backend/src/logic/weather_sync/forecast_db_tests.rs)
- [backend/src/logic/weather_sync/forecast_tests.rs](../../../backend/src/logic/weather_sync/forecast_tests.rs)
- [backend/src/logic/weather_sync/posted_tests.rs](../../../backend/src/logic/weather_sync/posted_tests.rs)
- [backend/src/logic/weather_sync/sensor_read_db_tests.rs](../../../backend/src/logic/weather_sync/sensor_read_db_tests.rs)
- [backend/src/logic/weather_sync/sensor_reading_tests.rs](../../../backend/src/logic/weather_sync/sensor_reading_tests.rs)
- [backend/src/models/forecast/daily_tests.rs](../../../backend/src/models/forecast/daily_tests.rs)
- [backend/src/models/forecast/day_tests.rs](../../../backend/src/models/forecast/day_tests.rs)
- [backend/src/models/forecast/tests.rs](../../../backend/src/models/forecast/tests.rs)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/models/environmental.rs`

### Code
- [backend/src/api/sensor_readings.rs](../../../backend/src/api/sensor_readings.rs)
- [backend/src/api/sensor_readings/take.rs](../../../backend/src/api/sensor_readings/take.rs)
- [backend/src/api/sources/write.rs](../../../backend/src/api/sources/write.rs)
- [backend/src/datasources/cadence.rs](../../../backend/src/datasources/cadence.rs)
- [backend/src/datasources/grid.rs](../../../backend/src/datasources/grid.rs)
- [backend/src/datasources/homeassistant/connection.rs](../../../backend/src/datasources/homeassistant/connection.rs)
- [backend/src/datasources/nws.rs](../../../backend/src/datasources/nws.rs)
- [backend/src/datasources/nws/parse.rs](../../../backend/src/datasources/nws/parse.rs)
- [backend/src/datasources/open_meteo.rs](../../../backend/src/datasources/open_meteo.rs)
- [backend/src/datasources/open_meteo/forecast.rs](../../../backend/src/datasources/open_meteo/forecast.rs)
- [backend/src/datasources/open_meteo/places.rs](../../../backend/src/datasources/open_meteo/places.rs)
- [backend/src/datasources/openweathermap.rs](../../../backend/src/datasources/openweathermap.rs)
- [backend/src/datasources/outside.rs](../../../backend/src/datasources/outside.rs)
- [backend/src/datasources/scan.rs](../../../backend/src/datasources/scan.rs)
- [backend/src/datasources/scan/parse.rs](../../../backend/src/datasources/scan/parse.rs)
- [backend/src/datasources/sensor_units.rs](../../../backend/src/datasources/sensor_units.rs)
- [backend/src/datasources/uscrn.rs](../../../backend/src/datasources/uscrn.rs)
- [backend/src/datasources/uscrn/parse.rs](../../../backend/src/datasources/uscrn/parse.rs)
- [backend/src/db/setup_queries.rs](../../../backend/src/db/setup_queries.rs)
- [backend/src/db/setup_queries/credentials.rs](../../../backend/src/db/setup_queries/credentials.rs)
- [backend/src/db/weather_queries/sensors.rs](../../../backend/src/db/weather_queries/sensors.rs)
- [backend/src/error.rs](../../../backend/src/error.rs)
- [backend/src/logic/weather_record/due.rs](../../../backend/src/logic/weather_record/due.rs)
- [backend/src/logic/weather_sync/forecast.rs](../../../backend/src/logic/weather_sync/forecast.rs)
- [backend/src/logic/weather_sync/posted.rs](../../../backend/src/logic/weather_sync/posted.rs)
- [backend/src/logic/weather_sync/sensor_read.rs](../../../backend/src/logic/weather_sync/sensor_read.rs)
- [backend/src/logic/weather_sync/sensor_reading.rs](../../../backend/src/logic/weather_sync/sensor_reading.rs)
- [backend/src/logic/weather_sync/sensor_store.rs](../../../backend/src/logic/weather_sync/sensor_store.rs)
- [backend/src/main.rs](../../../backend/src/main.rs)
- [backend/src/models/environmental.rs](../../../backend/src/models/environmental.rs)
- [backend/src/models/forecast.rs](../../../backend/src/models/forecast.rs)
- [backend/src/models/forecast/daily.rs](../../../backend/src/models/forecast/daily.rs)
- [backend/src/state.rs](../../../backend/src/state.rs)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `WX-SRC-NORM` — Units, sentinels and time
2. `WX-SRC-REQ` — How requests are made
3. `WX-SRC-CAD` — Cadence
4. `WX-SRC-USCRN` — USCRN
5. `WX-SRC-SCAN` — SCAN
6. `WX-SRC-GRID` — PRISM and NRCC grids
7. `WX-SRC-OM` — Open-Meteo
8. `WX-SRC-HA` — Home Assistant
9. `WX-SRC-POST` — Posted sensor readings
10. `WX-SRC-FC` — Forecast providers
11. `WX-SRC-OWM` — The forecast model and its day helpers
12. `WX-SRC-MODEL` — Shared units
13. `WX-SRC-CFG` — Settings
14. `WX-SRC-ERR` — Errors

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| NORM | WX-SRC-NORM-001 – WX-SRC-NORM-004 | 2 | 0 | 2 | 4 | 4 |
| REQ | WX-SRC-REQ-001 – WX-SRC-REQ-006 | 6 | 0 | 0 | 6 | 6 |
| CAD | WX-SRC-CAD-001 – WX-SRC-CAD-002 | 2 | 0 | 0 | 2 | 2 |
| USCRN | WX-SRC-USCRN-001 – WX-SRC-USCRN-006 | 6 | 0 | 0 | 6 | 6 |
| SCAN | WX-SRC-SCAN-001 – WX-SRC-SCAN-003 | 3 | 0 | 0 | 3 | 3 |
| GRID | WX-SRC-GRID-001 – WX-SRC-GRID-003 | 3 | 0 | 0 | 3 | 3 |
| OM | WX-SRC-OM-001 – WX-SRC-OM-004 | 4 | 0 | 0 | 4 | 4 |
| HA | WX-SRC-HA-001 – WX-SRC-HA-014 | 14 | 0 | 0 | 14 | 14 |
| POST | WX-SRC-POST-001 – WX-SRC-POST-015 | 15 | 0 | 0 | 15 | 15 |
| FC | WX-SRC-FC-001 – WX-SRC-FC-010 | 10 | 0 | 0 | 10 | 10 |
| OWM | WX-SRC-OWM-001 – WX-SRC-OWM-006 | 6 | 0 | 0 | 6 | 6 |
| MODEL | WX-SRC-MODEL-003 – WX-SRC-MODEL-004 | 2 | 0 | 0 | 2 | 1 |
| CFG | WX-SRC-CFG-002 – WX-SRC-CFG-004 | 3 | 0 | 0 | 3 | 1 |
| ERR | WX-SRC-ERR-001 – WX-SRC-ERR-002 | 1 | 0 | 1 | 2 | 2 |
| **Total** | | **77** | **0** | **3** | **80** | **77** |

**Summary:** 77 of 80 active specs implemented; 0 deferred; 74 implemented specs cited by a test, 3 not.

## Key Findings

1. **Marked `[ ]` but cited in code:** WX-SRC-NORM-003, WX-SRC-NORM-004, WX-SRC-ERR-001. Either the code is done and the marker is stale, or the annotation claims more than the code does; check each.
2. **Implemented, no test cites it:** WX-SRC-MODEL-003, WX-SRC-CFG-002, WX-SRC-CFG-004.

## Work Required

### Must Fix
- None.

### Should Fix
- Confirm WX-SRC-NORM-003, WX-SRC-NORM-004, WX-SRC-ERR-001 against the code and set their markers.
- Add a test citing WX-SRC-MODEL-003, WX-SRC-CFG-002, WX-SRC-CFG-004.

### Nice to Have
- None.
