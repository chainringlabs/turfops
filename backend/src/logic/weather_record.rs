//! The weather record's decisions (WX-REC): which source speaks for each input. Storage
//! is `db::weather_queries`; nothing here reads the database or an outside source.

pub mod plan;
