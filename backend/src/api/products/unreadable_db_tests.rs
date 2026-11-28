//! A stored product that cannot be read is skipped from every listing and is a logged
//! server error on its own (INV-SHELF-012, UI-ERR-002).

use super::crud_db_tests::{add, request, status_of};
use super::test_state::{lawn, state};
use super::*;
use crate::error::SERVER_ERROR_MESSAGE;
use crate::state::AppState;
use sqlx::PgPool;

/// Store a profile on `id` that is JSON but not a product profile.
async fn garble_profile(pool: &PgPool, id: i64) {
    sqlx::query(
        "UPDATE products SET profile = '\"garbled\"'::jsonb, profile_model = 'm', \
         profile_generated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .unwrap();
}

async fn names(state: &AppState) -> Vec<String> {
    let q = ListQuery {
        category: None,
        include_archived: true,
    };
    let Json(list) = list_products(State(state.clone()), Query(q)).await.unwrap();
    list.into_iter().map(|p| p.name).collect()
}

// @spec INV-SHELF-012
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_unreadable_product_is_skipped_from_the_list_and_the_shopping_list(pool: PgPool) {
    lawn(&pool).await;
    let state = state(pool.clone(), None).await;
    add(&state, "Milorganite", ProductCategory::Fertilizer).await;
    let bad = add(&state, "Mystery bag", ProductCategory::Fertilizer).await;
    garble_profile(&pool, bad.id.unwrap()).await;

    assert_eq!(names(&state).await, ["Milorganite"]);
    assert!(
        crate::api::inventory::get_shopping_list(State(state.clone()))
            .await
            .is_ok()
    );
}

// @spec INV-SHELF-012, UI-ERR-002, UI-ERR-003
#[sqlx::test(migrations = "src/db/migrations")]
async fn reading_an_unreadable_product_is_a_server_error_without_its_detail(pool: PgPool) {
    lawn(&pool).await;
    let state = state(pool.clone(), None).await;
    let bad = add(&state, "Mystery bag", ProductCategory::Fertilizer).await;
    garble_profile(&pool, bad.id.unwrap()).await;

    let err = get_product(State(state), Path(bad.id.unwrap()))
        .await
        .unwrap_err();
    assert_eq!(err.owner_message(), SERVER_ERROR_MESSAGE);
    assert_eq!(status_of(err), StatusCode::INTERNAL_SERVER_ERROR);
}

// @spec INV-SHELF-012, INV-API-013
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_unreadable_products_name_is_still_taken(pool: PgPool) {
    lawn(&pool).await;
    let state = state(pool.clone(), None).await;
    let bad = add(&state, "Mystery bag", ProductCategory::Fertilizer).await;
    garble_profile(&pool, bad.id.unwrap()).await;

    let err = create_product(
        State(state),
        Json(request(" mystery BAG ", Some(ProductCategory::Seed))),
    )
    .await
    .unwrap_err();
    assert_eq!(status_of(err), StatusCode::CONFLICT);
}
