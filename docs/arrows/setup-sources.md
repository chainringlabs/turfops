# Arrow: setup-sources

Setup and Sources — leaf segment `SETUP` under the root HLD.

## Status

**PARTIAL** — audited 2026-11-27 (git SHA `aa1cae8`): 70 of 74 active specs implemented; 4 marked `[ ]` (2 of them already cited in code).

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Setup and sources"

### LLD
- [docs/intent/setup-sources/setup-sources-design.md](../intent/setup-sources/setup-sources-design.md)

### EARS
- [docs/intent/setup-sources/setup-sources-specs.md](../intent/setup-sources/setup-sources-specs.md) (74 specs, prefix `SETUP`)

### Tests
- [backend/src/api/sensor_readings/db_tests.rs](../../backend/src/api/sensor_readings/db_tests.rs)
- [backend/src/api/setup/db_tests.rs](../../backend/src/api/setup/db_tests.rs)
- [backend/src/api/sources/db_tests.rs](../../backend/src/api/sources/db_tests.rs)
- [backend/src/api/sources/elevation_db_tests.rs](../../backend/src/api/sources/elevation_db_tests.rs)
- [backend/src/api/sources/home_assistant_db_tests.rs](../../backend/src/api/sources/home_assistant_db_tests.rs)
- [backend/src/api/sources/read_db_tests.rs](../../backend/src/api/sources/read_db_tests.rs)
- [backend/src/api/sources/sensor_check_db_tests.rs](../../backend/src/api/sources/sensor_check_db_tests.rs)
- [backend/src/api/sources/sensors_db_tests.rs](../../backend/src/api/sources/sensors_db_tests.rs)
- [backend/src/api/sources/write_db_tests.rs](../../backend/src/api/sources/write_db_tests.rs)
- [backend/src/config/load_tests.rs](../../backend/src/config/load_tests.rs)
- [backend/src/datasources/homeassistant/connection_tests.rs](../../backend/src/datasources/homeassistant/connection_tests.rs)
- [backend/src/db/setup_db_tests.rs](../../backend/src/db/setup_db_tests.rs)
- [backend/src/logic/soil_estimate/scores_tests.rs](../../backend/src/logic/soil_estimate/scores_tests.rs)
- [frontend/src/components/SetupNotice.test.tsx](../../frontend/src/components/SetupNotice.test.tsx)
- [frontend/src/components/setup/FoundStep.test.tsx](../../frontend/src/components/setup/FoundStep.test.tsx)
- [frontend/src/components/setup/HistoryStep.test.tsx](../../frontend/src/components/setup/HistoryStep.test.tsx)
- [frontend/src/components/setup/LawnStep.test.tsx](../../frontend/src/components/setup/LawnStep.test.tsx)
- [frontend/src/components/setup/LocationStep.test.tsx](../../frontend/src/components/setup/LocationStep.test.tsx)
- [frontend/src/components/setup/display.test.ts](../../frontend/src/components/setup/display.test.ts)
- [frontend/src/components/setup/sensors.test.ts](../../frontend/src/components/setup/sensors.test.ts)
- [frontend/src/components/setup/sensors/EntityList.test.tsx](../../frontend/src/components/setup/sensors/EntityList.test.tsx)
- [frontend/src/components/setup/sensors/HaConnection.test.tsx](../../frontend/src/components/setup/sensors/HaConnection.test.tsx)
- [frontend/src/components/setup/sensors/PostedAndMapped.test.tsx](../../frontend/src/components/setup/sensors/PostedAndMapped.test.tsx)
- [frontend/src/components/sources/display.test.ts](../../frontend/src/components/sources/display.test.ts)
- [frontend/src/pages/Dashboard.test.tsx](../../frontend/src/pages/Dashboard.test.tsx)
- [frontend/src/pages/DiseaseRisk.test.tsx](../../frontend/src/pages/DiseaseRisk.test.tsx)
- [frontend/src/pages/Environmental.test.tsx](../../frontend/src/pages/Environmental.test.tsx)
- [frontend/src/pages/Setup.test.tsx](../../frontend/src/pages/Setup.test.tsx)
- [frontend/src/pages/Sources.test.tsx](../../frontend/src/pages/Sources.test.tsx)
- [frontend/src/pages/Timing.test.tsx](../../frontend/src/pages/Timing.test.tsx)

