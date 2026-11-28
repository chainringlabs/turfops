# Soil Estimate — EARS Specs

Design: [soil-estimate-design.md](soil-estimate-design.md). Parent: [weather-data](../weather-data-design.md). Prefix `WX-EST`. Facets: `MODEL` (the regression and its series), `CAL` (calibration), `ANCHOR` (the anchored series), `FWD` (the forward series), `DATE` (which crossings may be dated), `SCORE` (the scoring tool and its data file), `FAIL` (degradation).

"Estimate" is the modeled daily soil temperature at 5 cm or 10 cm for the lawn's location. "Daily air" is half the sum of the grid's daily maximum and minimum at a point. "Basis station" is a station that the weather record's plan holds for soil temperature at that depth. "Measured series" is the lawn's soil temperature from a sensor or a station.

## The regression and its series

- [x] **WX-EST-MODEL-001**: The system shall estimate daily soil temperature at 5 cm and at 10 cm with one ordinary-least-squares model per depth, whose terms are the means of daily air over the 1, 3, 7, 14 and 30 days ending on the estimated day, the sine and the cosine of the day of year over a 365.25-day period, and a constant.
- [x] **WX-EST-MODEL-002**: A trailing mean of daily air shall be usable when at least 80 % of its days have a value, and a day on which any of the five trailing means is not usable shall have no estimate.
- [x] **WX-EST-MODEL-003**: The system shall write the estimate to the weather record as a daily source named "Soil temperature estimate", one value per depth per day in °F rounded to 0.1, for every day on which the record holds the daily air the model needs at the point of WX-EST-MODEL-006.
- [x] **WX-EST-MODEL-004**: When the weather record's daily air at the point the estimate is computed from changes for a day, the system shall recompute the estimate for that day and the 30 days after it; recomputing the whole series satisfies this.
- [x] **WX-EST-MODEL-006**: The system shall compute the estimate from the grid's daily air at the lawn's point while the model is fitted on the five nearest stations, and from the grid's daily air at the basis station's point while the model is fitted on that station alone.
- [x] **WX-EST-MODEL-005**: The system shall produce no hourly estimate.

## Calibration

- [x] **WX-EST-CAL-001**: While no basis station with at least 1,000 days of soil temperature at a depth exists, the system shall fit that depth's model on the pooled data of the five nearest USCRN stations that each have at least 1,000 days of soil temperature at that depth.
- [x] **WX-EST-CAL-002**: While a basis station, USCRN or SCAN, has at least 1,000 days of soil temperature at a depth, the system shall fit that depth's model on that station's data alone.
- [x] **WX-EST-CAL-003**: The system shall pair each calibration station's daily soil temperature with the grid's daily air at that station's own point, and shall not use the station's own air measurements in the fit.
- [x] **WX-EST-CAL-004**: The system shall fit on every day from 2012 on for which a calibration station has both a soil value and usable trailing means.
- [x] **WX-EST-CAL-005**: The system shall refit each model monthly and whenever its calibration set changes, and after a refit shall recompute the whole estimate series.
- [x] **WX-EST-CAL-006**: The system shall store, with each fit, the fit date, the stations used with their distances from the lawn, the number of training days and the root-mean-square daily error of the fit.
- [x] **WX-EST-CAL-008**: The system shall choose the five-nearest calibration set from the catalog's stations in order of distance among those the catalog shows operating for at least three years, and when a chosen station proves to hold fewer than 1,000 days of soil temperature at the depth, shall replace it with the next nearest.
- [x] **WX-EST-CAL-007**: The system shall ask the weather record to hold, for each calibration station, its daily soil temperature and the grid's daily air at its point from 2012 on; for a basis station fitted on alone, the grid's daily air at its point only; and it shall keep the daily soil temperature of a station it passed over for holding fewer than 1,000 days while that station is nearer than the farthest one chosen.

## The anchored series

- [x] **WX-EST-ANCHOR-001**: The offset at an edge of a gap in a measured series shall be the mean of the measured value minus the estimate over the measured days among the 7 days nearest that edge on its measured side; where an edge has no such day with an estimate, the gap shall take the other edge's offset throughout.
- [x] **WX-EST-ANCHOR-002**: For a gap with measured days on both sides, the anchored value on each day shall be the estimate plus an offset interpolated linearly between the offsets at the two edges.
- [x] **WX-EST-ANCHOR-003**: For a gap with measured days before it and none after, the anchored value on each day shall be the estimate plus the offset at the gap's start reduced linearly to zero over the 30 days after that start, and the estimate alone thereafter.
- [x] **WX-EST-ANCHOR-004**: For days before the first value of a measured series, the anchored value shall be the estimate without an offset.

## The forward series

- [x] **WX-EST-FWD-001**: When given a daily air series that continues past the days the weather record holds, the system shall return the estimate at the requested depth for each later day whose trailing means are usable, using the fitted model unchanged.
- [x] **WX-EST-FWD-002**: Where the forward series follows measured days, the system shall apply the offset of WX-EST-ANCHOR-003, taking the last measured day as the gap's start.

## Which crossings may be dated

- [x] **WX-EST-DATE-001**: `datable(depth, threshold, direction)` shall answer yes only when the scoring data file holds a score for that depth, threshold and direction under the calibration the lawn's model uses (five nearest stations, or the basis station alone) and that score's median error is smaller than the same crossing's median spread between years.
- [x] **WX-EST-DATE-002**: `datable` shall return, with a yes, the scored median and 80th-percentile error in days.
- [x] **WX-EST-DATE-003**: `datable` shall answer no for any crossing the scoring data file does not hold.

## The scoring tool and its data file

- [x] **WX-EST-SCORE-001**: The repository shall contain a scoring tool that, from public USCRN daily data and the grid's daily air at each station's point, fits each station's model on its five nearest neighbours and on the station alone, dates each crossing a consuming component dates by that component's own rule, and writes for each crossing and each calibration the median and 80th-percentile absolute error in days, the mean signed error, and the median and 80th-percentile spread between years.
- [x] **WX-EST-SCORE-002**: The scoring tool shall use no soil measurement from the target station when it scores the five-nearest calibration.
- [x] **WX-EST-SCORE-003**: The application shall embed the scoring tool's data file at build time and shall read every estimate accuracy figure it shows, and every `datable` answer, from that file.
- [x] **WX-EST-SCORE-004**: The scoring tool shall not run as part of the test suite.

## Degradation

- [x] **WX-EST-FAIL-001**: While a calibration station's wanted history is not yet held, the system shall fit no model for the depths that station calibrates and shall write no estimate for them.
- [x] **WX-EST-FAIL-002**: If fewer than five stations qualify for the five-nearest calibration, then the system shall fit on those that qualify when there are at least three and record how many were used, and shall fit no model when there are fewer than three.
- [x] **WX-EST-FAIL-003**: If a refit fails, then the system shall keep using the previous fit.
