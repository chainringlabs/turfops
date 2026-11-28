# Arrow: agronomic-rules

Agronomic Rules — leaf segment `RULE` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `92f4d75`): all 82 active specs implemented; 81 cited by a test, 82 cited in code.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Agronomic rules"

### LLD
- [docs/intent/agronomic-rules/agronomic-rules-design.md](../intent/agronomic-rules/agronomic-rules-design.md)

### EARS
- [docs/intent/agronomic-rules/agronomic-rules-specs.md](../intent/agronomic-rules/agronomic-rules-specs.md) (83 specs, prefix `RULE`)

### Related docs
- [docs/agronomy-methods.md](../agronomy-methods.md)

### Tests
- [backend/src/api/gdd/db_tests.rs](../../backend/src/api/gdd/db_tests.rs)
- [backend/src/api/nitrogen_budget/db_tests.rs](../../backend/src/api/nitrogen_budget/db_tests.rs)
- [backend/src/logic/gdd/lake_tests.rs](../../backend/src/logic/gdd/lake_tests.rs)
- [backend/src/logic/rules/broadleaf_herbicide/rule_tests.rs](../../backend/src/logic/rules/broadleaf_herbicide/rule_tests.rs)
- [backend/src/logic/rules/engine_tests.rs](../../backend/src/logic/rules/engine_tests.rs)
- [backend/src/logic/rules/fall_fertilization/rule_tests.rs](../../backend/src/logic/rules/fall_fertilization/rule_tests.rs)
- [backend/src/logic/rules/forecast_rule_tests.rs](../../backend/src/logic/rules/forecast_rule_tests.rs)
- [backend/src/logic/rules/grub_control/rule_tests.rs](../../backend/src/logic/rules/grub_control/rule_tests.rs)
- [backend/src/logic/rules/moment_tests.rs](../../backend/src/logic/rules/moment_tests.rs)
- [backend/src/logic/rules/nitrogen_guard/rule_tests.rs](../../backend/src/logic/rules/nitrogen_guard/rule_tests.rs)
- [backend/src/logic/rules/plan_state/tests.rs](../../backend/src/logic/rules/plan_state/tests.rs)
- [backend/src/logic/rules/plan_state/window_tests.rs](../../backend/src/logic/rules/plan_state/window_tests.rs)
- [backend/src/logic/rules/rain_delay_tests.rs](../../backend/src/logic/rules/rain_delay_tests.rs)
- [backend/src/logic/rules/spring_nitrogen/rule_tests.rs](../../backend/src/logic/rules/spring_nitrogen/rule_tests.rs)
- [backend/src/logic/rules/weather_rule_tests.rs](../../backend/src/logic/rules/weather_rule_tests.rs)
- [frontend/src/components/GddWidget.test.tsx](../../frontend/src/components/GddWidget.test.tsx)
- [frontend/src/components/NitrogenBudgetWidget.test.tsx](../../frontend/src/components/NitrogenBudgetWidget.test.tsx)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/nitrogen_budget.rs`, `backend/src/logic/rules/fall_fertilization/recs.rs`, `backend/src/logic/rules/fertilizer.rs`, `backend/src/logic/rules/irrigation_forecast.rs`, `backend/src/logic/rules/nitrogen_guard.rs`, `backend/src/logic/rules/spring_nitrogen.rs`, `backend/src/logic/rules/thresholds.rs`, `backend/src/models/nitrogen_budget.rs`

### Code
- [backend/src/api/gdd.rs](../../backend/src/api/gdd.rs)
- [backend/src/api/nitrogen_budget.rs](../../backend/src/api/nitrogen_budget.rs)
- [backend/src/logic/data_sync.rs](../../backend/src/logic/data_sync.rs)
- [backend/src/logic/gdd.rs](../../backend/src/logic/gdd.rs)
- [backend/src/logic/rules/application_window.rs](../../backend/src/logic/rules/application_window.rs)
- [backend/src/logic/rules/broadleaf_herbicide.rs](../../backend/src/logic/rules/broadleaf_herbicide.rs)
- [backend/src/logic/rules/engine.rs](../../backend/src/logic/rules/engine.rs)
- [backend/src/logic/rules/fall_fertilization.rs](../../backend/src/logic/rules/fall_fertilization.rs)
- [backend/src/logic/rules/fall_fertilization/recs.rs](../../backend/src/logic/rules/fall_fertilization/recs.rs)
- [backend/src/logic/rules/fertilizer.rs](../../backend/src/logic/rules/fertilizer.rs)
- [backend/src/logic/rules/grub_control.rs](../../backend/src/logic/rules/grub_control.rs)
- [backend/src/logic/rules/heat_stress.rs](../../backend/src/logic/rules/heat_stress.rs)
- [backend/src/logic/rules/irrigation_forecast.rs](../../backend/src/logic/rules/irrigation_forecast.rs)
- [backend/src/logic/rules/mod.rs](../../backend/src/logic/rules/mod.rs)
- [backend/src/logic/rules/mowing_height.rs](../../backend/src/logic/rules/mowing_height.rs)
- [backend/src/logic/rules/nitrogen_guard.rs](../../backend/src/logic/rules/nitrogen_guard.rs)
- [backend/src/logic/rules/plan_state.rs](../../backend/src/logic/rules/plan_state.rs)
- [backend/src/logic/rules/rain_delay.rs](../../backend/src/logic/rules/rain_delay.rs)
- [backend/src/logic/rules/spring_nitrogen.rs](../../backend/src/logic/rules/spring_nitrogen.rs)
- [backend/src/logic/rules/thresholds.rs](../../backend/src/logic/rules/thresholds.rs)
- [backend/src/models/lawn_profile.rs](../../backend/src/models/lawn_profile.rs)
- [backend/src/models/nitrogen_budget.rs](../../backend/src/models/nitrogen_budget.rs)
- [backend/src/models/recommendation.rs](../../backend/src/models/recommendation.rs)
- [frontend/src/components/GddWidget.tsx](../../frontend/src/components/GddWidget.tsx)
- [frontend/src/components/NitrogenBudgetWidget.tsx](../../frontend/src/components/NitrogenBudgetWidget.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `RULE-ENG` — Rule contract and engine
2. `RULE-SPRING` — Spring nitrogen
3. `RULE-BROAD` — Broadleaf herbicide
4. `RULE-GRUB` — Grub control
5. `RULE-BLOCK` — Fertilizer block
6. `RULE-FALL` — Fall fertilization
7. `RULE-WX` — Weather rules
8. `RULE-GUARD` — Nitrogen guard
9. `RULE-BUDGET` — Annual nitrogen budget
10. `RULE-GDD` — Growing degree days and the crabgrass model
11. `RULE-UI` — Dashboard widgets

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| ENG | RULE-ENG-001 – RULE-ENG-016 | 16 | 0 | 0 | 16 | 16 |
| SPRING | RULE-SPRING-001 – RULE-SPRING-010 | 10 | 0 | 0 | 10 | 10 |
| BROAD | RULE-BROAD-001 – RULE-BROAD-004 | 4 | 0 | 0 | 4 | 4 |
| GRUB | RULE-GRUB-001 – RULE-GRUB-006 | 6 | 0 | 0 | 6 | 6 |
| BLOCK | RULE-BLOCK-001 – RULE-BLOCK-002 | 2 | 0 | 0 | 2 | 2 |
| FALL | RULE-FALL-001 – RULE-FALL-007 | 7 | 0 | 0 | 7 | 7 |
| WX | RULE-WX-001 – RULE-WX-013 | 13 | 0 | 0 | 13 | 13 |
| GUARD | RULE-GUARD-001 – RULE-GUARD-011 | 10 | 1 | 0 | 10 | 10 |
| BUDGET | RULE-BUDGET-001 – RULE-BUDGET-005 | 5 | 0 | 0 | 5 | 5 |
| GDD | RULE-GDD-001 – RULE-GDD-007 | 7 | 0 | 0 | 7 | 6 |
| UI | RULE-UI-001 – RULE-UI-002 | 2 | 0 | 0 | 2 | 2 |
| **Total** | | **82** | **1** | **0** | **82** | **81** |

**Summary:** 82 of 82 active specs implemented; 1 deferred; 81 implemented specs cited by a test, 1 not.

## Key Findings

1. **Implemented, no test cites it:** RULE-GDD-002.
2. **Deferred:** RULE-GUARD-005.

## Work Required

### Must Fix
- None.

### Should Fix
- Add a test citing RULE-GDD-002.

### Nice to Have
- None.
