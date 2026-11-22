---
parent: weather-data
prefix: WX-EST
---

# Soil Estimate

## Context and Design Philosophy

About half of US lawns are more than 50 miles from a station that measures soil temperature,
and every station's probes fail for days to months. This leaf owns the modeled soil-temperature
series that stands in for a measurement in both cases: a daily series at 5 cm and 10 cm for the
lawn's location, computed from gridded air temperature by a regression fitted to nearby
stations' measured soil.

It reads gridded air and calibration stations' soil from the weather record and writes its
series back as one more source. The record decides when the estimate speaks; consumers decide
what to do with an estimated date.

Principles:

- **An estimate earns its place by being scored** (HLD tenet). Every threshold the estimate may
  date has a published error measured against stations. A threshold that was not scored, or
  that scored worse than the calendar, is not dated from the estimate.
- **The published figures are the scored figures.** The accuracy the app displays is read from
  the scoring run's output, not written by hand.
- **A model of the place, not of the country.** It is fitted to the stations nearest the lawn,
  because the air-to-soil relation differs by soil, cover and climate.

## The Model

One model per depth (5 cm, 10 cm), fitted by ordinary least squares:

```
soil(d) = b0
        + b1·A1(d) + b3·A3(d) + b7·A7(d) + b14·A14(d) + b30·A30(d)
        + s·sin(2π·doy(d) / 365.25) + c·cos(2π·doy(d) / 365.25)
```

- `Aw(d)` is the mean of daily air temperature over the `w` days ending on `d`, where daily air
  is `(max + min) / 2` from the grid at the point. A window is usable when at least 80 % of its
  days have a value; a day with any unusable window has no estimate.
- The day-of-year terms carry what air alone misses: soil lags the season, so the same air
  temperature means cooler soil in spring than in autumn.
- The output is the daily mean soil temperature in °F, rounded to 0.1.

The estimate is daily. It has no hourly form.

## Calibration

| Situation | Fitted on |
|---|---|
| No station is the basis for soil temperature at the lawn | The five nearest USCRN stations that have at least 1,000 days of soil at that depth, pooled |
| A station (USCRN or SCAN) is the basis for soil temperature and has at least 1,000 days of soil at that depth | That station alone, and the series is computed from the grid's air at that station's point |

- **Each station's soil is paired with the grid's air at that station's point**, not with the
  station's own air sensor, so the model learns the relation between the driver it will be
  given and measured soil. The record therefore holds grid air at up to six points: the lawn's
  and each calibration station's.
- **Training span:** every day from 2012 on for which both sides exist. USCRN soil probes were
  installed network-wide by then.
- **Refit** monthly, and whenever the calibration set changes. Coefficients are stored with the
  fit date, the stations used and the number of days, and the whole series is recomputed after
  a refit.
- **Recent days** are recomputed whenever the grid revises their air (the record re-fetches the
  grid's last 7 days daily), and the 30 days after a revised day with them.

- **Choosing the five** before their data is held: the five nearest by the catalog's years of
  operation. Any that turn out to have fewer than 1,000 days are replaced by the next nearest.
- **Whose air drives the series.** With no basis station the model runs on the grid's air at
  the lawn's point. Where it fills a basis station's gaps it runs on the air at that station's
  point, because it is standing in for that station's readings, not describing a different
  place.

Calibration stations are fetched as daily values only. They are never a basis for the lawn.

## What It Publishes

| Output | Form |
|---|---|
| The series | One daily value per depth, for the span the record holds grid air, written to the record as the source "Soil estimate" |
| The anchored series | The series adjusted to a measured series around a gap (below) |
| The forward estimate | The same model applied to a daily air series that continues into the forecast, for consumers that need soil on days not yet observed |
| `datable(depth, threshold, direction)` | Whether the estimate, as calibrated for this lawn, may date that crossing, and its scored error (median and 80th percentile, days) |
| The fit | Stations used with distances, days of training data, fit date, daily error at each depth |

### The anchored series

Where the estimate fills days in a station's or sensor's series, an unadjusted estimate would
put a step at each end of the gap, and a step can be read as a threshold crossing. The anchored
series removes it:

- The **offset** at an edge of a gap is the mean of (measured − estimate) over the measured
  days among the 7 nearest that edge.
- **A gap with measurements on both sides** takes the offset interpolated linearly from one
  edge to the other.
- **A gap still open** takes the offset at its start, shrinking linearly to zero over 30 days.
- **The days before a source's first value** are not anchored. A newly mapped sensor's past
  is the unadjusted series of the next basis.

Anchored values are labeled as estimates like any other.

### Which crossings the estimate may date

A crossing is datable when it was scored and its median error is smaller than the median
spread of that crossing between years at the same station; that is, when the estimate beats
the calendar. The test is applied to the calibration in use, so `datable` can differ between a
lawn with no soil station and a lawn whose station's gaps are being filled. Scored with no soil measurement from the target station used (median | 80th percentile
absolute error, days):

