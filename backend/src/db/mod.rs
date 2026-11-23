// Queries use sqlx's runtime API only: no compile-time-checked macros, no offline data.
// @spec DEPLOY-START-005
pub mod plant_queries;
pub mod pool;
pub mod product_queries;
pub mod queries;
#[allow(dead_code)]
pub mod setup_queries;
#[allow(dead_code)]
pub mod soil_estimate_queries;
pub mod soil_test_queries;
// Used by the fetch schedule and the record's contracts as they land.
#[allow(dead_code)]
pub mod weather_queries;

// @spec DEPLOY-CI-010
#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod setup_db_tests;
