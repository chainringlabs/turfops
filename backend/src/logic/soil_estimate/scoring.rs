//! The scoring tool (WX-EST-SCORE): fits each USCRN station's model on its neighbours and
//! on itself, dates every crossing a consumer dates by that consumer's own rule, and
//! writes the data file the application embeds. Everything here is pure; the command that
//! downloads the public data and runs it is behind the `scoring` feature.

pub mod crossings;
pub mod inputs;
#[cfg(feature = "scoring")]
pub mod run;
pub mod score;
pub mod stats;

#[cfg(test)]
mod crossings_tests;
#[cfg(test)]
mod inputs_tests;
#[cfg(test)]
mod score_tests;
#[cfg(test)]
mod stats_tests;
