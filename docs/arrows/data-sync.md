# Arrow: data-sync

Data Sync — leaf segment `SYNC` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `1b12d9d`): all 50 active specs implemented; 47 cited by a test, 49 cited in code.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Data sync"

### LLD
- [docs/intent/data-sync/data-sync-design.md](../intent/data-sync/data-sync-design.md)

### EARS
- [docs/intent/data-sync/data-sync-specs.md](../intent/data-sync/data-sync-specs.md) (50 specs, prefix `SYNC`)

### Tests
- [backend/src/api/health/db_tests.rs](../../backend/src/api/health/db_tests.rs)
- [backend/src/api/health/tests.rs](../../backend/src/api/health/tests.rs)
- [backend/src/api/profile/db_tests.rs](../../backend/src/api/profile/db_tests.rs)
- [backend/src/api/sources/read_db_tests.rs](../../backend/src/api/sources/read_db_tests.rs)
- [backend/src/api/timing/handler_tests.rs](../../backend/src/api/timing/handler_tests.rs)
- [backend/src/api/timing/memo_tests.rs](../../backend/src/api/timing/memo_tests.rs)
- [backend/src/logic/data_sync/assemble_tests.rs](../../backend/src/logic/data_sync/assemble_tests.rs)
- [backend/src/logic/data_sync/clock_tests.rs](../../backend/src/logic/data_sync/clock_tests.rs)
- [backend/src/logic/data_sync/db_tests.rs](../../backend/src/logic/data_sync/db_tests.rs)
- [backend/src/logic/data_sync/memo_slot_tests.rs](../../backend/src/logic/data_sync/memo_slot_tests.rs)
- [backend/src/logic/data_sync/memo_tests.rs](../../backend/src/logic/data_sync/memo_tests.rs)
- [backend/src/logic/data_sync/record_db_tests.rs](../../backend/src/logic/data_sync/record_db_tests.rs)
- [backend/src/logic/weather_record/due_tests.rs](../../backend/src/logic/weather_record/due_tests.rs)
- [backend/src/logic/weather_record/live_db_tests.rs](../../backend/src/logic/weather_record/live_db_tests.rs)
- [backend/src/logic/weather_sync/fetcher_db_tests.rs](../../backend/src/logic/weather_sync/fetcher_db_tests.rs)
- [backend/src/logic/weather_sync/forecast_db_tests.rs](../../backend/src/logic/weather_sync/forecast_db_tests.rs)
- [backend/src/logic/weather_sync/forecast_tests.rs](../../backend/src/logic/weather_sync/forecast_tests.rs)
- [backend/src/logic/weather_sync/order_tests.rs](../../backend/src/logic/weather_sync/order_tests.rs)
- [backend/src/logic/weather_sync/schedule_db_tests.rs](../../backend/src/logic/weather_sync/schedule_db_tests.rs)
- [backend/src/logic/weather_sync/sensor_read_db_tests.rs](../../backend/src/logic/weather_sync/sensor_read_db_tests.rs)
- [backend/src/logic/weather_sync/sensor_reading_tests.rs](../../backend/src/logic/weather_sync/sensor_reading_tests.rs)
- [backend/src/mcp/tools/snapshot_db_tests.rs](../../backend/src/mcp/tools/snapshot_db_tests.rs)

