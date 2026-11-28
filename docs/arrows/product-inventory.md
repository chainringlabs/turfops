# Arrow: product-inventory

Product Inventory — leaf segment `INV` under the root HLD.

## Status

**OK** — audited 2026-11-28 (git SHA `83eb10a`): all 73 active specs implemented, each cited in code and by a test.

## References

### HLD
- [docs/high-level-design.md](../high-level-design.md) § System Design — component "Product inventory"

### LLD
- [docs/intent/product-inventory/product-inventory-design.md](../intent/product-inventory/product-inventory-design.md)

### EARS
- [docs/intent/product-inventory/product-inventory-specs.md](../intent/product-inventory/product-inventory-specs.md) (74 specs, prefix `INV`)

### Related docs
- [docs/agronomy-methods.md](../agronomy-methods.md)
- [docs/plans/product-inventory.md](../plans/product-inventory.md)

### Tests
- [backend/src/api/products/assistant_db_tests.rs](../../backend/src/api/products/assistant_db_tests.rs)
- [backend/src/api/products/checks_tests.rs](../../backend/src/api/products/checks_tests.rs)
- [backend/src/api/products/crud_db_tests.rs](../../backend/src/api/products/crud_db_tests.rs)
- [backend/src/api/products/edit_db_tests.rs](../../backend/src/api/products/edit_db_tests.rs)
- [backend/src/api/products/link_db_tests.rs](../../backend/src/api/products/link_db_tests.rs)
- [backend/src/api/products/missing_category_db_tests.rs](../../backend/src/api/products/missing_category_db_tests.rs)
- [backend/src/api/products/refresh_outage_db_tests.rs](../../backend/src/api/products/refresh_outage_db_tests.rs)
- [backend/src/api/products/unreadable_db_tests.rs](../../backend/src/api/products/unreadable_db_tests.rs)
- [backend/src/datasources/openrouter/product/schema_tests.rs](../../backend/src/datasources/openrouter/product/schema_tests.rs)
- [backend/src/db/product_queries/row_tests.rs](../../backend/src/db/product_queries/row_tests.rs)
- [backend/src/logic/inventory/enrich_tests.rs](../../backend/src/logic/inventory/enrich_tests.rs)
- [backend/src/logic/inventory/shopping/spec_tests.rs](../../backend/src/logic/inventory/shopping/spec_tests.rs)
- [backend/src/models/inventory/need_tests.rs](../../backend/src/models/inventory/need_tests.rs)
- [backend/src/models/product/accepts_tests.rs](../../backend/src/models/product/accepts_tests.rs)
- [backend/src/models/product/vocab_tests.rs](../../backend/src/models/product/vocab_tests.rs)
- [frontend/src/api/productTimeouts.test.ts](../../frontend/src/api/productTimeouts.test.ts)
- [frontend/src/components/InventoryBadge.test.tsx](../../frontend/src/components/InventoryBadge.test.tsx)
- [frontend/src/components/inventory/AddProductForm.test.tsx](../../frontend/src/components/inventory/AddProductForm.test.tsx)
- [frontend/src/components/inventory/LogSuggestions.test.tsx](../../frontend/src/components/inventory/LogSuggestions.test.tsx)
- [frontend/src/components/inventory/ProductDetails.test.tsx](../../frontend/src/components/inventory/ProductDetails.test.tsx)
- [frontend/src/components/inventory/ProductRow.test.tsx](../../frontend/src/components/inventory/ProductRow.test.tsx)
- [frontend/src/components/inventory/ShoppingListPanel.test.tsx](../../frontend/src/components/inventory/ShoppingListPanel.test.tsx)
- [frontend/src/components/inventory/StockBadge.test.tsx](../../frontend/src/components/inventory/StockBadge.test.tsx)
- [frontend/src/components/severity.test.ts](../../frontend/src/components/severity.test.ts)
- [frontend/src/pages/Inventory.test.tsx](../../frontend/src/pages/Inventory.test.tsx)
- [frontend/src/pages/InventoryActions.test.tsx](../../frontend/src/pages/InventoryActions.test.tsx)
- [frontend/src/types/inventoryCompat.test.ts](../../frontend/src/types/inventoryCompat.test.ts)
- [frontend/src/types/inventoryTargets.test.ts](../../frontend/src/types/inventoryTargets.test.ts)
- Inline `#[cfg(test)]` modules citing this prefix: `backend/src/api/recommendation_feed.rs`, `backend/src/datasources/openrouter.rs`, `backend/src/datasources/openrouter/product.rs`, `backend/src/db/product_queries.rs`, `backend/src/error.rs`, `backend/src/logic/inventory.rs`, `backend/src/logic/inventory/shopping.rs`, `backend/src/models/inventory.rs`, `backend/src/models/product.rs`, `backend/src/models/product/checks.rs`, `backend/src/models/product/enums.rs`

