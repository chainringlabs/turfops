# Arrow: soil-tests

Soil Tests — leaf segment `SOIL` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `92f4d75`): all 64 active specs implemented; 64 cited by a test, 57 cited in code.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Soil tests"

### LLD
- [docs/intent/soil-tests/soil-tests-design.md](../intent/soil-tests/soil-tests-design.md)

### EARS
- [docs/intent/soil-tests/soil-tests-specs.md](../intent/soil-tests/soil-tests-specs.md) (72 specs, prefix `SOIL`)

### Related docs
- [docs/plans/product-inventory.md](../plans/product-inventory.md)

### Tests
- [backend/src/api/soil_tests.rs](../../backend/src/api/soil_tests.rs)
- [backend/src/api/soil_tests/db_data_tests.rs](../../backend/src/api/soil_tests/db_data_tests.rs)
- [backend/src/api/soil_tests/db_tests.rs](../../backend/src/api/soil_tests/db_tests.rs)
- [backend/src/api/soil_tests/tests.rs](../../backend/src/api/soil_tests/tests.rs)
- [backend/src/logic/soil_test_recommendations/advice_tests.rs](../../backend/src/logic/soil_test_recommendations/advice_tests.rs)
- [backend/src/logic/soil_test_recommendations/gap_tests.rs](../../backend/src/logic/soil_test_recommendations/gap_tests.rs)
- [backend/src/logic/soil_test_recommendations/npk_tests.rs](../../backend/src/logic/soil_test_recommendations/npk_tests.rs)
- [backend/src/logic/soil_test_recommendations/ph_tests.rs](../../backend/src/logic/soil_test_recommendations/ph_tests.rs)
- [backend/src/logic/soil_test_recommendations/tests.rs](../../backend/src/logic/soil_test_recommendations/tests.rs)
- [backend/src/models/soil_test.rs](../../backend/src/models/soil_test.rs)
- [frontend/src/components/soilTests/AdvicePanel.test.tsx](../../frontend/src/components/soilTests/AdvicePanel.test.tsx)
- [frontend/src/components/soilTests/display.test.ts](../../frontend/src/components/soilTests/display.test.ts)
- [frontend/src/pages/SoilTests.route.test.tsx](../../frontend/src/pages/SoilTests.route.test.tsx)
- [frontend/src/pages/SoilTests.test.tsx](../../frontend/src/pages/SoilTests.test.tsx)

### Code
- [backend/src/db/soil_test_queries.rs](../../backend/src/db/soil_test_queries.rs)
- [backend/src/logic/soil_test_recommendations.rs](../../backend/src/logic/soil_test_recommendations.rs)
- [backend/src/logic/soil_test_recommendations/ratio.rs](../../backend/src/logic/soil_test_recommendations/ratio.rs)
- [frontend/src/App.tsx](../../frontend/src/App.tsx)
- [frontend/src/components/soilTests/AdvicePanel.tsx](../../frontend/src/components/soilTests/AdvicePanel.tsx)
- [frontend/src/components/soilTests/LevelBadge.tsx](../../frontend/src/components/soilTests/LevelBadge.tsx)
- [frontend/src/components/soilTests/display.ts](../../frontend/src/components/soilTests/display.ts)
- [frontend/src/pages/SoilTests.tsx](../../frontend/src/pages/SoilTests.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `SOIL-DATA` — Stored tests
2. `SOIL-API` — Endpoints
3. `SOIL-ADV` — Advice rules
4. `SOIL-PH` — pH correction
5. `SOIL-NPK` — N-P-K advice
6. `SOIL-NUTR` — Nutrient deficiencies
7. `SOIL-UI` — Soil Tests page
8. `SOIL-REJOIN` — Rejoining the feed

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| DATA | SOIL-DATA-001 – SOIL-DATA-005 | 5 | 0 | 0 | 2 | 5 |
| API | SOIL-API-001 – SOIL-API-012 | 12 | 0 | 0 | 8 | 12 |
| ADV | SOIL-ADV-001 – SOIL-ADV-005 | 5 | 0 | 0 | 5 | 5 |
| PH | SOIL-PH-001 – SOIL-PH-007 | 6 | 1 | 0 | 6 | 6 |
| NPK | SOIL-NPK-001 – SOIL-NPK-013 | 13 | 0 | 0 | 13 | 13 |
| NUTR | SOIL-NUTR-001 – SOIL-NUTR-005 | 5 | 0 | 0 | 5 | 5 |
| UI | SOIL-UI-001 – SOIL-UI-018 | 18 | 0 | 0 | 18 | 18 |
| REJOIN | SOIL-REJOIN-001 – SOIL-REJOIN-007 | 0 | 7 | 0 | 0 | 0 |
| **Total** | | **64** | **8** | **0** | **57** | **64** |

**Summary:** 64 of 64 active specs implemented; 8 deferred; 64 implemented specs cited by a test.

## Key Findings

1. **Implemented, no `@spec` in code:** SOIL-DATA-001, SOIL-DATA-002, SOIL-DATA-004, SOIL-API-006, SOIL-API-007, SOIL-API-010, SOIL-API-011.
2. **Deferred:** SOIL-PH-007, SOIL-REJOIN-001, SOIL-REJOIN-002, SOIL-REJOIN-003, SOIL-REJOIN-004, SOIL-REJOIN-005, SOIL-REJOIN-006, SOIL-REJOIN-007.

## Work Required

### Must Fix
- None.

### Should Fix
- Annotate the code behind SOIL-DATA-001, SOIL-DATA-002, SOIL-DATA-004, SOIL-API-006, SOIL-API-007, SOIL-API-010, SOIL-API-011.

### Nice to Have
- None.
