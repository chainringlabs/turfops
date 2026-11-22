//! The weather record's decisions (WX-REC): which source speaks for each input. Storage
//! is `db::weather_queries`; nothing here reads the database or an outside source.

pub mod daily;
pub mod plan;
mod readings;
pub mod record;
pub mod resolve;

#[cfg(all(test, feature = "db-tests"))]
mod daily_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod history_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod lab;
