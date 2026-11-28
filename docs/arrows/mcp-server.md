# Arrow: mcp-server

MCP Server — leaf segment `MCP` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `92f4d75`): all 85 active specs implemented, each cited in code and by a test.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "MCP server"

### LLD
- [docs/intent/mcp-server/mcp-server-design.md](../intent/mcp-server/mcp-server-design.md)

### EARS
- [docs/intent/mcp-server/mcp-server-specs.md](../intent/mcp-server/mcp-server-specs.md) (85 specs, prefix `MCP`)

### Related docs
- [docs/agronomy-methods.md](../agronomy-methods.md)
- [docs/plans/ai-assistant.md](../plans/ai-assistant.md)
- [docs/timing-windows.md](../timing-windows.md)

### Tests
- [backend/src/logic/data_sync/db_tests.rs](../../backend/src/logic/data_sync/db_tests.rs)
- [backend/src/mcp/doctrine_tests.rs](../../backend/src/mcp/doctrine_tests.rs)
- [backend/src/mcp/guard_tests.rs](../../backend/src/mcp/guard_tests.rs)
- [backend/src/mcp/mount_tests.rs](../../backend/src/mcp/mount_tests.rs)
- [backend/src/mcp/resource_tests.rs](../../backend/src/mcp/resource_tests.rs)
- [backend/src/mcp/server_tests.rs](../../backend/src/mcp/server_tests.rs)
- [backend/src/mcp/tools/error_tests.rs](../../backend/src/mcp/tools/error_tests.rs)
- [backend/src/mcp/tools/history_db_tests.rs](../../backend/src/mcp/tools/history_db_tests.rs)
- [backend/src/mcp/tools/log_db_tests.rs](../../backend/src/mcp/tools/log_db_tests.rs)
- [backend/src/mcp/tools/season_db_tests.rs](../../backend/src/mcp/tools/season_db_tests.rs)
- [backend/src/mcp/tools/shelf_db_tests.rs](../../backend/src/mcp/tools/shelf_db_tests.rs)
- [backend/src/mcp/tools/snapshot_db_tests.rs](../../backend/src/mcp/tools/snapshot_db_tests.rs)
- [backend/src/mcp/trim_model_tests.rs](../../backend/src/mcp/trim_model_tests.rs)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/mcp/auth.rs`, `backend/src/mcp/config.rs`, `backend/src/mcp/mod.rs`, `backend/src/mcp/resources.rs`, `backend/src/mcp/tools/log.rs`, `backend/src/mcp/tools/mod.rs`, `backend/src/mcp/tools/shelf.rs`, `backend/src/mcp/tools/weather.rs`, `backend/src/mcp/trim.rs`

### Code
- [backend/src/api/health.rs](../../backend/src/api/health.rs)
- [backend/src/db/queries.rs](../../backend/src/db/queries.rs)
- [backend/src/main.rs](../../backend/src/main.rs)
- [backend/src/mcp/auth.rs](../../backend/src/mcp/auth.rs)
- [backend/src/mcp/config.rs](../../backend/src/mcp/config.rs)
- [backend/src/mcp/instructions.rs](../../backend/src/mcp/instructions.rs)
- [backend/src/mcp/mod.rs](../../backend/src/mcp/mod.rs)
- [backend/src/mcp/resources.rs](../../backend/src/mcp/resources.rs)
- [backend/src/mcp/tools/disease.rs](../../backend/src/mcp/tools/disease.rs)
- [backend/src/mcp/tools/lawn.rs](../../backend/src/mcp/tools/lawn.rs)
- [backend/src/mcp/tools/log.rs](../../backend/src/mcp/tools/log.rs)
- [backend/src/mcp/tools/mod.rs](../../backend/src/mcp/tools/mod.rs)
- [backend/src/mcp/tools/season.rs](../../backend/src/mcp/tools/season.rs)
- [backend/src/mcp/tools/shelf.rs](../../backend/src/mcp/tools/shelf.rs)
- [backend/src/mcp/tools/weather.rs](../../backend/src/mcp/tools/weather.rs)
- [backend/src/mcp/trim.rs](../../backend/src/mcp/trim.rs)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `MCP-CFG` — Configuration and mounting
2. `MCP-AUTH` — Bearer guard
3. `MCP-SRV` — Transport and server contract
4. `MCP-TOOL` — Tool catalog
5. `MCP-SNAP` — lawn_snapshot
6. `MCP-HIST` — weather_history
7. `MCP-ERR` — Errors
8. `MCP-DOC` — Doctrine
9. `MCP-RES` — Resources

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| CFG | MCP-CFG-001 – MCP-CFG-006 | 6 | 0 | 0 | 6 | 6 |
| AUTH | MCP-AUTH-001 – MCP-AUTH-003 | 3 | 0 | 0 | 3 | 3 |
| SRV | MCP-SRV-001 – MCP-SRV-007 | 7 | 0 | 0 | 7 | 7 |
| TOOL | MCP-TOOL-001 – MCP-TOOL-029 | 29 | 0 | 0 | 29 | 29 |
| SNAP | MCP-SNAP-001 – MCP-SNAP-010 | 10 | 0 | 0 | 10 | 10 |
| HIST | MCP-HIST-001 – MCP-HIST-008 | 8 | 0 | 0 | 8 | 8 |
| ERR | MCP-ERR-001 – MCP-ERR-007 | 6 | 0 | 0 | 6 | 6 |
| DOC | MCP-DOC-001 – MCP-DOC-012 | 12 | 0 | 0 | 12 | 12 |
| RES | MCP-RES-001 – MCP-RES-004 | 4 | 0 | 0 | 4 | 4 |
| **Total** | | **85** | **0** | **0** | **85** | **85** |

**Summary:** 85 of 85 active specs implemented; 0 deferred; 85 implemented specs cited by a test.

## Key Findings

1. No findings.

## Work Required

### Must Fix
- None.

### Should Fix
- None.

### Nice to Have
- None.
