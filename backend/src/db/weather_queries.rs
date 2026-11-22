//! Storage for the weather record (WX-REC): what was fetched, kept one row per source.
//! Nothing here resolves a value for the lawn or talks to an outside source.

pub mod catalog;
pub mod days;
pub mod disease_days;
pub mod fetch_state;
pub mod plan;
pub mod rows;
pub mod sensors;
pub mod sources;

#[cfg(all(test, feature = "db-tests"))]
mod days_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod fetch_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod plan_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod rows_db_tests;
