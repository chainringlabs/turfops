//! The modeled soil-temperature series (WX-EST): a regression of soil on gridded air
//! with a day-of-year term, and the scored accuracy that says which crossings it may date.

pub mod model;
pub mod scores;
pub mod scoring;

#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod scores_tests;