| Depth | Crossing | Estimate | Bias | Calibrated on the station itself | Between years | Datable |
|---|---|---|---|---|---|---|
| 5 cm | Rising 45 °F | 2 \| 11 | -2.6 | 1 \| 7 | 4 \| 12 | Yes |
| 5 cm | Rising 50 °F | 2 \| 12 | -1.8 | 1 \| 7 | 5 \| 12.5 | Yes |
| 5 cm | Rising 55 °F | 2 \| 13 | -1.8 | 1 \| 8 | 5.5 \| 13 | Yes |
| 5 cm | Rising 65 °F | 5 \| 17 | +1.1 | 2 \| 10 | 6 \| 13 | Yes |
| 5 cm | Falling 75 °F | 1 \| 11 | -1.3 | 1 \| 6 | 2 \| 9 | Yes |
| 5 cm | Falling 70 °F | 2 \| 15 | +0.0 | 1 \| 8 | 4.5 \| 12 | Yes |
| 5 cm | Falling 65 °F | 3 \| 13 | +2.4 | 1 \| 8 | 5 \| 11.5 | Yes |
| 5 cm | Falling 55 °F | 2 \| 10 | +3.3 | 2 \| 7 | 5.5 \| 11 | Yes |
| 5 cm | Falling 40 °F | 3.5 \| 12 | +5.3 | 2.5 \| 11 | 5 \| 12 | Yes |
| 10 cm | Rising 50 °F | 3 \| 14 | -2.4 | 2 \| 8 | 6.5 \| 14.5 | Yes |
| 10 cm | Rising 55 °F | 3 \| 14 | -1.7 | 2 \| 7 | 6 \| 13.5 | Yes |
| 10 cm | Rising 60 °F | 4 \| 15 | -0.4 | 2 \| 9 | 6 \| 13 | Yes |
| 10 cm | Rising 65 °F | 4 \| 15 | +2.0 | 2 \| 7 | 6.5 \| 13 | Yes |
| 10 cm | Rising 75 °F | 7 \| 19 | +4.1 | 2 \| 10 | 5.5 \| 12 | Only when calibrated on the station itself |
| 10 cm | Falling 65 °F | 3 \| 14 | +2.0 | 1 \| 7 | 5.5 \| 12 | Yes |
| 10 cm | Falling 55 °F | 3 \| 10 | +2.9 | 2 \| 6 | 5 \| 10.5 | Yes |
| 10 cm | Falling 50 °F | 3 \| 11 | +3.6 | 2 \| 8 | 5.5 \| 10.5 | Yes |
| 10 cm | Falling 45 °F | 4 \| 12 | +5.1 | 2.5 \| 10 | 5.5 \| 11.5 | Yes |

The figures are the scoring tool's (`backend/src/logic/soil_estimate/scores.json`), from 114
USCRN stations over 2012–2025. Each crossing is dated by its consumer's own rule: at 5 cm the
timing windows' 5-day mean held 5 days within the window's scan range, at 10 cm the seasonal
plan's 7-row mean held 5 days. "Estimate" is the five-nearest fit, which is what a lawn with
no soil station gets; the next column is the fit that fills gaps in a station's own record.
Bias is the mean signed error of the five-nearest fit; positive means the estimate dates the
crossing late. The daily error of the series is 3.5 °F (root mean square) at 5 cm and 3.1 °F
at 10 cm from the five-nearest fit, and 2.6 °F and 2.2 °F calibrated on the station itself.

The estimate tends to date the late-autumn crossings late: falling 40 °F at 5 cm and falling
45 °F at 10 cm by about 5 days on average. Rising 75 °F at 10 cm is the one crossing the
five-nearest fit dates worse than the calendar.

Without the day-of-year terms the 10 cm warming crossings ran 3 to 5 days late and the daily
error was 0.5 °F larger at both depths.

A crossing absent from the table is not datable. The consumer shows its typical date or
withholds, as it does when a station is stale.

## Scoring

The scoring tool lives in the repository and reproduces the table above from public data: it
downloads the USCRN daily files and the grid's air at each station's point, fits each station's
model on its five nearest neighbours, dates each crossing by the consuming component's own
rule (mean length, days held), and writes the result as a data file. The application embeds
that file; `datable` and every accuracy figure shown to the owner are read from it.

The tool is run by a maintainer when the model, the calibration rule or a consumer's crossing
rule changes. It is not part of the test suite: it needs the network and several minutes.

