# Arrow: web-ui

Web UI — leaf segment `UI` under the root HLD.

## Status

**PARTIAL** — audited 2026-11-27 (git SHA `24a3baf`): 63 of 65 active specs implemented; 2 marked `[ ]` (2 of them already cited in code).

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Web UI"

### LLD
- [docs/intent/web-ui/web-ui-design.md](../intent/web-ui/web-ui-design.md)

### EARS
- [docs/intent/web-ui/web-ui-specs.md](../intent/web-ui/web-ui-specs.md) (65 specs, prefix `UI`)

### Related docs
- [docs/timing-windows.md](../timing-windows.md)

### Tests
- [backend/src/config/cors_origin_tests.rs](../../backend/src/config/cors_origin_tests.rs)
- [backend/src/config/load_tests.rs](../../backend/src/config/load_tests.rs)
- [backend/src/error/error_body_tests.rs](../../backend/src/error/error_body_tests.rs)
- [backend/src/error/log_level_tests.rs](../../backend/src/error/log_level_tests.rs)
- [backend/src/web/cors_tests.rs](../../backend/src/web/cors_tests.rs)
- [backend/src/web/limits_tests.rs](../../backend/src/web/limits_tests.rs)
- [backend/src/web/tests.rs](../../backend/src/web/tests.rs)
- [frontend/src/App.test.tsx](../../frontend/src/App.test.tsx)
- [frontend/src/api/client.contract.test.ts](../../frontend/src/api/client.contract.test.ts)
- [frontend/src/api/client.test.ts](../../frontend/src/api/client.test.ts)
- [frontend/src/api/errors.test.ts](../../frontend/src/api/errors.test.ts)
- [frontend/src/components/Layout.test.tsx](../../frontend/src/components/Layout.test.tsx)
- [frontend/src/components/PredictionChart.test.tsx](../../frontend/src/components/PredictionChart.test.tsx)
- [frontend/src/components/SoilTempForecastWidget.test.tsx](../../frontend/src/components/SoilTempForecastWidget.test.tsx)
- [frontend/src/components/dashboard/loadDashboard.test.ts](../../frontend/src/components/dashboard/loadDashboard.test.ts)
- [frontend/src/components/disease/DiseaseDetail.test.tsx](../../frontend/src/components/disease/DiseaseDetail.test.tsx)
- [frontend/src/components/timing/SoilSeasonChart.test.tsx](../../frontend/src/components/timing/SoilSeasonChart.test.tsx)
- [frontend/src/pages/DashboardLoad.test.tsx](../../frontend/src/pages/DashboardLoad.test.tsx)
- [frontend/src/pages/DiseaseRisk.test.tsx](../../frontend/src/pages/DiseaseRisk.test.tsx)
- [frontend/src/pages/Environmental.test.tsx](../../frontend/src/pages/Environmental.test.tsx)
- [frontend/src/pages/Timing.test.tsx](../../frontend/src/pages/Timing.test.tsx)
- [frontend/src/pages/pageErrors.test.tsx](../../frontend/src/pages/pageErrors.test.tsx)
- [frontend/src/test/setup.test.tsx](../../frontend/src/test/setup.test.tsx)
- [frontend/src/test/setup.ts](../../frontend/src/test/setup.ts)
- [frontend/src/types/mirror.test.ts](../../frontend/src/types/mirror.test.ts)
- [frontend/src/types/statusPalette.test.ts](../../frontend/src/types/statusPalette.test.ts)
- [frontend/src/utils/dates.test.ts](../../frontend/src/utils/dates.test.ts)
- [frontend/src/utils/helpers.test.ts](../../frontend/src/utils/helpers.test.ts)
- [frontend/tests/repo.test.ts](../../frontend/tests/repo.test.ts)
- [frontend/tests/shell.test.ts](../../frontend/tests/shell.test.ts)
- [frontend/tests/tooling.test.ts](../../frontend/tests/tooling.test.ts)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/json_errors.rs`, `backend/src/error.rs`

### Code
- [.github/workflows/ci.yml](../../.github/workflows/ci.yml)
- [backend/src/api/json_errors.rs](../../backend/src/api/json_errors.rs)
- [backend/src/config/load.rs](../../backend/src/config/load.rs)
- [backend/src/error.rs](../../backend/src/error.rs)
- [backend/src/main.rs](../../backend/src/main.rs)
- [backend/src/web.rs](../../backend/src/web.rs)
- [frontend/biome.jsonc](../../frontend/biome.jsonc)
- [frontend/index.html](../../frontend/index.html)
- [frontend/src/App.tsx](../../frontend/src/App.tsx)
- [frontend/src/api/client.ts](../../frontend/src/api/client.ts)
- [frontend/src/api/errors.ts](../../frontend/src/api/errors.ts)
- [frontend/src/components/ChartTable.tsx](../../frontend/src/components/ChartTable.tsx)
- [frontend/src/components/Layout.tsx](../../frontend/src/components/Layout.tsx)
- [frontend/src/components/PredictionChart.tsx](../../frontend/src/components/PredictionChart.tsx)
- [frontend/src/components/SoilTempForecastWidget.tsx](../../frontend/src/components/SoilTempForecastWidget.tsx)
- [frontend/src/components/dashboard/loadDashboard.ts](../../frontend/src/components/dashboard/loadDashboard.ts)
- [frontend/src/components/disease/DiseaseDetail.tsx](../../frontend/src/components/disease/DiseaseDetail.tsx)
- [frontend/src/components/timing/SoilSeasonChart.tsx](../../frontend/src/components/timing/SoilSeasonChart.tsx)
- [frontend/src/components/useServerToday.ts](../../frontend/src/components/useServerToday.ts)
- [frontend/src/pages/Applications.tsx](../../frontend/src/pages/Applications.tsx)
- [frontend/src/pages/Dashboard.tsx](../../frontend/src/pages/Dashboard.tsx)
- [frontend/src/pages/DiseaseRisk.tsx](../../frontend/src/pages/DiseaseRisk.tsx)
- [frontend/src/pages/Environmental.tsx](../../frontend/src/pages/Environmental.tsx)
- [frontend/src/pages/SoilTests.tsx](../../frontend/src/pages/SoilTests.tsx)
- [frontend/src/pages/Timing.tsx](../../frontend/src/pages/Timing.tsx)
- [frontend/src/types/disease.ts](../../frontend/src/types/disease.ts)
- [frontend/src/types/index.ts](../../frontend/src/types/index.ts)
- [frontend/src/types/inventory.ts](../../frontend/src/types/inventory.ts)
- [frontend/src/types/timing.ts](../../frontend/src/types/timing.ts)
- [frontend/src/utils/dates.ts](../../frontend/src/utils/dates.ts)
- [frontend/src/utils/units.ts](../../frontend/src/utils/units.ts)
- [frontend/tsconfig.app.json](../../frontend/tsconfig.app.json)
- [frontend/tsconfig.json](../../frontend/tsconfig.json)
- [frontend/vite.config.ts](../../frontend/vite.config.ts)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `UI-SHELL` — Shell and routing
2. `UI-NAV` — Navigation
3. `UI-DASH` — Dashboard data loading
4. `UI-API` — API client
5. `UI-ERR` — Human-readable errors
6. `UI-TYPE` — Shared type mirror
7. `UI-UTIL` — Shared helpers
8. `UI-CONV` — Presentation conventions
9. `UI-BUILD` — Build, lint and type-check tooling
10. `UI-SRV` — SPA serving and CORS

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| SHELL | UI-SHELL-001 – UI-SHELL-007 | 7 | 0 | 0 | 7 | 7 |
| NAV | UI-NAV-001 – UI-NAV-003 | 3 | 0 | 0 | 3 | 3 |
| DASH | UI-DASH-001 – UI-DASH-003 | 3 | 0 | 0 | 3 | 3 |
| API | UI-API-001 – UI-API-006 | 6 | 0 | 0 | 6 | 6 |
| ERR | UI-ERR-001 – UI-ERR-012 | 10 | 0 | 2 | 12 | 12 |
| TYPE | UI-TYPE-001 – UI-TYPE-004 | 4 | 0 | 0 | 4 | 4 |
| UTIL | UI-UTIL-001 – UI-UTIL-006 | 6 | 0 | 0 | 6 | 6 |
| CONV | UI-CONV-001 – UI-CONV-005 | 5 | 0 | 0 | 5 | 5 |
| BUILD | UI-BUILD-001 – UI-BUILD-010 | 10 | 0 | 0 | 10 | 10 |
| SRV | UI-SRV-001 – UI-SRV-010 | 9 | 0 | 0 | 9 | 9 |
| **Total** | | **63** | **0** | **2** | **65** | **65** |

**Summary:** 63 of 65 active specs implemented; 0 deferred; 63 implemented specs cited by a test.

## Key Findings

1. **Marked `[ ]` but cited in code:** UI-ERR-002, UI-ERR-003. Either the code is done and the marker is stale, or the annotation claims more than the code does; check each.

## Work Required

### Must Fix
- None.

### Should Fix
- Confirm UI-ERR-002, UI-ERR-003 against the code and set their markers.

### Nice to Have
- None.
