# Arrow: applications

Applications — leaf segment `APP` under the root HLD.

## Status

**PARTIAL** — audited 2026-11-28 (git SHA `1b12d9d`): 81 of 82 active specs implemented; 1 marked `[ ]`.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Applications"

### LLD
- [docs/intent/applications/applications-design.md](../intent/applications/applications-design.md)

### EARS
- [docs/intent/applications/applications-specs.md](../intent/applications/applications-specs.md) (82 specs, prefix `APP`)

### Related docs
- [docs/agronomy-methods.md](../agronomy-methods.md)
- [docs/plans/product-inventory.md](../plans/product-inventory.md)

### Tests
- [backend/src/api/applications/duplicate_db_tests.rs](../../backend/src/api/applications/duplicate_db_tests.rs)
- [backend/src/api/applications/link_db_tests.rs](../../backend/src/api/applications/link_db_tests.rs)
- [backend/src/api/applications/list_db_tests.rs](../../backend/src/api/applications/list_db_tests.rs)
- [backend/src/api/applications/request_tests.rs](../../backend/src/api/applications/request_tests.rs)
- [backend/src/api/applications/write_db_tests.rs](../../backend/src/api/applications/write_db_tests.rs)
- [backend/src/api/profile/db_tests.rs](../../backend/src/api/profile/db_tests.rs)
- [backend/src/api/profile/route_tests.rs](../../backend/src/api/profile/route_tests.rs)
- [backend/src/config/location_seed_tests.rs](../../backend/src/config/location_seed_tests.rs)
- [backend/src/config/startup_profile_db_tests.rs](../../backend/src/config/startup_profile_db_tests.rs)
- [backend/src/db/db_tests.rs](../../backend/src/db/db_tests.rs)
- [backend/src/db/queries/row_tests.rs](../../backend/src/db/queries/row_tests.rs)
- [backend/src/logic/follow_up/reminder_tests.rs](../../backend/src/logic/follow_up/reminder_tests.rs)
- [backend/src/models/application/log_tests.rs](../../backend/src/models/application/log_tests.rs)
- [backend/src/models/lawn_profile/location_tests.rs](../../backend/src/models/lawn_profile/location_tests.rs)
- [backend/src/models/lawn_profile/parse_tests.rs](../../backend/src/models/lawn_profile/parse_tests.rs)
- [frontend/src/api/client.test.ts](../../frontend/src/api/client.test.ts)
- [frontend/src/components/FracClassPicker.test.tsx](../../frontend/src/components/FracClassPicker.test.tsx)
- [frontend/src/components/ProductPicker.test.tsx](../../frontend/src/components/ProductPicker.test.tsx)
- [frontend/src/components/applications/formRules.test.ts](../../frontend/src/components/applications/formRules.test.ts)
- [frontend/src/pages/ApplicationForm.test.tsx](../../frontend/src/pages/ApplicationForm.test.tsx)
- [frontend/src/pages/Applications.test.tsx](../../frontend/src/pages/Applications.test.tsx)
- [frontend/src/pages/Settings.test.tsx](../../frontend/src/pages/Settings.test.tsx)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/applications/product_link.rs`, `backend/src/api/applications/validate.rs`, `backend/src/config.rs`, `backend/src/error.rs`, `backend/src/logic/follow_up.rs`, `backend/src/models/application.rs`, `backend/src/models/lawn_profile.rs`

### Code
- [backend/src/api/applications.rs](../../backend/src/api/applications.rs)
- [backend/src/api/applications/product_link.rs](../../backend/src/api/applications/product_link.rs)
- [backend/src/api/applications/validate.rs](../../backend/src/api/applications/validate.rs)
- [backend/src/api/profile.rs](../../backend/src/api/profile.rs)
- [backend/src/config.rs](../../backend/src/config.rs)
- [backend/src/config/location_seed.rs](../../backend/src/config/location_seed.rs)
- [backend/src/db/migrations/20261011000100_applications_cleanup.sql](../../backend/src/db/migrations/20261011000100_applications_cleanup.sql)
- [backend/src/db/plant_queries.rs](../../backend/src/db/plant_queries.rs)
- [backend/src/db/product_queries.rs](../../backend/src/db/product_queries.rs)
- [backend/src/db/queries.rs](../../backend/src/db/queries.rs)
- [backend/src/error.rs](../../backend/src/error.rs)
- [backend/src/logic/follow_up.rs](../../backend/src/logic/follow_up.rs)
- [backend/src/main.rs](../../backend/src/main.rs)
- [backend/src/models/application.rs](../../backend/src/models/application.rs)
- [backend/src/models/lawn_profile.rs](../../backend/src/models/lawn_profile.rs)
- [backend/src/models/lawn_profile/location.rs](../../backend/src/models/lawn_profile/location.rs)
- [frontend/src/api/client.ts](../../frontend/src/api/client.ts)
- [frontend/src/components/FracClassPicker.tsx](../../frontend/src/components/FracClassPicker.tsx)
- [frontend/src/components/ProductPicker.tsx](../../frontend/src/components/ProductPicker.tsx)
- [frontend/src/components/applications/formRules.ts](../../frontend/src/components/applications/formRules.ts)
- [frontend/src/components/profile/LawnProfileForm.tsx](../../frontend/src/components/profile/LawnProfileForm.tsx)
- [frontend/src/pages/Applications.tsx](../../frontend/src/pages/Applications.tsx)
- [frontend/src/pages/Settings.tsx](../../frontend/src/pages/Settings.tsx)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `APP-PROF` — Lawn profile
2. `APP-LOG` — Application log model
3. `APP-DUP` — Duplicate key
4. `APP-API` — Applications endpoints
5. `APP-LINK` — Shelf-product link
6. `APP-FRAC` — FRAC classes on the entry
7. `APP-FOLLOW` — Follow-up reminders
8. `APP-LIST` — Applications page list
9. `APP-FORM` — Application form
10. `APP-SET` — Settings page

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| PROF | APP-PROF-001 – APP-PROF-019 | 18 | 0 | 1 | 17 | 17 |
| LOG | APP-LOG-001 – APP-LOG-012 | 12 | 0 | 0 | 12 | 12 |
| DUP | APP-DUP-001 – APP-DUP-003 | 3 | 0 | 0 | 3 | 3 |
| API | APP-API-001 – APP-API-012 | 12 | 0 | 0 | 12 | 12 |
| LINK | APP-LINK-001 – APP-LINK-004 | 4 | 0 | 0 | 4 | 4 |
| FRAC | APP-FRAC-001 – APP-FRAC-005 | 5 | 0 | 0 | 5 | 5 |
| FOLLOW | APP-FOLLOW-001 – APP-FOLLOW-008 | 8 | 0 | 0 | 8 | 8 |
| LIST | APP-LIST-001 – APP-LIST-005 | 5 | 0 | 0 | 5 | 5 |
| FORM | APP-FORM-001 – APP-FORM-011 | 11 | 0 | 0 | 11 | 11 |
| SET | APP-SET-001 – APP-SET-003 | 3 | 0 | 0 | 3 | 3 |
| **Total** | | **81** | **0** | **1** | **80** | **80** |

**Summary:** 81 of 82 active specs implemented; 0 deferred; 80 implemented specs cited by a test, 1 not.

## Key Findings

1. **Open gaps (not yet built):** APP-PROF-018.
2. **Implemented, no `@spec` in code:** APP-PROF-019.
3. **Implemented, no test cites it:** APP-PROF-019.

## Work Required

### Must Fix
- None.

### Should Fix
- Build or defer APP-PROF-018.
- Add a test citing APP-PROF-019.
- Annotate the code behind APP-PROF-019.

### Nice to Have
- None.
