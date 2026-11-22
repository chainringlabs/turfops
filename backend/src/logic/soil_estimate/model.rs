//! The regression: trailing means of daily air, a day-of-year sine and cosine, a constant.

use chrono::{Datelike, Duration, NaiveDate};
use std::collections::BTreeMap;

/// Lengths, in days, of the trailing air means the model reads.
pub const WINDOWS: [usize; 5] = [1, 3, 7, 14, 30];
/// Five trailing means, sine, cosine, constant.
pub const TERMS: usize = 8;
/// Share of a trailing window's days that must have a value.
pub const MIN_COVERAGE: f64 = 0.8;
/// A column whose remaining length is under this share of the largest is dependent.
const RANK_TOLERANCE: f64 = 1e-10;

/// One day's model inputs, in `WINDOWS` order, then sine, cosine and 1.0.
pub type Features = [f64; TERMS];
/// Daily air temperature (°F): half the sum of the grid's maximum and minimum.
pub type DailyAir = BTreeMap<NaiveDate, f64>;

/// A fitted model for one depth.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub coefficients: Features,
}

/// The model inputs for `day`, or `None` when any trailing mean lacks coverage.
// @spec WX-EST-MODEL-001, WX-EST-MODEL-002
pub fn features(air: &DailyAir, day: NaiveDate) -> Option<Features> {
    let mut inputs = [0.0; TERMS];
    for (slot, &window) in inputs.iter_mut().zip(&WINDOWS) {
        *slot = trailing_mean(air, day, window)?;
    }
    let angle = std::f64::consts::TAU * f64::from(day.ordinal()) / 365.25;
    inputs[WINDOWS.len()] = angle.sin();
    inputs[WINDOWS.len() + 1] = angle.cos();
    inputs[WINDOWS.len() + 2] = 1.0;
    Some(inputs)
}

/// Mean of the air on the `window` days ending on `day`, when enough of them have a value.
fn trailing_mean(air: &DailyAir, day: NaiveDate, window: usize) -> Option<f64> {
    let first = day - Duration::days(window as i64 - 1);
    let (sum, count) = air
        .range(first..=day)
        .fold((0.0, 0usize), |(sum, count), (_, value)| {
            (sum + value, count + 1)
        });
    let needed = (MIN_COVERAGE * window as f64).ceil() as usize;
    (count >= needed.max(1)).then(|| sum / count as f64)
}

/// Ordinary least squares over `rows`, by Householder QR so that the strongly correlated
/// trailing means do not cost precision. `None` with too few rows or no unique solution.
// @spec WX-EST-MODEL-001
pub fn fit(rows: &[(Features, f64)]) -> Option<Model> {
    if rows.len() < TERMS {
        return None;
    }
    let mut matrix: Vec<Features> = rows.iter().map(|(inputs, _)| *inputs).collect();
    let mut target: Vec<f64> = rows.iter().map(|(_, soil)| *soil).collect();
    let scale = (0..TERMS)
        .map(|column| column_norm(&matrix, column, 0))
        .fold(0.0, f64::max);
    for column in 0..TERMS {
        reflect(&mut matrix, &mut target, column, scale * RANK_TOLERANCE)?;
    }
    Some(Model {
        coefficients: back_substitute(&matrix, &target),
    })
}

fn column_norm(matrix: &[Features], column: usize, from_row: usize) -> f64 {
    matrix[from_row..]
        .iter()
        .map(|row| row[column] * row[column])
        .sum::<f64>()
        .sqrt()
}

/// One Householder step: zero `column` below the diagonal, applying the same reflection
/// to the remaining columns and the target. `None` when the column is dependent.
fn reflect(
    matrix: &mut [Features],
    target: &mut [f64],
    column: usize,
    tolerance: f64,
) -> Option<()> {
    let norm = column_norm(matrix, column, column);
    if norm <= tolerance {
        return None;
    }
    let diagonal = matrix[column][column];
    let alpha = if diagonal > 0.0 { -norm } else { norm };
    let mut v: Vec<f64> = matrix[column..].iter().map(|row| row[column]).collect();
    v[0] -= alpha;
    let v_squared: f64 = v.iter().map(|x| x * x).sum();
    for other in column + 1..TERMS {
        let dot: f64 = v
            .iter()
            .zip(&matrix[column..])
            .map(|(x, row)| x * row[other])
            .sum();
        let factor = 2.0 * dot / v_squared;
        for (x, row) in v.iter().zip(&mut matrix[column..]) {
            row[other] -= factor * x;
        }
    }
    let dot: f64 = v.iter().zip(&target[column..]).map(|(x, y)| x * y).sum();
    let factor = 2.0 * dot / v_squared;
    for (x, y) in v.iter().zip(&mut target[column..]) {
        *y -= factor * x;
    }
    matrix[column][column] = alpha;
    Some(())
}

/// Solves the upper-triangular system the reflections left in the first `TERMS` rows.
fn back_substitute(matrix: &[Features], target: &[f64]) -> Features {
    let mut solution = [0.0; TERMS];
    for row in (0..TERMS).rev() {
        let known: f64 = (row + 1..TERMS)
            .map(|column| matrix[row][column] * solution[column])
            .sum();
        solution[row] = (target[row] - known) / matrix[row][row];
    }
    solution
}

/// One training row per day from `from` on that has both a soil value and usable inputs.
// @spec WX-EST-CAL-004
pub fn training_rows(
    air: &DailyAir,
    soil: &BTreeMap<NaiveDate, f64>,
    from: NaiveDate,
) -> Vec<(Features, f64)> {
    soil.range(from..)
        .filter_map(|(&day, &value)| Some((features(air, day)?, value)))
        .collect()
}

impl Model {
    /// The estimate for one day, °F rounded to 0.1.
    // @spec WX-EST-MODEL-003
    pub fn estimate(&self, inputs: &Features) -> f64 {
        let value: f64 = self
            .coefficients
            .iter()
            .zip(inputs)
            .map(|(coefficient, input)| coefficient * input)
            .sum();
        (value * 10.0).round() / 10.0
    }

    /// The estimate for every day of `air` whose inputs are usable, ascending.
    // @spec WX-EST-MODEL-003, WX-EST-FWD-001
    pub fn series(&self, air: &DailyAir) -> Vec<(NaiveDate, f64)> {
        air.keys()
            .filter_map(|&day| Some((day, self.estimate(&features(air, day)?))))
            .collect()
    }
}