## Failure and Degradation

| Failure | Effect |
|---|---|
| A calibration station's history has not loaded yet | No fit; the series is absent and the record reports history still loading |
| Fewer than five stations qualify after fetching | Fit on those that do, down to three; the fit records how many. Fewer than three: no fit |
| Grid air missing for recent days | No estimate for those days until the record holds air from the fallback grid or the grid returns |
| Fewer than 30 days of air before a day | That day has no estimate |
| A refit fails | The previous coefficients stay in use |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Source of estimated soil | A regression on gridded air, fitted to nearby stations | A reanalysis soil product (Open-Meteo, ERA5-Land); copying the nearest station's dates | Reanalysis dated rising 55 °F 4 \| 18 to 4 \| 21 days with a 4-day early bias; the regression 2 \| 13. Copying a station's dates is 5 \| 17 by 60–100 miles and 8 \| 22 beyond. |
| Calibration set without a basis station | The five nearest stations, pooled | The nearest one; every station pooled | Nearest-one scored 3 \| 16 on rising 55 °F and all-pooled worse than five; five averages out one odd site while staying regional. |
| Calibration with a basis station | That station alone | The five nearest, as elsewhere | A model fitted to the station it fills scored 1 \| 8 on rising 55 °F at 5 cm against 2 \| 13 for the pooled fit, and the gaps being filled are in that station's own series. |
| Air behind a gap-filling series | The grid at the basis station's point | The grid at the lawn's point | The missing readings are the station's. Air from a lawn 30 miles away would put a step at each edge of the gap, and the station-alone accuracy was scored on the station's own point. |
| Air paired with station soil | The grid's air at the station's point | The station's own air sensor | The model is driven by grid air at the lawn; fitting it to a different air measurement would build the grid's bias into every estimate. |
| Seasonal term | One day-of-year sine and cosine | Air terms only; separate spring and autumn models | Removes a 3 to 5 day late bias on the 10 cm warming crossings and cuts the daily error by 0.5 °F, with two coefficients. Separate models add a seam between seasons. |
| Depths | 5 cm and 10 cm | 5 cm only; all five station depths | These are the depths the timing windows, seasonal plan and outlook read. Deeper soil drives no advice. |
| Not-datable rule | Scored, and median error below the between-year median spread | Date every crossing and show the error; a fixed day limit | An estimate that misses by more than the calendar does is worse than showing the typical date (HLD tenet: withhold rather than invent). |
| Gap edges | Offset matched to the measured series, interpolated across the gap or decayed over 30 days | Unadjusted estimate with the seam marked only | The series' daily error is about 3 °F, enough for a step at a gap edge to cross a threshold that the soil did not. |
| A new sensor's past | Unadjusted | Anchor backwards from the sensor's first days | One week of a sensor says little about its offset in another season. |
| Soil on forecast days, estimate basis | The same model run on forecast air | Fit the outlook's 30-day lagged regression to the estimated series | A regression fitted to the output of another regression on the same air adds error and no information. |
| Where accuracy figures come from | A data file written by the scoring tool and embedded in the app | Figures typed into code and documents | The figure shown cannot drift from the figure measured (HLD success metric: estimate accuracy). |

## Open Questions & Future Decisions

### Deferred

1. **Calibration on a SCAN station alone is unscored.** The station-alone figures come from
   USCRN stations.
1. **Nearest-one and all-stations calibration were scored without the day-of-year terms**
   only. The choice of five rests on that comparison.
2. **The anchoring rules are unscored.** The 7-day edge and the 30-day decay are reasoned, not
   measured. Hiding a station's real days and filling them would score both.
3. **Thin evidence in the mountain and arid West**, where stations are far apart and the five
   nearest can span very different sites.
4. **Individual stations fail badly under every method tried**: Millbrook NY (falling 70 °F
   about 22 days early), Des Moines IA, Necedah WI. No rule yet detects such a site from the
   lawn's side.
5. **Reconciling an owner's soil probe with the estimate** once a year of overlap exists, so
   the probe's past could be an anchored estimate.
6. **The grid may ingest the stations it was scored against**, which would flatter the air
   driver at stations relative to an arbitrary lawn.

## References

- Parent: `docs/intent/weather-data/weather-data-design.md`.
- Siblings: `weather-record/` (storage, when the estimate is the basis, the anchored series'
  use), `weather-sources/` (the grid and USCRN daily clients), `soil-temp-outlook/` (the
  forward estimate's consumer at 10 cm).
- `docs/intent/timing-windows/`: the 5 cm crossing rules and what a window does with a
  crossing that is not datable.
- [docs/plans: standalone-weather-sources](../../../plans/standalone-weather-sources.md): the
  tests behind every figure here.
