# Arrow: recommendation-feed

Recommendation Feed — leaf segment `FEED` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `83eb10a`): all 52 active specs implemented, each cited in code and by a test.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Recommendation feed and state"

### LLD
- [docs/intent/recommendation-feed/recommendation-feed-design.md](../intent/recommendation-feed/recommendation-feed-design.md)

### EARS
- [docs/intent/recommendation-feed/recommendation-feed-specs.md](../intent/recommendation-feed/recommendation-feed-specs.md) (53 specs, prefix `FEED`)

### Tests
- [backend/src/api/recommendation_feed/clock_db_tests.rs](../../backend/src/api/recommendation_feed/clock_db_tests.rs)
- [backend/src/api/recommendation_feed/feed_db_tests.rs](../../backend/src/api/recommendation_feed/feed_db_tests.rs)
- [backend/src/api/recommendation_feed/outage_tests.rs](../../backend/src/api/recommendation_feed/outage_tests.rs)
- [backend/src/api/recommendation_feed/shared_log_db_tests.rs](../../backend/src/api/recommendation_feed/shared_log_db_tests.rs)
- [backend/src/api/recommendation_feed/stage_tests.rs](../../backend/src/api/recommendation_feed/stage_tests.rs)
- [backend/src/api/recommendations/db_tests.rs](../../backend/src/api/recommendations/db_tests.rs)
- [backend/src/models/recommendation/contract_tests.rs](../../backend/src/models/recommendation/contract_tests.rs)
- [backend/src/models/recommendation_state/policy_tests.rs](../../backend/src/models/recommendation_state/policy_tests.rs)
- [frontend/src/components/dashboard/loadDashboard.test.ts](../../frontend/src/components/dashboard/loadDashboard.test.ts)
- [frontend/src/components/sources/display.test.ts](../../frontend/src/components/sources/display.test.ts)
- [frontend/src/pages/Dashboard.test.tsx](../../frontend/src/pages/Dashboard.test.tsx)
- [frontend/src/pages/Recommendations.test.tsx](../../frontend/src/pages/Recommendations.test.tsx)
- [frontend/tests/repo.test.ts](../../frontend/tests/repo.test.ts)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/recommendation_feed.rs`, `backend/src/logic/rules/fertilizer.rs`, `backend/src/logic/rules/spring_nitrogen.rs`, `backend/src/models/recommendation_state.rs`

### Code
- [backend/src/api/dashboard.rs](../../backend/src/api/dashboard.rs)
- [backend/src/api/disease_risk.rs](../../backend/src/api/disease_risk.rs)
- [backend/src/api/recommendation_feed.rs](../../backend/src/api/recommendation_feed.rs)
- [backend/src/api/recommendations.rs](../../backend/src/api/recommendations.rs)
- [backend/src/api/timing.rs](../../backend/src/api/timing.rs)
- [backend/src/db/queries.rs](../../backend/src/db/queries.rs)
- [backend/src/logic/plant_maintenance.rs](../../backend/src/logic/plant_maintenance.rs)
- [backend/src/logic/rules/spring_nitrogen.rs](../../backend/src/logic/rules/spring_nitrogen.rs)
- [backend/src/models/recommendation.rs](../../backend/src/models/recommendation.rs)
- [backend/src/models/recommendation_state.rs](../../backend/src/models/recommendation_state.rs)
- [frontend/src/components/AlertCard.tsx](../../frontend/src/components/AlertCard.tsx)
- [frontend/src/components/dashboard/loadDashboard.ts](../../frontend/src/components/dashboard/loadDashboard.ts)
- [frontend/src/components/environmental/GaugeCard.tsx](../../frontend/src/components/environmental/GaugeCard.tsx)
- [frontend/src/components/sources/SourceStatusLine.tsx](../../frontend/src/components/sources/SourceStatusLine.tsx)
- [frontend/src/components/sources/display.ts](../../frontend/src/components/sources/display.ts)
- [frontend/src/pages/Dashboard.tsx](../../frontend/src/pages/Dashboard.tsx)
- [frontend/src/pages/Recommendations.tsx](../../frontend/src/pages/Recommendations.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `FEED-REC` — Recommendation contract
2. `FEED-PIPE` — Building the feed
3. `FEED-STATE` — Answered state
4. `FEED-API` — Endpoints
5. `FEED-UI` — Recommendations page and dashboard

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| REC | FEED-REC-001 – FEED-REC-005 | 5 | 0 | 0 | 5 | 5 |
| PIPE | FEED-PIPE-001 – FEED-PIPE-018 | 16 | 1 | 0 | 16 | 16 |
| STATE | FEED-STATE-001 – FEED-STATE-010 | 10 | 0 | 0 | 10 | 10 |
| API | FEED-API-001 – FEED-API-006 | 6 | 0 | 0 | 6 | 6 |
| UI | FEED-UI-001 – FEED-UI-015 | 15 | 0 | 0 | 15 | 15 |
| **Total** | | **52** | **1** | **0** | **52** | **52** |

**Summary:** 52 of 52 active specs implemented; 1 deferred; 52 implemented specs cited by a test.

## Key Findings

1. **Deferred:** FEED-PIPE-010.

## Work Required

### Must Fix
- None.

### Should Fix
- None.

### Nice to Have
- None.
