//! Summaries of signed day errors and of the spread between years.

/// Stations with fewer dated years than this say nothing about the spread between years.
pub const MIN_YEARS_FOR_SPREAD: usize = 6;

/// Absolute-error median and 80th percentile, mean signed error, and sample count.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub median: f64,
    pub p80: f64,
    pub bias: f64,
    pub samples: u32,
}

fn sorted(values: &[f64]) -> Vec<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted
}

/// Median: the middle value, or the mean of the two middle values.
pub fn median(values: &[f64]) -> Option<f64> {
    let sorted = sorted(values);
    let middle = sorted.len() / 2;
    match sorted.len() {
        0 => None,
        n if n % 2 == 1 => Some(sorted[middle]),
        _ => Some((sorted[middle - 1] + sorted[middle]) / 2.0),
    }
}

/// Nearest-rank percentile: the value at index `floor(q × n)`, capped at the last.
pub fn percentile(values: &[f64], q: f64) -> Option<f64> {
    let sorted = sorted(values);
    let last = sorted.len().checked_sub(1)?;
    Some(sorted[((q * sorted.len() as f64) as usize).min(last)])
}

/// Summary of signed errors in days (estimate minus measured). `None` when empty.
// @spec WX-EST-SCORE-001
pub fn summarize(signed_errors: &[f64]) -> Option<Summary> {
    let absolute: Vec<f64> = signed_errors.iter().map(|e| e.abs()).collect();
    Some(Summary {
        median: median(&absolute)?,
        p80: percentile(&absolute, 0.8)?,
        bias: signed_errors.iter().sum::<f64>() / signed_errors.len() as f64,
        samples: signed_errors.len() as u32,
    })
}

/// Each year's absolute distance from the station's median date, for a station with at
/// least `MIN_YEARS_FOR_SPREAD` dated years; empty otherwise.
// @spec WX-EST-SCORE-001
pub fn spread(day_of_year: &[f64]) -> Vec<f64> {
    match median(day_of_year) {
        Some(middle) if day_of_year.len() >= MIN_YEARS_FOR_SPREAD => {
            day_of_year.iter().map(|day| (day - middle).abs()).collect()
        }
        _ => Vec::new(),
    }
}
