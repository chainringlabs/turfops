# Arrow: deployment

Deployment — leaf segment `DEPLOY` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `92f4d75`): all 77 active specs implemented; 64 cited by a test, 77 cited in code.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Deployment"

### LLD
- [docs/intent/deployment/deployment-design.md](../intent/deployment/deployment-design.md)

### EARS
- [docs/intent/deployment/deployment-specs.md](../intent/deployment/deployment-specs.md) (77 specs, prefix `DEPLOY`)

### Related docs
- [docs/agronomy-methods.md](../agronomy-methods.md)
- [docs/plans/ai-assistant.md](../plans/ai-assistant.md)
- [docs/timing-windows.md](../timing-windows.md)

### Tests
- [backend/src/api/health/db_tests.rs](../../backend/src/api/health/db_tests.rs)
- [backend/src/api/health/probe_tests.rs](../../backend/src/api/health/probe_tests.rs)
- [backend/src/api/health/tests.rs](../../backend/src/api/health/tests.rs)
- [backend/src/api/profile/db_tests.rs](../../backend/src/api/profile/db_tests.rs)
- [backend/src/api/sensor_readings/config_tests.rs](../../backend/src/api/sensor_readings/config_tests.rs)
- [backend/src/api/sources/write_db_tests.rs](../../backend/src/api/sources/write_db_tests.rs)
- [backend/src/clock/frozen_tests.rs](../../backend/src/clock/frozen_tests.rs)
- [backend/src/config/defaults_tests.rs](../../backend/src/config/defaults_tests.rs)
- [backend/src/config/load_tests.rs](../../backend/src/config/load_tests.rs)
- [backend/src/config/location_seed_tests.rs](../../backend/src/config/location_seed_tests.rs)
- [backend/src/db/pool/tests.rs](../../backend/src/db/pool/tests.rs)
- [backend/src/deployment_tests.rs](../../backend/src/deployment_tests.rs)
- [backend/src/deployment_tests/compose.rs](../../backend/src/deployment_tests/compose.rs)
- [backend/src/deployment_tests/image.rs](../../backend/src/deployment_tests/image.rs)
- [backend/src/deployment_tests/shutdown.rs](../../backend/src/deployment_tests/shutdown.rs)
- [backend/src/deployment_tests/sources.rs](../../backend/src/deployment_tests/sources.rs)
- [backend/src/deployment_tests/startup.rs](../../backend/src/deployment_tests/startup.rs)
- [backend/src/deployment_tests/startup_db.rs](../../backend/src/deployment_tests/startup_db.rs)
- [backend/src/error/contract_tests.rs](../../backend/src/error/contract_tests.rs)
- [backend/src/error/log_level_tests.rs](../../backend/src/error/log_level_tests.rs)
- [backend/src/health_probe/tests.rs](../../backend/src/health_probe/tests.rs)
- [backend/src/web/tests.rs](../../backend/src/web/tests.rs)
- [frontend/src/api/client.test.ts](../../frontend/src/api/client.test.ts)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/json_errors.rs`, `backend/src/api/today.rs`, `backend/src/clock.rs`, `backend/src/error.rs`, `backend/src/logging.rs`

### Code
- [.dockerignore](../../.dockerignore)
- [.github/workflows/ci.yml](../../.github/workflows/ci.yml)
- [.github/workflows/docker-publish.yml](../../.github/workflows/docker-publish.yml)
- [.github/workflows/ghcr-retention.yml](../../.github/workflows/ghcr-retention.yml)
- [Dockerfile](../../Dockerfile)
- [backend/deny.toml](../../backend/deny.toml)
- [backend/osv-scanner.toml](../../backend/osv-scanner.toml)
- [backend/src/api/health.rs](../../backend/src/api/health.rs)
- [backend/src/api/json_errors.rs](../../backend/src/api/json_errors.rs)
- [backend/src/api/sensor_readings.rs](../../backend/src/api/sensor_readings.rs)
- [backend/src/api/today.rs](../../backend/src/api/today.rs)
- [backend/src/clock.rs](../../backend/src/clock.rs)
- [backend/src/config.rs](../../backend/src/config.rs)
- [backend/src/config/load.rs](../../backend/src/config/load.rs)
- [backend/src/datasources/openweathermap.rs](../../backend/src/datasources/openweathermap.rs)
- [backend/src/db/mod.rs](../../backend/src/db/mod.rs)
- [backend/src/db/pool.rs](../../backend/src/db/pool.rs)
- [backend/src/error.rs](../../backend/src/error.rs)
- [backend/src/health_probe.rs](../../backend/src/health_probe.rs)
- [backend/src/logging.rs](../../backend/src/logging.rs)
- [backend/src/logic/data_sync.rs](../../backend/src/logic/data_sync.rs)
- [backend/src/logic/weather_sync/fetcher.rs](../../backend/src/logic/weather_sync/fetcher.rs)
- [backend/src/logic/weather_sync/schedule.rs](../../backend/src/logic/weather_sync/schedule.rs)
- [backend/src/main.rs](../../backend/src/main.rs)
- [backend/src/mcp/config.rs](../../backend/src/mcp/config.rs)
- [backend/src/state.rs](../../backend/src/state.rs)
- [backend/src/web.rs](../../backend/src/web.rs)
- [docker-compose.yml](../../docker-compose.yml)
- [renovate.json5](../../renovate.json5)
- [scripts/github-protection.sh](../../scripts/github-protection.sh)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `DEPLOY-IMAGE` — Container image
2. `DEPLOY-COMPOSE` — Compose stack
3. `DEPLOY-CONFIG` — Configuration
4. `DEPLOY-TZ` — Timezone ("today")
5. `DEPLOY-START` — Startup and migrations
6. `DEPLOY-HTTP` — Router, middleware and error contract
7. `DEPLOY-HEALTH` — Health endpoint
8. `DEPLOY-LOG` — Logging
9. `DEPLOY-CI` — CI, release and supply chain

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| IMAGE | DEPLOY-IMAGE-001 – DEPLOY-IMAGE-010 | 10 | 0 | 0 | 10 | 10 |
| COMPOSE | DEPLOY-COMPOSE-001 – DEPLOY-COMPOSE-011 | 9 | 0 | 0 | 9 | 9 |
| CONFIG | DEPLOY-CONFIG-001 – DEPLOY-CONFIG-017 | 15 | 0 | 0 | 15 | 14 |
| TZ | DEPLOY-TZ-001 – DEPLOY-TZ-008 | 6 | 0 | 0 | 6 | 6 |
| START | DEPLOY-START-001 – DEPLOY-START-007 | 7 | 0 | 0 | 7 | 7 |
| HTTP | DEPLOY-HTTP-001 – DEPLOY-HTTP-013 | 11 | 0 | 0 | 11 | 10 |
| HEALTH | DEPLOY-HEALTH-001 – DEPLOY-HEALTH-003 | 3 | 0 | 0 | 3 | 3 |
| LOG | DEPLOY-LOG-001 – DEPLOY-LOG-004 | 4 | 0 | 0 | 4 | 3 |
| CI | DEPLOY-CI-001 – DEPLOY-CI-012 | 12 | 0 | 0 | 12 | 2 |
| **Total** | | **77** | **0** | **0** | **77** | **64** |

**Summary:** 77 of 77 active specs implemented; 0 deferred; 64 implemented specs cited by a test, 13 not.

## Key Findings

1. **Implemented, no test cites it:** DEPLOY-CONFIG-016, DEPLOY-HTTP-013, DEPLOY-LOG-004, DEPLOY-CI-001, DEPLOY-CI-002, DEPLOY-CI-003, DEPLOY-CI-004, DEPLOY-CI-005, DEPLOY-CI-006, DEPLOY-CI-007, DEPLOY-CI-008, DEPLOY-CI-011, DEPLOY-CI-012.

## Work Required

### Must Fix
- None.

### Should Fix
- Add a test citing DEPLOY-CONFIG-016, DEPLOY-HTTP-013, DEPLOY-LOG-004, DEPLOY-CI-001, DEPLOY-CI-002, DEPLOY-CI-003, DEPLOY-CI-004, DEPLOY-CI-005, DEPLOY-CI-006, DEPLOY-CI-007, DEPLOY-CI-008, DEPLOY-CI-011, DEPLOY-CI-012.

### Nice to Have
- None.