### Code
- [backend/src/api/inventory.rs](../../backend/src/api/inventory.rs)
- [backend/src/api/products.rs](../../backend/src/api/products.rs)
- [backend/src/api/products/checks.rs](../../backend/src/api/products/checks.rs)
- [backend/src/api/products/link.rs](../../backend/src/api/products/link.rs)
- [backend/src/api/products/refresh.rs](../../backend/src/api/products/refresh.rs)
- [backend/src/api/recommendation_feed.rs](../../backend/src/api/recommendation_feed.rs)
- [backend/src/datasources/openrouter/product.rs](../../backend/src/datasources/openrouter/product.rs)
- [backend/src/db/product_queries.rs](../../backend/src/db/product_queries.rs)
- [backend/src/error.rs](../../backend/src/error.rs)
- [backend/src/logic/inventory.rs](../../backend/src/logic/inventory.rs)
- [backend/src/logic/inventory/shopping.rs](../../backend/src/logic/inventory/shopping.rs)
- [backend/src/models/inventory.rs](../../backend/src/models/inventory.rs)
- [backend/src/models/product.rs](../../backend/src/models/product.rs)
- [backend/src/models/product/checks.rs](../../backend/src/models/product/checks.rs)
- [backend/src/models/product/enums.rs](../../backend/src/models/product/enums.rs)
- [backend/src/models/product/profile.rs](../../backend/src/models/product/profile.rs)
- [frontend/src/api/client.ts](../../frontend/src/api/client.ts)
- [frontend/src/components/InventoryBadge.tsx](../../frontend/src/components/InventoryBadge.tsx)
- [frontend/src/components/inventory/AddProductForm.tsx](../../frontend/src/components/inventory/AddProductForm.tsx)
- [frontend/src/components/inventory/EditProductForm.tsx](../../frontend/src/components/inventory/EditProductForm.tsx)
- [frontend/src/components/inventory/LogSuggestions.tsx](../../frontend/src/components/inventory/LogSuggestions.tsx)
- [frontend/src/components/inventory/ProductDetails.tsx](../../frontend/src/components/inventory/ProductDetails.tsx)
- [frontend/src/components/inventory/ProductRow.tsx](../../frontend/src/components/inventory/ProductRow.tsx)
- [frontend/src/components/inventory/ShoppingListPanel.tsx](../../frontend/src/components/inventory/ShoppingListPanel.tsx)
- [frontend/src/components/inventory/StockBadge.tsx](../../frontend/src/components/inventory/StockBadge.tsx)
- [frontend/src/components/inventory/useProductActions.ts](../../frontend/src/components/inventory/useProductActions.ts)
- [frontend/src/components/severity.ts](../../frontend/src/components/severity.ts)
- [frontend/src/pages/Applications.tsx](../../frontend/src/pages/Applications.tsx)
- [frontend/src/pages/Inventory.tsx](../../frontend/src/pages/Inventory.tsx)
- [frontend/src/types/inventory.ts](../../frontend/src/types/inventory.ts)
- [frontend/src/types/inventoryCompat.ts](../../frontend/src/types/inventoryCompat.ts)
- [frontend/src/types/inventoryTargets.ts](../../frontend/src/types/inventoryTargets.ts)

## Architecture

**Purpose:** see the LLD's Context and Design Philosophy; this page carries pointers and coverage only.

**Spec facets** (named by their section in the spec file):
1. `INV-SHELF` — The shelf
2. `INV-API` — Product endpoints
3. `INV-LLM` — Assistant profile
4. `INV-RATE` — Label rate
5. `INV-LOG` — Log and shelf
6. `INV-NEED` — Needs and matching
7. `INV-ENRICH` — Inventory status and the feed's inventory pass
8. `INV-SHOP` — Shopping list
9. `INV-UI` — Inventory UI

## Spec Coverage

Derived from the spec file and a `@spec` scan; regenerate, don't hand-edit.

| Facet | Spec IDs | Implemented | Deferred | Gaps | `@spec` in code | `@spec` in tests |
|---|---|---|---|---|---|---|
| SHELF | INV-SHELF-001 – INV-SHELF-012 | 12 | 0 | 0 | 12 | 12 |
| API | INV-API-001 – INV-API-014 | 14 | 0 | 0 | 14 | 14 |
| LLM | INV-LLM-001 – INV-LLM-012 | 12 | 0 | 0 | 12 | 12 |
| RATE | INV-RATE-001 – INV-RATE-002 | 2 | 0 | 0 | 2 | 2 |
| LOG | INV-LOG-001 – INV-LOG-004 | 4 | 0 | 0 | 4 | 4 |
| NEED | INV-NEED-001 – INV-NEED-006 | 5 | 1 | 0 | 5 | 5 |
| ENRICH | INV-ENRICH-001 – INV-ENRICH-005 | 5 | 0 | 0 | 5 | 5 |
| SHOP | INV-SHOP-001 – INV-SHOP-006 | 6 | 0 | 0 | 6 | 6 |
| UI | INV-UI-001 – INV-UI-013 | 13 | 0 | 0 | 13 | 13 |
| **Total** | | **73** | **1** | **0** | **73** | **73** |

**Summary:** 73 of 73 active specs implemented; 1 deferred; 73 implemented specs cited by a test.

## Key Findings

1. **Deferred:** INV-NEED-006.

## Work Required

### Must Fix
- None.

### Should Fix
- None.

### Nice to Have
- None.
