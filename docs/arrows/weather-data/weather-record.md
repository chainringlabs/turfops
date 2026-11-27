# Arrow: weather-record

Weather Record — leaf segment `WX-REC` under the `weather-data` sub-HLD.

## Status

**OK** — audited 2026-11-27 (git SHA `24a3baf`): all 61 active specs implemented; 59 cited by a test, 61 cited in code.

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Weather data access"
- [docs/intent/weather-data/weather-data-design.md](../../intent/weather-data/weather-data-design.md) (parent sub-HLD)

### LLD
- [docs/intent/weather-data/weather-record/weather-record-design.md](../../intent/weather-data/weather-record/weather-record-design.md)

### EARS
- [docs/intent/weather-data/weather-record/weather-record-specs.md](../../intent/weather-data/weather-record/weather-record-specs.md) (61 specs, prefix `WX-REC`)

### Related docs
- [docs/plans/standalone-weather-sources.md](../../plans/standalone-weather-sources.md)
- [docs/plans/uscrn-soil-dropout-investigation.md](../../plans/uscrn-soil-dropout-investigation.md)

### Tests
- [backend/src/api/sources/elevation_db_tests.rs](../../../backend/src/api/sources/elevation_db_tests.rs)
- [backend/src/db/setup_db_tests.rs](../../../backend/src/db/setup_db_tests.rs)
- [backend/src/db/weather_queries/days_db_tests.rs](../../../backend/src/db/weather_queries/days_db_tests.rs)
- [backend/src/db/weather_queries/db_tests.rs](../../../backend/src/db/weather_queries/db_tests.rs)
- [backend/src/db/weather_queries/fetch_db_tests.rs](../../../backend/src/db/weather_queries/fetch_db_tests.rs)
- [backend/src/db/weather_queries/plan_db_tests.rs](../../../backend/src/db/weather_queries/plan_db_tests.rs)
- [backend/src/db/weather_queries/rows_db_tests.rs](../../../backend/src/db/weather_queries/rows_db_tests.rs)
- [backend/src/logic/soil_estimate/estimator_db_tests.rs](../../../backend/src/logic/soil_estimate/estimator_db_tests.rs)
- [backend/src/logic/weather_record/anchored_db_tests.rs](../../../backend/src/logic/weather_record/anchored_db_tests.rs)
- [backend/src/logic/weather_record/daily_db_tests.rs](../../../backend/src/logic/weather_record/daily_db_tests.rs)
- [backend/src/logic/weather_record/disease_db_tests.rs](../../../backend/src/logic/weather_record/disease_db_tests.rs)
- [backend/src/logic/weather_record/due_tests.rs](../../../backend/src/logic/weather_record/due_tests.rs)
- [backend/src/logic/weather_record/fill_db_tests.rs](../../../backend/src/logic/weather_record/fill_db_tests.rs)
- [backend/src/logic/weather_record/history_db_tests.rs](../../../backend/src/logic/weather_record/history_db_tests.rs)
- [backend/src/logic/weather_record/hourly_db_tests.rs](../../../backend/src/logic/weather_record/hourly_db_tests.rs)
- [backend/src/logic/weather_record/live_db_tests.rs](../../../backend/src/logic/weather_record/live_db_tests.rs)
- [backend/src/logic/weather_record/plan_preview_tests.rs](../../../backend/src/logic/weather_record/plan_preview_tests.rs)
- [backend/src/logic/weather_record/plan_tests.rs](../../../backend/src/logic/weather_record/plan_tests.rs)
- [backend/src/logic/weather_record/resolve_tests.rs](../../../backend/src/logic/weather_record/resolve_tests.rs)
- [backend/src/logic/weather_record/summary_tests.rs](../../../backend/src/logic/weather_record/summary_tests.rs)
- [backend/src/logic/weather_sync/fetcher_db_tests.rs](../../../backend/src/logic/weather_sync/fetcher_db_tests.rs)
- [backend/src/logic/weather_sync/schedule_db_tests.rs](../../../backend/src/logic/weather_sync/schedule_db_tests.rs)
- [backend/src/logic/weather_sync/sensor_read_db_tests.rs](../../../backend/src/logic/weather_sync/sensor_read_db_tests.rs)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/geo.rs`

### Code
- [backend/src/api/sources.rs](../../../backend/src/api/sources.rs)
- [backend/src/db/weather_queries/catalog.rs](../../../backend/src/db/weather_queries/catalog.rs)
- [backend/src/db/weather_queries/days.rs](../../../backend/src/db/weather_queries/days.rs)
- [backend/src/db/weather_queries/disease_days.rs](../../../backend/src/db/weather_queries/disease_days.rs)
- [backend/src/db/weather_queries/fetch_state.rs](../../../backend/src/db/weather_queries/fetch_state.rs)
- [backend/src/db/weather_queries/plan.rs](../../../backend/src/db/weather_queries/plan.rs)
- [backend/src/db/weather_queries/rows.rs](../../../backend/src/db/weather_queries/rows.rs)
- [backend/src/db/weather_queries/sensors.rs](../../../backend/src/db/weather_queries/sensors.rs)
- [backend/src/db/weather_queries/sources.rs](../../../backend/src/db/weather_queries/sources.rs)
- [backend/src/geo.rs](../../../backend/src/geo.rs)
- [backend/src/logic/weather_record/daily.rs](../../../backend/src/logic/weather_record/daily.rs)
- [backend/src/logic/weather_record/disease.rs](../../../backend/src/logic/weather_record/disease.rs)
- [backend/src/logic/weather_record/due.rs](../../../backend/src/logic/weather_record/due.rs)
- [backend/src/logic/weather_record/fill.rs](../../../backend/src/logic/weather_record/fill.rs)
- [backend/src/logic/weather_record/hourly.rs](../../../backend/src/logic/weather_record/hourly.rs)
- [backend/src/logic/weather_record/live.rs](../../../backend/src/logic/weather_record/live.rs)
- [backend/src/logic/weather_record/plan.rs](../../../backend/src/logic/weather_record/plan.rs)
- [backend/src/logic/weather_record/record.rs](../../../backend/src/logic/weather_record/record.rs)
- [backend/src/logic/weather_record/resolve.rs](../../../backend/src/logic/weather_record/resolve.rs)
- [backend/src/logic/weather_record/summary.rs](../../../backend/src/logic/weather_record/summary.rs)
- [backend/src/logic/weather_sync/fetcher.rs](../../../backend/src/logic/weather_sync/fetcher.rs)
- [backend/src/logic/weather_sync/planning.rs](../../../backend/src/logic/weather_sync/planning.rs)
- [backend/src/models/weather_basis.rs](../../../backend/src/models/weather_basis.rs)
- [backend/src/models/weather_record.rs](../../../backend/src/models/weather_record.rs)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `WX-REC-STORE` — What is stored and how it is written
2. `WX-REC-PLAN` — Station catalog and basis plan
3. `WX-REC-RES` — Resolving a value
4. `WX-REC-BASIS` — Basis in the contracts
5. `WX-REC-API` — Query contracts
6. `WX-REC-FILL` — Backfill, what is due, status
7. `WX-REC-FAIL` — Outages

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| STORE | WX-REC-STORE-001 – WX-REC-STORE-008 | 8 | 0 | 0 | 8 | 8 |
| PLAN | WX-REC-PLAN-001 – WX-REC-PLAN-013 | 13 | 0 | 0 | 13 | 13 |
| RES | WX-REC-RES-001 – WX-REC-RES-010 | 10 | 0 | 0 | 10 | 9 |
| BASIS | WX-REC-BASIS-001 – WX-REC-BASIS-005 | 5 | 0 | 0 | 5 | 5 |
| API | WX-REC-API-001 – WX-REC-API-012 | 12 | 0 | 0 | 12 | 11 |
| FILL | WX-REC-FILL-001 – WX-REC-FILL-008 | 8 | 0 | 0 | 8 | 8 |
| FAIL | WX-REC-FAIL-001 – WX-REC-FAIL-005 | 5 | 0 | 0 | 5 | 5 |
| **Total** | | **61** | **0** | **0** | **61** | **59** |

**Summary:** 61 of 61 active specs implemented; 0 deferred; 59 implemented specs cited by a test, 2 not.

## Key Findings

1. **Implemented, no test cites it:** WX-REC-RES-009, WX-REC-API-001.

## Work Required

### Must Fix
- None.

### Should Fix
- Add a test citing WX-REC-RES-009, WX-REC-API-001.

### Nice to Have
- None.
