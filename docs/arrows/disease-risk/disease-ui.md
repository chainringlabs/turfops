# Arrow: disease-ui

Disease UI — leaf segment `DIS-UI` under the `disease-risk` sub-HLD.

## Status

**OK** — audited 2026-11-27 (git SHA `aa1cae8`): all 19 active specs implemented, each cited in code and by a test.

## References

### HLD
- [docs/high-level-design.md](../../high-level-design.md) § System Design — component "Disease risk"
- [docs/intent/disease-risk/disease-risk-design.md](../../intent/disease-risk/disease-risk-design.md) (parent sub-HLD)

### LLD
- [docs/intent/disease-risk/disease-ui/disease-ui-design.md](../../intent/disease-risk/disease-ui/disease-ui-design.md)

### EARS
- [docs/intent/disease-risk/disease-ui/disease-ui-specs.md](../../intent/disease-risk/disease-ui/disease-ui-specs.md) (19 specs, prefix `DIS-UI`)

### Tests
- [frontend/src/components/disease/DailyRiskChart.test.tsx](../../../frontend/src/components/disease/DailyRiskChart.test.tsx)
- [frontend/src/components/disease/DiseaseDetail.test.tsx](../../../frontend/src/components/disease/DiseaseDetail.test.tsx)
- [frontend/src/components/disease/DiseaseRiskWidget.test.tsx](../../../frontend/src/components/disease/DiseaseRiskWidget.test.tsx)
- [frontend/src/components/disease/ManagementPanel.test.tsx](../../../frontend/src/components/disease/ManagementPanel.test.tsx)
- [frontend/src/components/disease/RiskMeter.test.tsx](../../../frontend/src/components/disease/RiskMeter.test.tsx)
- [frontend/src/components/disease/TierBadge.test.tsx](../../../frontend/src/components/disease/TierBadge.test.tsx)
- [frontend/src/pages/DiseaseRisk.test.tsx](../../../frontend/src/pages/DiseaseRisk.test.tsx)

### Code
- [frontend/src/components/disease/DailyRiskChart.tsx](../../../frontend/src/components/disease/DailyRiskChart.tsx)
- [frontend/src/components/disease/DiseaseDetail.tsx](../../../frontend/src/components/disease/DiseaseDetail.tsx)
- [frontend/src/components/disease/DiseaseRiskWidget.tsx](../../../frontend/src/components/disease/DiseaseRiskWidget.tsx)
- [frontend/src/components/disease/ManagementPanel.tsx](../../../frontend/src/components/disease/ManagementPanel.tsx)
- [frontend/src/components/disease/RiskMeter.tsx](../../../frontend/src/components/disease/RiskMeter.tsx)
- [frontend/src/components/disease/TierBadge.tsx](../../../frontend/src/components/disease/TierBadge.tsx)
- [frontend/src/pages/DiseaseRisk.tsx](../../../frontend/src/pages/DiseaseRisk.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `DIS-UI-PAGE` — Pages
2. `DIS-UI-DETAIL` — Detail and management
3. `DIS-UI-CHART` — Daily chart and meter
4. `DIS-UI-WIDGET` — Dashboard widget
5. `DIS-UI-CONV` — Conventions

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| PAGE | DIS-UI-PAGE-001 – DIS-UI-PAGE-005 | 5 | 0 | 0 | 5 | 5 |
| DETAIL | DIS-UI-DETAIL-001 – DIS-UI-DETAIL-005 | 5 | 0 | 0 | 5 | 5 |
| CHART | DIS-UI-CHART-001 – DIS-UI-CHART-005 | 5 | 0 | 0 | 5 | 5 |
| WIDGET | DIS-UI-WIDGET-001 | 1 | 0 | 0 | 1 | 1 |
| CONV | DIS-UI-CONV-001 – DIS-UI-CONV-003 | 3 | 0 | 0 | 3 | 3 |
| **Total** | | **19** | **0** | **0** | **19** | **19** |

**Summary:** 19 of 19 active specs implemented; 0 deferred; 19 implemented specs cited by a test.

## Key Findings

1. No findings.

## Work Required

### Must Fix
- None.

### Should Fix
- None.

### Nice to Have
- None.
