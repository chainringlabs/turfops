//! Keeping the weather record current (SYNC): running the fetches the record says are
//! due and writing what they return.

pub mod fetcher;
pub mod order;
pub mod schedule;

#[cfg(all(test, feature = "db-tests"))]
mod fetcher_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod schedule_db_tests;
