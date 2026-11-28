# Arrow: timing-windows

Timing Windows — leaf segment `TIME` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `1b12d9d`): all 80 active specs implemented, each cited in code and by a test.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Timing windows"

### LLD
- [docs/intent/timing-windows/timing-windows-design.md](../intent/timing-windows/timing-windows-design.md)

### EARS
- [docs/intent/timing-windows/timing-windows-specs.md](../intent/timing-windows/timing-windows-specs.md) (80 specs, prefix `TIME`)

### Related docs
- [docs/agronomy-methods.md](../agronomy-methods.md)
- [docs/timing-windows.md](../timing-windows.md)

### Tests
- [backend/src/api/timing/handler_tests.rs](../../backend/src/api/timing/handler_tests.rs)
- [backend/src/api/timing/hook_tests.rs](../../backend/src/api/timing/hook_tests.rs)
- [backend/src/api/timing/memo_tests.rs](../../backend/src/api/timing/memo_tests.rs)
- [backend/src/logic/timing/plan/spec_tests.rs](../../backend/src/logic/timing/plan/spec_tests.rs)
- [backend/src/logic/timing/plan/tests.rs](../../backend/src/logic/timing/plan/tests.rs)
- [backend/src/logic/timing/tests.rs](../../backend/src/logic/timing/tests.rs)
- [backend/src/logic/timing/tests/confirmation.rs](../../backend/src/logic/timing/tests/confirmation.rs)
- [backend/src/logic/timing/tests/definitions.rs](../../backend/src/logic/timing/tests/definitions.rs)
- [backend/src/logic/timing/tests/estimated.rs](../../backend/src/logic/timing/tests/estimated.rs)
- [backend/src/logic/timing/tests/feed.rs](../../backend/src/logic/timing/tests/feed.rs)
- [backend/src/logic/timing/tests/history.rs](../../backend/src/logic/timing/tests/history.rs)
- [backend/src/logic/timing/tests/log_spans.rs](../../backend/src/logic/timing/tests/log_spans.rs)
- [backend/src/logic/timing/tests/outlook.rs](../../backend/src/logic/timing/tests/outlook.rs)
- [backend/src/logic/timing/tests/resolve.rs](../../backend/src/logic/timing/tests/resolve.rs)
- [backend/src/logic/timing/tests/season_context.rs](../../backend/src/logic/timing/tests/season_context.rs)
- [backend/src/logic/timing/tests/states.rs](../../backend/src/logic/timing/tests/states.rs)
- [frontend/src/components/timing/SoilSeasonChart.test.tsx](../../frontend/src/components/timing/SoilSeasonChart.test.tsx)
- [frontend/src/components/timing/StateBadge.test.tsx](../../frontend/src/components/timing/StateBadge.test.tsx)
- [frontend/src/components/timing/TimingWidget.test.tsx](../../frontend/src/components/timing/TimingWidget.test.tsx)
- [frontend/src/components/timing/WindowCard.test.tsx](../../frontend/src/components/timing/WindowCard.test.tsx)
- [frontend/src/components/timing/WindowTimeline.test.tsx](../../frontend/src/components/timing/WindowTimeline.test.tsx)
- [frontend/src/pages/Timing.test.tsx](../../frontend/src/pages/Timing.test.tsx)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/timing.rs`, `backend/src/logic/timing/climatology.rs`, `backend/src/logic/timing/companions.rs`, `backend/src/logic/timing/context.rs`, `backend/src/logic/timing/evaluate.rs`, `backend/src/logic/timing/log.rs`, `backend/src/logic/timing/recommendations.rs`, `backend/src/logic/timing/series.rs`, `backend/src/logic/timing/text.rs`, `backend/src/logic/timing/windows.rs`

### Code
- [backend/src/api/timing.rs](../../backend/src/api/timing.rs)
- [backend/src/api/timing/memo.rs](../../backend/src/api/timing/memo.rs)
- [backend/src/logic/timing/climatology.rs](../../backend/src/logic/timing/climatology.rs)
- [backend/src/logic/timing/companions.rs](../../backend/src/logic/timing/companions.rs)
- [backend/src/logic/timing/context.rs](../../backend/src/logic/timing/context.rs)
- [backend/src/logic/timing/evaluate.rs](../../backend/src/logic/timing/evaluate.rs)
- [backend/src/logic/timing/log.rs](../../backend/src/logic/timing/log.rs)
- [backend/src/logic/timing/mod.rs](../../backend/src/logic/timing/mod.rs)
- [backend/src/logic/timing/plan.rs](../../backend/src/logic/timing/plan.rs)
- [backend/src/logic/timing/recommendations.rs](../../backend/src/logic/timing/recommendations.rs)
- [backend/src/logic/timing/series.rs](../../backend/src/logic/timing/series.rs)
- [backend/src/logic/timing/text.rs](../../backend/src/logic/timing/text.rs)
- [backend/src/logic/timing/windows.rs](../../backend/src/logic/timing/windows.rs)
- [backend/src/logic/weather_record/daily.rs](../../backend/src/logic/weather_record/daily.rs)
- [frontend/src/components/timing/SoilSeasonChart.tsx](../../frontend/src/components/timing/SoilSeasonChart.tsx)
- [frontend/src/components/timing/SoilSeasonTable.tsx](../../frontend/src/components/timing/SoilSeasonTable.tsx)
- [frontend/src/components/timing/StateBadge.tsx](../../frontend/src/components/timing/StateBadge.tsx)
- [frontend/src/components/timing/TimingWidget.tsx](../../frontend/src/components/timing/TimingWidget.tsx)
- [frontend/src/components/timing/WindowCard.tsx](../../frontend/src/components/timing/WindowCard.tsx)
- [frontend/src/components/timing/WindowTimeline.tsx](../../frontend/src/components/timing/WindowTimeline.tsx)
- [frontend/src/pages/Timing.tsx](../../frontend/src/pages/Timing.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `TIME-ENG` — Window engine
2. `TIME-LOG` — Seed vs. pre-emergent exclusivity
3. `TIME-FEED` — Timing recommendations and companions
4. `TIME-PLAN` — Seasonal-plan activities
5. `TIME-UI` — Timing page and dashboard widget

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| ENG | TIME-ENG-001 – TIME-ENG-052 | 52 | 0 | 0 | 52 | 52 |
| LOG | TIME-LOG-001 – TIME-LOG-004 | 4 | 0 | 0 | 4 | 4 |
| FEED | TIME-FEED-001 – TIME-FEED-009 | 9 | 0 | 0 | 9 | 9 |
| PLAN | TIME-PLAN-001 – TIME-PLAN-007 | 7 | 0 | 0 | 7 | 7 |
| UI | TIME-UI-001 – TIME-UI-008 | 8 | 0 | 0 | 8 | 8 |
| **Total** | | **80** | **0** | **0** | **80** | **80** |

**Summary:** 80 of 80 active specs implemented; 0 deferred; 80 implemented specs cited by a test.

## Key Findings

1. No findings.

## Work Required

### Must Fix
- None.

### Should Fix
- None.

### Nice to Have
- None.
