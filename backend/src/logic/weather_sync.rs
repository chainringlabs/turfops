//! Keeping the weather record current (SYNC): running the fetches the record says are
//! due and writing what they return.

pub mod fetcher;

#[cfg(all(test, feature = "db-tests"))]
mod fetcher_db_tests;
