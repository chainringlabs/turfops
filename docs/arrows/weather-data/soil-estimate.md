# Arrow: soil-estimate

Soil Estimate — leaf segment `WX-EST` under the `weather-data` sub-HLD.

## Status

**PARTIAL** — audited 2026-11-27 (git SHA `24a3baf`): 29 of 30 active specs implemented; 1 marked `[ ]` (1 of them already cited in code).

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Weather data access"
- [docs/intent/weather-data/weather-data-design.md](../../intent/weather-data/weather-data-design.md) (parent sub-HLD)

### LLD
- [docs/intent/weather-data/soil-estimate/soil-estimate-design.md](../../intent/weather-data/soil-estimate/soil-estimate-design.md)

### EARS
- [docs/intent/weather-data/soil-estimate/soil-estimate-specs.md](../../intent/weather-data/soil-estimate/soil-estimate-specs.md) (30 specs, prefix `WX-EST`)

### Related docs
- [docs/plans/standalone-weather-sources.md](../../plans/standalone-weather-sources.md)

### Tests
- [backend/src/logic/soil_estimate/anchor_tests.rs](../../../backend/src/logic/soil_estimate/anchor_tests.rs)
- [backend/src/logic/soil_estimate/calibration_tests.rs](../../../backend/src/logic/soil_estimate/calibration_tests.rs)
- [backend/src/logic/soil_estimate/estimator_db_tests.rs](../../../backend/src/logic/soil_estimate/estimator_db_tests.rs)
- [backend/src/logic/soil_estimate/fit_db_tests.rs](../../../backend/src/logic/soil_estimate/fit_db_tests.rs)
- [backend/src/logic/soil_estimate/model_tests.rs](../../../backend/src/logic/soil_estimate/model_tests.rs)
- [backend/src/logic/soil_estimate/scores_tests.rs](../../../backend/src/logic/soil_estimate/scores_tests.rs)
- [backend/src/logic/soil_estimate/scoring/crossings_tests.rs](../../../backend/src/logic/soil_estimate/scoring/crossings_tests.rs)
- [backend/src/logic/soil_estimate/scoring/inputs_tests.rs](../../../backend/src/logic/soil_estimate/scoring/inputs_tests.rs)
- [backend/src/logic/soil_estimate/scoring/score_tests.rs](../../../backend/src/logic/soil_estimate/scoring/score_tests.rs)
- [backend/src/logic/soil_estimate/scoring/stats_tests.rs](../../../backend/src/logic/soil_estimate/scoring/stats_tests.rs)

### Code
- [backend/src/api/sources/read.rs](../../../backend/src/api/sources/read.rs)
- [backend/src/db/soil_estimate_queries.rs](../../../backend/src/db/soil_estimate_queries.rs)
- [backend/src/logic/soil_estimate/anchor.rs](../../../backend/src/logic/soil_estimate/anchor.rs)
- [backend/src/logic/soil_estimate/calibration.rs](../../../backend/src/logic/soil_estimate/calibration.rs)
- [backend/src/logic/soil_estimate/estimator.rs](../../../backend/src/logic/soil_estimate/estimator.rs)
- [backend/src/logic/soil_estimate/fit.rs](../../../backend/src/logic/soil_estimate/fit.rs)
- [backend/src/logic/soil_estimate/model.rs](../../../backend/src/logic/soil_estimate/model.rs)
- [backend/src/logic/soil_estimate/scores.rs](../../../backend/src/logic/soil_estimate/scores.rs)
- [backend/src/logic/soil_estimate/scoring/crossings.rs](../../../backend/src/logic/soil_estimate/scoring/crossings.rs)
- [backend/src/logic/soil_estimate/scoring/inputs.rs](../../../backend/src/logic/soil_estimate/scoring/inputs.rs)
- [backend/src/logic/soil_estimate/scoring/run.rs](../../../backend/src/logic/soil_estimate/scoring/run.rs)
- [backend/src/logic/soil_estimate/scoring/score.rs](../../../backend/src/logic/soil_estimate/scoring/score.rs)
- [backend/src/logic/soil_estimate/scoring/stats.rs](../../../backend/src/logic/soil_estimate/scoring/stats.rs)
- [backend/src/logic/soil_estimate/series.rs](../../../backend/src/logic/soil_estimate/series.rs)
- [backend/src/logic/weather_record/due.rs](../../../backend/src/logic/weather_record/due.rs)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `WX-EST-MODEL` — The regression and its series
2. `WX-EST-CAL` — Calibration
3. `WX-EST-ANCHOR` — The anchored series
4. `WX-EST-FWD` — The forward series
5. `WX-EST-DATE` — Which crossings may be dated
6. `WX-EST-SCORE` — The scoring tool and its data file
7. `WX-EST-FAIL` — Degradation

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| MODEL | WX-EST-MODEL-001 – WX-EST-MODEL-006 | 6 | 0 | 0 | 6 | 4 |
| CAL | WX-EST-CAL-001 – WX-EST-CAL-008 | 8 | 0 | 0 | 8 | 8 |
| ANCHOR | WX-EST-ANCHOR-001 – WX-EST-ANCHOR-004 | 4 | 0 | 0 | 4 | 4 |
| FWD | WX-EST-FWD-001 – WX-EST-FWD-002 | 2 | 0 | 0 | 2 | 2 |
| DATE | WX-EST-DATE-001 – WX-EST-DATE-003 | 3 | 0 | 0 | 3 | 3 |
| SCORE | WX-EST-SCORE-001 – WX-EST-SCORE-004 | 3 | 0 | 1 | 4 | 3 |
| FAIL | WX-EST-FAIL-001 – WX-EST-FAIL-003 | 3 | 0 | 0 | 3 | 3 |
| **Total** | | **29** | **0** | **1** | **30** | **27** |

**Summary:** 29 of 30 active specs implemented; 0 deferred; 26 implemented specs cited by a test, 3 not.

## Key Findings

1. **Marked `[ ]` but cited in code:** WX-EST-SCORE-003. Either the code is done and the marker is stale, or the annotation claims more than the code does; check each.
2. **Implemented, no test cites it:** WX-EST-MODEL-004, WX-EST-MODEL-005, WX-EST-SCORE-004.

## Work Required

### Must Fix
- None.

### Should Fix
- Confirm WX-EST-SCORE-003 against the code and set their markers.
- Add a test citing WX-EST-MODEL-004, WX-EST-MODEL-005, WX-EST-SCORE-004.

### Nice to Have
- None.
