# Arrow: seasonal-plan

Seasonal Plan — leaf segment `PLAN` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `92f4d75`): all 70 active specs implemented; 70 cited by a test, 69 cited in code.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Seasonal plan and calendar"

### LLD
- [docs/intent/seasonal-plan/seasonal-plan-design.md](../intent/seasonal-plan/seasonal-plan-design.md)

### EARS
- [docs/intent/seasonal-plan/seasonal-plan-specs.md](../intent/seasonal-plan/seasonal-plan-specs.md) (70 specs, prefix `PLAN`)

### Related docs
- [docs/timing-windows.md](../timing-windows.md)

### Tests
- [backend/src/api/calendar/db_tests.rs](../../backend/src/api/calendar/db_tests.rs)
- [backend/src/api/seasonal_plan/merge_db_tests.rs](../../backend/src/api/seasonal_plan/merge_db_tests.rs)
- [backend/src/api/seasonal_plan/plant_overlay_db_tests.rs](../../backend/src/api/seasonal_plan/plant_overlay_db_tests.rs)
- [backend/src/api/seasonal_plan/record_db_tests.rs](../../backend/src/api/seasonal_plan/record_db_tests.rs)
- [backend/src/logic/seasonal_plan/activity_tests.rs](../../backend/src/logic/seasonal_plan/activity_tests.rs)
- [backend/src/logic/seasonal_plan/completion_tests.rs](../../backend/src/logic/seasonal_plan/completion_tests.rs)
- [backend/src/logic/seasonal_plan/crossing_tests.rs](../../backend/src/logic/seasonal_plan/crossing_tests.rs)
- [backend/src/logic/seasonal_plan/details_tests.rs](../../backend/src/logic/seasonal_plan/details_tests.rs)
- [backend/src/logic/seasonal_plan/status_tests.rs](../../backend/src/logic/seasonal_plan/status_tests.rs)
- [backend/src/logic/seasonal_plan/window_tests.rs](../../backend/src/logic/seasonal_plan/window_tests.rs)
- [frontend/src/components/plan/activityStatus.test.ts](../../frontend/src/components/plan/activityStatus.test.ts)
- [frontend/src/pages/Calendar.test.tsx](../../frontend/src/pages/Calendar.test.tsx)
- [frontend/src/pages/SeasonalPlan.test.tsx](../../frontend/src/pages/SeasonalPlan.test.tsx)

### Code
- [backend/src/api/calendar.rs](../../backend/src/api/calendar.rs)
- [backend/src/api/seasonal_plan.rs](../../backend/src/api/seasonal_plan.rs)
- [backend/src/logic/seasonal_plan.rs](../../backend/src/logic/seasonal_plan.rs)
- [backend/src/logic/seasonal_plan/crossings.rs](../../backend/src/logic/seasonal_plan/crossings.rs)
- [backend/src/logic/seasonal_plan/fall.rs](../../backend/src/logic/seasonal_plan/fall.rs)
- [backend/src/logic/seasonal_plan/spring.rs](../../backend/src/logic/seasonal_plan/spring.rs)
- [backend/src/models/seasonal_plan.rs](../../backend/src/models/seasonal_plan.rs)
- [frontend/src/components/plan/activityStatus.ts](../../frontend/src/components/plan/activityStatus.ts)
- [frontend/src/pages/Calendar.tsx](../../frontend/src/pages/Calendar.tsx)
- [frontend/src/pages/SeasonalPlan.tsx](../../frontend/src/pages/SeasonalPlan.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `PLAN-CROSS` — 10 cm crossing detection
2. `PLAN-ACT` — 10 cm activities
3. `PLAN-MERGE` — Seasonal plan endpoint
4. `PLAN-CAL` — Calendar endpoint
5. `PLAN-UI` — Seasonal Plan page
6. `PLAN-CALUI` — Calendar page

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| CROSS | PLAN-CROSS-001 – PLAN-CROSS-016 | 16 | 0 | 0 | 15 | 16 |
| ACT | PLAN-ACT-001 – PLAN-ACT-023 | 23 | 0 | 0 | 23 | 23 |
| MERGE | PLAN-MERGE-001 – PLAN-MERGE-008 | 8 | 0 | 0 | 8 | 8 |
| CAL | PLAN-CAL-001 – PLAN-CAL-003 | 3 | 0 | 0 | 3 | 3 |
| UI | PLAN-UI-001 – PLAN-UI-011 | 11 | 0 | 0 | 11 | 11 |
| CALUI | PLAN-CALUI-001 – PLAN-CALUI-009 | 9 | 0 | 0 | 9 | 9 |
| **Total** | | **70** | **0** | **0** | **69** | **70** |

**Summary:** 70 of 70 active specs implemented; 0 deferred; 70 implemented specs cited by a test.

## Key Findings

1. **Implemented, no `@spec` in code:** PLAN-CROSS-016.

## Work Required

### Must Fix
- None.

### Should Fix
- Annotate the code behind PLAN-CROSS-016.

### Nice to Have
- None.
