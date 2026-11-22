//! The weather record's decisions (WX-REC): which source speaks for each input. Storage
//! is `db::weather_queries`; nothing here reads the database or an outside source.

pub mod daily;
pub mod disease;
pub mod hourly;
pub mod live;
pub mod plan;
mod readings;
pub mod record;
pub mod resolve;
pub mod summary;

#[cfg(all(test, feature = "db-tests"))]
mod daily_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod disease_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod history_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod hourly_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod lab;
#[cfg(all(test, feature = "db-tests"))]
mod live_db_tests;
