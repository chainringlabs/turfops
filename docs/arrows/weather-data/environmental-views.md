# Arrow: environmental-views

Environmental Views — leaf segment `WX-VIEW` under the `weather-data` sub-HLD.

## Status

**PARTIAL** — audited 2026-11-27 (git SHA `44031e1`): 24 of 26 active specs implemented; 2 marked `[ ]` (2 of them already cited in code).

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Weather data access"
- [docs/intent/weather-data/weather-data-design.md](../../intent/weather-data/weather-data-design.md) (parent sub-HLD)

### LLD
- [docs/intent/weather-data/environmental-views/environmental-views-design.md](../../intent/weather-data/environmental-views/environmental-views-design.md)

### EARS
- [docs/intent/weather-data/environmental-views/environmental-views-specs.md](../../intent/weather-data/environmental-views/environmental-views-specs.md) (26 specs, prefix `WX-VIEW`)

### Tests
- [backend/src/api/historical/db_tests.rs](../../../backend/src/api/historical/db_tests.rs)
- [backend/src/logic/data_sync/assemble_tests.rs](../../../backend/src/logic/data_sync/assemble_tests.rs)
- [backend/src/logic/data_sync/record_db_tests.rs](../../../backend/src/logic/data_sync/record_db_tests.rs)
- [frontend/src/components/environmental/display.test.ts](../../../frontend/src/components/environmental/display.test.ts)
- [frontend/src/components/environmental/trendBasis.test.ts](../../../frontend/src/components/environmental/trendBasis.test.ts)
- [frontend/src/components/sources/BasisLabel.test.tsx](../../../frontend/src/components/sources/BasisLabel.test.tsx)
- [frontend/src/components/sources/display.test.ts](../../../frontend/src/components/sources/display.test.ts)
- [frontend/src/pages/Environmental.test.tsx](../../../frontend/src/pages/Environmental.test.tsx)
- [frontend/src/pages/Environmental.trends.test.tsx](../../../frontend/src/pages/Environmental.trends.test.tsx)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/historical.rs`

### Code
- [backend/src/api/environmental.rs](../../../backend/src/api/environmental.rs)
- [backend/src/api/historical.rs](../../../backend/src/api/historical.rs)
- [backend/src/logic/data_sync.rs](../../../backend/src/logic/data_sync.rs)
- [backend/src/models/environmental.rs](../../../backend/src/models/environmental.rs)
- [backend/src/models/historical.rs](../../../backend/src/models/historical.rs)
- [frontend/src/components/TrendChart.tsx](../../../frontend/src/components/TrendChart.tsx)
- [frontend/src/components/environmental/GaugeCard.tsx](../../../frontend/src/components/environmental/GaugeCard.tsx)
- [frontend/src/components/environmental/display.ts](../../../frontend/src/components/environmental/display.ts)
- [frontend/src/components/environmental/trendBasis.ts](../../../frontend/src/components/environmental/trendBasis.ts)
- [frontend/src/components/gaugeConfigs.ts](../../../frontend/src/components/gaugeConfigs.ts)
- [frontend/src/components/sources/BasisLabel.tsx](../../../frontend/src/components/sources/BasisLabel.tsx)
- [frontend/src/components/sources/display.ts](../../../frontend/src/components/sources/display.ts)
- [frontend/src/pages/Environmental.tsx](../../../frontend/src/pages/Environmental.tsx)
- [frontend/src/utils/units.ts](../../../frontend/src/utils/units.ts)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `WX-VIEW-API` — Endpoints
2. `WX-VIEW-BASIS` — How a basis is shown
3. `WX-VIEW-STATUS` — Source status
4. `WX-VIEW-PAGE` — Environmental page

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| API | WX-VIEW-API-001 – WX-VIEW-API-007 | 7 | 0 | 0 | 7 | 7 |
| BASIS | WX-VIEW-BASIS-001 – WX-VIEW-BASIS-004 | 3 | 0 | 1 | 4 | 4 |
| STATUS | WX-VIEW-STATUS-001 – WX-VIEW-STATUS-002 | 2 | 0 | 0 | 1 | 2 |
| PAGE | WX-VIEW-PAGE-001 – WX-VIEW-PAGE-013 | 12 | 0 | 1 | 12 | 12 |
| **Total** | | **24** | **0** | **2** | **24** | **25** |

**Summary:** 24 of 26 active specs implemented; 0 deferred; 23 implemented specs cited by a test, 1 not.

## Key Findings

1. **Marked `[ ]` but cited in code:** WX-VIEW-BASIS-001, WX-VIEW-PAGE-008. Either the code is done and the marker is stale, or the annotation claims more than the code does; check each.
2. **Implemented, no `@spec` in code:** WX-VIEW-STATUS-002, WX-VIEW-PAGE-013.
3. **Implemented, no test cites it:** WX-VIEW-PAGE-013.

## Work Required

### Must Fix
- None.

### Should Fix
- Confirm WX-VIEW-BASIS-001, WX-VIEW-PAGE-008 against the code and set their markers.
- Add a test citing WX-VIEW-PAGE-013.
- Annotate the code behind WX-VIEW-STATUS-002, WX-VIEW-PAGE-013.

### Nice to Have
- None.
