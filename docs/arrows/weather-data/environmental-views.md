# Arrow: environmental-views

Environmental Views — leaf segment `WX-VIEW` under the `weather-data` sub-HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `1b12d9d`): all 26 active specs implemented; 25 cited by a test, 24 cited in code.

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Weather data access"
- [docs/intent/weather-data/weather-data-design.md](../../intent/weather-data/weather-data-design.md) (parent sub-HLD)

### LLD
- [docs/intent/weather-data/environmental-views/environmental-views-design.md](../../intent/weather-data/environmental-views/environmental-views-design.md)

### EARS
- [docs/intent/weather-data/environmental-views/environmental-views-specs.md](../../intent/weather-data/environmental-views/environmental-views-specs.md) (26 specs, prefix `WX-VIEW`)

### Tests
- [backend/src/api/gdd/db_tests.rs](../../../backend/src/api/gdd/db_tests.rs)
- [backend/src/api/historical/db_tests.rs](../../../backend/src/api/historical/db_tests.rs)
- [backend/src/logic/data_sync/assemble_tests.rs](../../../backend/src/logic/data_sync/assemble_tests.rs)
- [backend/src/logic/data_sync/record_db_tests.rs](../../../backend/src/logic/data_sync/record_db_tests.rs)
- [backend/src/logic/disease/recommendations/feed_tests.rs](../../../backend/src/logic/disease/recommendations/feed_tests.rs)
- [backend/src/logic/seasonal_plan/crossing_tests.rs](../../../backend/src/logic/seasonal_plan/crossing_tests.rs)
- [backend/src/models/recommendation/contract_tests.rs](../../../backend/src/models/recommendation/contract_tests.rs)
- [frontend/src/components/environmental/display.test.ts](../../../frontend/src/components/environmental/display.test.ts)
- [frontend/src/components/environmental/trendBasis.test.ts](../../../frontend/src/components/environmental/trendBasis.test.ts)
- [frontend/src/components/sources/BasisLabel.test.tsx](../../../frontend/src/components/sources/BasisLabel.test.tsx)
- [frontend/src/components/sources/basisEverywhere.test.tsx](../../../frontend/src/components/sources/basisEverywhere.test.tsx)
- [frontend/src/components/sources/display.test.ts](../../../frontend/src/components/sources/display.test.ts)
- [frontend/src/pages/Environmental.test.tsx](../../../frontend/src/pages/Environmental.test.tsx)
- [frontend/src/pages/Environmental.trends.test.tsx](../../../frontend/src/pages/Environmental.trends.test.tsx)
- [frontend/src/pages/Recommendations.test.tsx](../../../frontend/src/pages/Recommendations.test.tsx)
- [frontend/src/pages/SeasonalPlan.test.tsx](../../../frontend/src/pages/SeasonalPlan.test.tsx)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/historical.rs`, `backend/src/logic/rules/fertilizer.rs`, `backend/src/models/environmental.rs`

### Code
- [backend/src/api/environmental.rs](../../../backend/src/api/environmental.rs)
- [backend/src/api/gdd.rs](../../../backend/src/api/gdd.rs)
- [backend/src/api/historical.rs](../../../backend/src/api/historical.rs)
- [backend/src/api/seasonal_plan.rs](../../../backend/src/api/seasonal_plan.rs)
- [backend/src/logic/data_sync.rs](../../../backend/src/logic/data_sync.rs)
- [backend/src/logic/disease/recommendations.rs](../../../backend/src/logic/disease/recommendations.rs)
- [backend/src/logic/weather_record/daily.rs](../../../backend/src/logic/weather_record/daily.rs)
- [backend/src/models/environmental.rs](../../../backend/src/models/environmental.rs)
- [backend/src/models/historical.rs](../../../backend/src/models/historical.rs)
- [backend/src/models/recommendation.rs](../../../backend/src/models/recommendation.rs)
- [frontend/src/components/GddWidget.tsx](../../../frontend/src/components/GddWidget.tsx)
- [frontend/src/components/SoilTempForecastWidget.tsx](../../../frontend/src/components/SoilTempForecastWidget.tsx)
- [frontend/src/components/TrendChart.tsx](../../../frontend/src/components/TrendChart.tsx)
- [frontend/src/components/disease/DiseaseRiskWidget.tsx](../../../frontend/src/components/disease/DiseaseRiskWidget.tsx)
- [frontend/src/components/environmental/GaugeCard.tsx](../../../frontend/src/components/environmental/GaugeCard.tsx)
- [frontend/src/components/environmental/display.ts](../../../frontend/src/components/environmental/display.ts)
- [frontend/src/components/environmental/trendBasis.ts](../../../frontend/src/components/environmental/trendBasis.ts)
- [frontend/src/components/gaugeConfigs.ts](../../../frontend/src/components/gaugeConfigs.ts)
- [frontend/src/components/sources/BasisLabel.tsx](../../../frontend/src/components/sources/BasisLabel.tsx)
- [frontend/src/components/sources/display.ts](../../../frontend/src/components/sources/display.ts)
- [frontend/src/components/timing/TimingWidget.tsx](../../../frontend/src/components/timing/TimingWidget.tsx)
- [frontend/src/pages/Calendar.tsx](../../../frontend/src/pages/Calendar.tsx)
- [frontend/src/pages/Environmental.tsx](../../../frontend/src/pages/Environmental.tsx)
- [frontend/src/pages/Recommendations.tsx](../../../frontend/src/pages/Recommendations.tsx)
- [frontend/src/pages/SeasonalPlan.tsx](../../../frontend/src/pages/SeasonalPlan.tsx)
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
| BASIS | WX-VIEW-BASIS-001 – WX-VIEW-BASIS-004 | 4 | 0 | 0 | 4 | 4 |
| STATUS | WX-VIEW-STATUS-001 – WX-VIEW-STATUS-002 | 2 | 0 | 0 | 1 | 2 |
| PAGE | WX-VIEW-PAGE-001 – WX-VIEW-PAGE-013 | 13 | 0 | 0 | 12 | 12 |
| **Total** | | **26** | **0** | **0** | **24** | **25** |

**Summary:** 26 of 26 active specs implemented; 0 deferred; 25 implemented specs cited by a test, 1 not.

## Key Findings

1. **Implemented, no `@spec` in code:** WX-VIEW-STATUS-002, WX-VIEW-PAGE-013.
2. **Implemented, no test cites it:** WX-VIEW-PAGE-013.

## Work Required

### Must Fix
- None.

### Should Fix
- Add a test citing WX-VIEW-PAGE-013.
- Annotate the code behind WX-VIEW-STATUS-002, WX-VIEW-PAGE-013.

### Nice to Have
- None.
