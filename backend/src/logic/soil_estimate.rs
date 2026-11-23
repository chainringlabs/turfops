//! The modeled soil-temperature series (WX-EST): a regression of soil on gridded air
//! with a day-of-year term, and the scored accuracy that says which crossings it may date.

pub mod anchor;
pub mod calibration;
pub mod estimator;
pub mod fit;
pub mod model;
pub mod scores;
pub mod scoring;
mod series;

#[cfg(all(test, feature = "db-tests"))]
mod estimator_db_tests;
#[cfg(all(test, feature = "db-tests"))]
mod fit_db_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod scores_tests;
