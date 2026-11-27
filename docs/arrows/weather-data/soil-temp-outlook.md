# Arrow: soil-temp-outlook

Soil-Temperature Outlook — leaf segment `WX-OUTLOOK` under the `weather-data` sub-HLD.

## Status

**OK** — audited 2026-11-27 (git SHA `24a3baf`): all 25 active specs implemented, each cited in code and by a test.

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Weather data access"
- [docs/intent/weather-data/weather-data-design.md](../../intent/weather-data/weather-data-design.md) (parent sub-HLD)

### LLD
- [docs/intent/weather-data/soil-temp-outlook/soil-temp-outlook-design.md](../../intent/weather-data/soil-temp-outlook/soil-temp-outlook-design.md)

### EARS
- [docs/intent/weather-data/soil-temp-outlook/soil-temp-outlook-specs.md](../../intent/weather-data/soil-temp-outlook/soil-temp-outlook-specs.md) (25 specs, prefix `WX-OUTLOOK`)

### Related docs
- [docs/agronomy-methods.md](../../agronomy-methods.md)
- [docs/timing-windows.md](../../timing-windows.md)

### Tests
- [backend/src/api/soil_temp_prediction/db_tests.rs](../../../backend/src/api/soil_temp_prediction/db_tests.rs)
- [backend/src/logic/soil_temp_prediction/crossing_tests.rs](../../../backend/src/logic/soil_temp_prediction/crossing_tests.rs)
- [backend/src/logic/soil_temp_prediction/dip_tests.rs](../../../backend/src/logic/soil_temp_prediction/dip_tests.rs)
- [backend/src/logic/soil_temp_prediction/method_tests.rs](../../../backend/src/logic/soil_temp_prediction/method_tests.rs)
- [backend/src/logic/soil_temp_prediction/model_tests.rs](../../../backend/src/logic/soil_temp_prediction/model_tests.rs)
- [backend/src/logic/soil_temp_prediction/outlook_tests.rs](../../../backend/src/logic/soil_temp_prediction/outlook_tests.rs)
- [backend/src/logic/soil_temp_prediction/tests.rs](../../../backend/src/logic/soil_temp_prediction/tests.rs)
- [frontend/src/components/PredictionChart.test.tsx](../../../frontend/src/components/PredictionChart.test.tsx)
- [frontend/src/components/SoilTempForecastWidget.test.tsx](../../../frontend/src/components/SoilTempForecastWidget.test.tsx)
- [frontend/src/components/environmental/display.test.ts](../../../frontend/src/components/environmental/display.test.ts)
- [frontend/src/pages/Dashboard.soilWidget.test.tsx](../../../frontend/src/pages/Dashboard.soilWidget.test.tsx)
- [frontend/src/pages/Environmental.test.tsx](../../../frontend/src/pages/Environmental.test.tsx)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/error.rs`

### Code
- [backend/src/api/soil_temp_prediction.rs](../../../backend/src/api/soil_temp_prediction.rs)
- [backend/src/logic/soil_temp_prediction.rs](../../../backend/src/logic/soil_temp_prediction.rs)
- [backend/src/logic/soil_temp_prediction/outlook.rs](../../../backend/src/logic/soil_temp_prediction/outlook.rs)
- [backend/src/logic/soil_temp_prediction/series.rs](../../../backend/src/logic/soil_temp_prediction/series.rs)
- [backend/src/models/soil_temp_prediction.rs](../../../backend/src/models/soil_temp_prediction.rs)
- [frontend/src/components/PredictionChart.tsx](../../../frontend/src/components/PredictionChart.tsx)
- [frontend/src/components/SoilTempForecastWidget.tsx](../../../frontend/src/components/SoilTempForecastWidget.tsx)
- [frontend/src/components/environmental/display.ts](../../../frontend/src/components/environmental/display.ts)
- [frontend/src/pages/Dashboard.tsx](../../../frontend/src/pages/Dashboard.tsx)
- [frontend/src/pages/Environmental.tsx](../../../frontend/src/pages/Environmental.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `WX-OUTLOOK-MODEL` — Regression and predictions
2. `WX-OUTLOOK-METHOD` — Fitted or estimated
3. `WX-OUTLOOK-CROSS` — Thresholds and crossings
4. `WX-OUTLOOK-API` — Endpoint
5. `WX-OUTLOOK-UI` — UI

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| MODEL | WX-OUTLOOK-MODEL-001 – WX-OUTLOOK-MODEL-008 | 8 | 0 | 0 | 8 | 8 |
| METHOD | WX-OUTLOOK-METHOD-001 – WX-OUTLOOK-METHOD-004 | 4 | 0 | 0 | 4 | 4 |
| CROSS | WX-OUTLOOK-CROSS-001 – WX-OUTLOOK-CROSS-006 | 6 | 0 | 0 | 6 | 6 |
| API | WX-OUTLOOK-API-001 – WX-OUTLOOK-API-003 | 3 | 0 | 0 | 3 | 3 |
| UI | WX-OUTLOOK-UI-001 – WX-OUTLOOK-UI-004 | 4 | 0 | 0 | 4 | 4 |
| **Total** | | **25** | **0** | **0** | **25** | **25** |

**Summary:** 25 of 25 active specs implemented; 0 deferred; 25 implemented specs cited by a test.

## Key Findings

1. No findings.

## Work Required

### Must Fix
- None.

### Should Fix
- None.

### Nice to Have
- None.