### Code
- [backend/src/api/setup.rs](../../backend/src/api/setup.rs)
- [backend/src/api/sources.rs](../../backend/src/api/sources.rs)
- [backend/src/api/sources/home_assistant.rs](../../backend/src/api/sources/home_assistant.rs)
- [backend/src/api/sources/read.rs](../../backend/src/api/sources/read.rs)
- [backend/src/api/sources/sensor_check.rs](../../backend/src/api/sources/sensor_check.rs)
- [backend/src/api/sources/sensors.rs](../../backend/src/api/sources/sensors.rs)
- [backend/src/api/sources/view.rs](../../backend/src/api/sources/view.rs)
- [backend/src/api/sources/write.rs](../../backend/src/api/sources/write.rs)
- [backend/src/config/load.rs](../../backend/src/config/load.rs)
- [backend/src/datasources/homeassistant/connection.rs](../../backend/src/datasources/homeassistant/connection.rs)
- [backend/src/db/setup_queries.rs](../../backend/src/db/setup_queries.rs)
- [backend/src/db/setup_queries/apply.rs](../../backend/src/db/setup_queries/apply.rs)
- [backend/src/db/setup_queries/credentials.rs](../../backend/src/db/setup_queries/credentials.rs)
- [backend/src/error.rs](../../backend/src/error.rs)
- [backend/src/logic/soil_estimate/scores.rs](../../backend/src/logic/soil_estimate/scores.rs)
- [backend/src/logic/weather_sync/fetcher.rs](../../backend/src/logic/weather_sync/fetcher.rs)
- [backend/src/logic/weather_sync/posted.rs](../../backend/src/logic/weather_sync/posted.rs)
- [backend/src/main.rs](../../backend/src/main.rs)
- [frontend/src/api/client.ts](../../frontend/src/api/client.ts)
- [frontend/src/components/SetupNotice.tsx](../../frontend/src/components/SetupNotice.tsx)
- [frontend/src/components/profile/LawnProfileForm.tsx](../../frontend/src/components/profile/LawnProfileForm.tsx)
- [frontend/src/components/setup/FoundStep.tsx](../../frontend/src/components/setup/FoundStep.tsx)
- [frontend/src/components/setup/HistoryStep.tsx](../../frontend/src/components/setup/HistoryStep.tsx)
- [frontend/src/components/setup/LawnStep.tsx](../../frontend/src/components/setup/LawnStep.tsx)
- [frontend/src/components/setup/LocationStep.tsx](../../frontend/src/components/setup/LocationStep.tsx)
- [frontend/src/components/setup/SensorsStep.tsx](../../frontend/src/components/setup/SensorsStep.tsx)
- [frontend/src/components/setup/display.ts](../../frontend/src/components/setup/display.ts)
- [frontend/src/components/setup/sensors.ts](../../frontend/src/components/setup/sensors.ts)
- [frontend/src/components/setup/sensors/EntityList.tsx](../../frontend/src/components/setup/sensors/EntityList.tsx)
- [frontend/src/components/setup/sensors/HaConnection.tsx](../../frontend/src/components/setup/sensors/HaConnection.tsx)
- [frontend/src/components/setup/sensors/MapForm.tsx](../../frontend/src/components/setup/sensors/MapForm.tsx)
- [frontend/src/components/setup/sensors/MappedSensors.tsx](../../frontend/src/components/setup/sensors/MappedSensors.tsx)
- [frontend/src/components/setup/sensors/PostedSensorForm.tsx](../../frontend/src/components/setup/sensors/PostedSensorForm.tsx)
- [frontend/src/components/sources/InputCard.tsx](../../frontend/src/components/sources/InputCard.tsx)
- [frontend/src/components/sources/SourcesTable.tsx](../../frontend/src/components/sources/SourcesTable.tsx)
- [frontend/src/components/sources/display.ts](../../frontend/src/components/sources/display.ts)
- [frontend/src/pages/DiseaseRisk.tsx](../../frontend/src/pages/DiseaseRisk.tsx)
- [frontend/src/pages/Environmental.tsx](../../frontend/src/pages/Environmental.tsx)
- [frontend/src/pages/Setup.tsx](../../frontend/src/pages/Setup.tsx)
- [frontend/src/pages/Sources.tsx](../../frontend/src/pages/Sources.tsx)
- [frontend/src/pages/Timing.tsx](../../frontend/src/pages/Timing.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `SETUP-FLOW` — The setup flow
2. `SETUP-LOC` — The location step
3. `SETUP-PLAN` — What we found, pins and the forecast provider
4. `SETUP-HA` — Connecting Home Assistant
5. `SETUP-MAP` — Mapping sensors
6. `SETUP-PAGE` — The Sources page
7. `SETUP-API` — The endpoints
8. `SETUP-CRED` — Stored credentials
9. `SETUP-FAIL` — Failures

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| FLOW | SETUP-FLOW-001 – SETUP-FLOW-008 | 8 | 0 | 0 | 8 | 8 |
| LOC | SETUP-LOC-001 – SETUP-LOC-006 | 6 | 0 | 0 | 6 | 6 |
| PLAN | SETUP-PLAN-001 – SETUP-PLAN-006 | 6 | 0 | 0 | 6 | 6 |
| HA | SETUP-HA-001 – SETUP-HA-005 | 5 | 0 | 0 | 5 | 5 |
| MAP | SETUP-MAP-001 – SETUP-MAP-014 | 14 | 0 | 0 | 14 | 14 |
| PAGE | SETUP-PAGE-001 – SETUP-PAGE-013 | 13 | 0 | 0 | 13 | 13 |
| API | SETUP-API-001 – SETUP-API-015 | 14 | 0 | 0 | 14 | 14 |
| CRED | SETUP-CRED-001 – SETUP-CRED-004 | 3 | 0 | 1 | 3 | 3 |
| FAIL | SETUP-FAIL-001 – SETUP-FAIL-004 | 1 | 0 | 3 | 3 | 3 |
| **Total** | | **70** | **0** | **4** | **72** | **72** |

**Summary:** 70 of 74 active specs implemented; 0 deferred; 70 implemented specs cited by a test.

## Key Findings

1. **Marked `[ ]` but cited in code:** SETUP-FAIL-001, SETUP-FAIL-004. Either the code is done and the marker is stale, or the annotation claims more than the code does; check each.
2. **Open gaps (not yet built):** SETUP-CRED-004, SETUP-FAIL-003.

## Work Required

### Must Fix
- None.

### Should Fix
- Confirm SETUP-FAIL-001, SETUP-FAIL-004 against the code and set their markers.
- Build or defer SETUP-CRED-004, SETUP-FAIL-003.

### Nice to Have
- None.