### Code
- [backend/src/api/dashboard.rs](../../backend/src/api/dashboard.rs)
- [backend/src/api/environmental.rs](../../backend/src/api/environmental.rs)
- [backend/src/api/health.rs](../../backend/src/api/health.rs)
- [backend/src/api/profile.rs](../../backend/src/api/profile.rs)
- [backend/src/api/sources/write.rs](../../backend/src/api/sources/write.rs)
- [backend/src/api/timing.rs](../../backend/src/api/timing.rs)
- [backend/src/api/timing/memo.rs](../../backend/src/api/timing/memo.rs)
- [backend/src/db/weather_queries/lawn.rs](../../backend/src/db/weather_queries/lawn.rs)
- [backend/src/logic/data_sync.rs](../../backend/src/logic/data_sync.rs)
- [backend/src/logic/data_sync/assemble.rs](../../backend/src/logic/data_sync/assemble.rs)
- [backend/src/logic/data_sync/memo.rs](../../backend/src/logic/data_sync/memo.rs)
- [backend/src/logic/weather_record/due.rs](../../backend/src/logic/weather_record/due.rs)
- [backend/src/logic/weather_sync.rs](../../backend/src/logic/weather_sync.rs)
- [backend/src/logic/weather_sync/fetcher.rs](../../backend/src/logic/weather_sync/fetcher.rs)
- [backend/src/logic/weather_sync/forecast.rs](../../backend/src/logic/weather_sync/forecast.rs)
- [backend/src/logic/weather_sync/order.rs](../../backend/src/logic/weather_sync/order.rs)
- [backend/src/logic/weather_sync/schedule.rs](../../backend/src/logic/weather_sync/schedule.rs)
- [backend/src/logic/weather_sync/sensor_read.rs](../../backend/src/logic/weather_sync/sensor_read.rs)
- [backend/src/logic/weather_sync/sensor_reading.rs](../../backend/src/logic/weather_sync/sensor_reading.rs)
- [backend/src/logic/weather_sync/sensor_store.rs](../../backend/src/logic/weather_sync/sensor_store.rs)
- [backend/src/main.rs](../../backend/src/main.rs)
- [backend/src/mcp/tools/lawn.rs](../../backend/src/mcp/tools/lawn.rs)
- [backend/src/state.rs](../../backend/src/state.rs)
- [frontend/src/pages/Dashboard.tsx](../../frontend/src/pages/Dashboard.tsx)
- [frontend/src/pages/Environmental.tsx](../../frontend/src/pages/Environmental.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `SYNC-SCHED` — The fetch schedule
2. `SYNC-REFRESH` — Startup and the clock
3. `SYNC-MERGE` — Assembling the summary
4. `SYNC-AGE` — How old a value may be
5. `SYNC-FAIL` — Failures and degradation
6. `SYNC-CONN` — Source status and health
7. `SYNC-MEMO` — Climate-record slot

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| SCHED | SYNC-SCHED-001 – SYNC-SCHED-023 | 23 | 0 | 0 | 22 | 22 |
| REFRESH | SYNC-REFRESH-007 – SYNC-REFRESH-009 | 2 | 0 | 0 | 2 | 1 |
| MERGE | SYNC-MERGE-003 – SYNC-MERGE-011 | 7 | 0 | 0 | 7 | 7 |
| AGE | SYNC-AGE-001 – SYNC-AGE-004 | 4 | 0 | 0 | 4 | 4 |
| FAIL | SYNC-FAIL-001 – SYNC-FAIL-011 | 5 | 0 | 0 | 5 | 4 |
| CONN | SYNC-CONN-001 – SYNC-CONN-008 | 7 | 0 | 0 | 7 | 7 |
| MEMO | SYNC-MEMO-001 – SYNC-MEMO-002 | 2 | 0 | 0 | 2 | 2 |
| **Total** | | **50** | **0** | **0** | **49** | **47** |

**Summary:** 50 of 50 active specs implemented; 0 deferred; 47 implemented specs cited by a test, 3 not.

## Key Findings

1. **Implemented, no `@spec` in code:** SYNC-SCHED-012.
2. **Implemented, no test cites it:** SYNC-SCHED-012, SYNC-REFRESH-007, SYNC-FAIL-001.

## Work Required

### Must Fix
- None.

### Should Fix
- Add a test citing SYNC-SCHED-012, SYNC-REFRESH-007, SYNC-FAIL-001.
- Annotate the code behind SYNC-SCHED-012.

### Nice to Have
- None.
