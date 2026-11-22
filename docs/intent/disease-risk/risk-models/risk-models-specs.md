# Disease Risk Models — EARS Specs

Design: [risk-models-design.md](risk-models-design.md). Parent: [disease-risk](../disease-risk-design.md). Prefix `DIS-MODEL`. Facets: `DAY` (today, the daily series, usable days), `HEAD` (headline day), `SCORE` (the five models and tiers), `AMP` (history amplifiers), `API` (`GET /disease-risk` and data notes).

A "usable day" has at least 12 covered hours, at least one hour from 03–08 local (dawn) and at least one hour from 13–17 local (afternoon). The "headline tier" is the single tier per disease that management, the feed and the UI read.

## Today and the daily series

- [x] **DIS-MODEL-DAY-001**: The disease risk computation shall use the lawn's local date (the app clock) as "today" for the headline search, the display window, the lookback fetch and every date it hands to management, including when the weather record holds no hours.
- [ ] **DIS-MODEL-DAY-002**: When building the observed daily series, the system shall take each lawn-local day's hourly aggregates from the weather record's daily disease inputs as returned, with the basis of that day's hours, without re-bucketing them.
- [ ] **DIS-MODEL-DAY-003**: When building forecast days, the system shall bucket each forecast point into the lawn-local date of that point's own time, so points on either side of a daylight-saving change land on the correct day.
- [x] **DIS-MODEL-DAY-004**: The system shall express each daily weather record in °C with mean, minimum and maximum temperature, mean RH, hours at RH ≥ 90%, leaf-wetness hours (RH ≥ 90% or any precipitation), precipitation, peak dew point, hours covered, whether it is forecast, and whether it covers dawn and afternoon hours; it shall never store these records.
- [ ] **DIS-MODEL-DAY-005**: When building forecast days, the system shall count each forecast point as its slot's length in hours, counting that length toward RH ≥ 90% hours when its RH is at least 90% and toward leaf wetness when its RH is at least 90% or its precipitation is above 0.
- [x] **DIS-MODEL-DAY-006**: When joining observed and forecast days, the system shall keep a fully observed (24-hour) day as observed, merge a partly observed day with that day's forecast (hour-weighted means, min of minimums, max of maximums, summed hours and precipitation, hours capped at 24), and append forecast-only days, sorted by date.
- [ ] **DIS-MODEL-DAY-007**: When merging a partly observed day with its forecast, the system shall use forecast points only for hours after the newest observed hour the record returned, so no hour is counted twice in any field.
- [x] **DIS-MODEL-DAY-008**: The system shall fetch 12 observed days so trailing windows are full on the first displayed day, and shall display the 6 days before today, today and the 4 days after.
- [x] **DIS-MODEL-DAY-009**: When computing a trailing mean for a model, the system shall average over the usable days in the window ending at the scored day, always including the scored day, and return no value when fewer days than the model's minimum contribute.

## Headline day

- [x] **DIS-MODEL-HEAD-001**: The system shall score every day in the display window that has a value, recording its value, tier, whether it is forecast, and whether it is partial (not usable).
- [x] **DIS-MODEL-HEAD-002**: The system shall take each disease's headline from the latest usable day on or before today, searching back no further than the start of the 6-day display history, and shall name that day as the risk's as-of date.
- [x] **DIS-MODEL-HEAD-003**: If no day in the 6-day history is usable for a disease, then the system shall give that disease no headline and leave it out of the risk list.

## Models and tiers

- [x] **DIS-MODEL-SCORE-001**: The system shall score brown patch with the Fidanza-Dernoeden E-index (`−21.5 + 0.15·RH + 1.4·Tmin − 0.033·Tmin²`, that day) and tier it Moderate ≥ 0, High ≥ 5, Severe ≥ 6.
- [x] **DIS-MODEL-SCORE-002**: The system shall score dollar spot with the Smith-Kerns logistic model on 5-day trailing means of temperature and RH (at least 3 usable days), as 0% outside 10–35°C, and tier it Moderate ≥ 10%, High ≥ 20%, Severe ≥ 40%.
- [x] **DIS-MODEL-SCORE-003**: The system shall score Pythium blight as a Nutter-criteria threshold score (+1 for max ≥ 30°C, +1 for min ≥ 20°C, +1/+2/+3 for ≥ 6/10/14 hours at RH ≥ 90%, capped at 1 when no heat criterion is met), mark it experimental, and tier it Moderate ≥ 2, High ≥ 3, Severe ≥ 4.
- [x] **DIS-MODEL-SCORE-004**: The system shall score gray leaf spot as a suitability index (100 × a temperature trapezoid 18/25/30/35°C × a wetness ramp 6→16 h, 0 outside July–October, 3-day trailing mean), mark it experimental, and tier it Moderate ≥ 25, High ≥ 50, Severe ≥ 75.
- [x] **DIS-MODEL-SCORE-005**: The system shall score red thread as a suitability index (100 × a temperature trapezoid 4/15/22/27°C × a wetness ramp 4→12 h floored at 0.5 on a day with ≥ 2.5 mm of rain, 5-day trailing mean), mark it experimental, and tier it Moderate ≥ 25, High ≥ 50, Severe ≥ 75.
- [x] **DIS-MODEL-SCORE-006**: The system shall keep each model's native score and never blend scores across diseases; only tiers compare.
- [x] **DIS-MODEL-SCORE-007**: For every disease, the system shall return the risk scale (gauge bounds, unit, tier floors), contributing factors (label, value, Unfavorable / Marginal / Favorable, note), a one-sentence summary and a methodology (model, citation, whether validated, summary, steps).
- [x] **DIS-MODEL-SCORE-008**: The Pythium model shall show the peak dew point as a factor without scoring it.

## History amplifiers

- [x] **DIS-MODEL-AMP-001**: When building the disease context, the system shall read only lawn applications dated on or before today, ignoring plant applications, and record the days since the latest Overseed and since the latest feeding (feeding as defined by Applications (`Application::is_feeding()`: a turf Fertilizer application with nitrogen unrecorded or above 0)).
- [x] **DIS-MODEL-AMP-002**: When an Overseed application is logged within the last 60 days and gray leaf spot's headline tier is at least Moderate, the system shall raise gray leaf spot's headline tier one step.
- [x] **DIS-MODEL-AMP-003**: When no feeding is logged within the last 60 days and red thread's headline tier is at least Moderate, the system shall raise red thread's headline tier one step.
- [x] **DIS-MODEL-AMP-004**: When an amplifier raises a headline tier, the system shall raise it at most to Severe, leave the daily tiers unchanged, and set the tier note "Raised from <tier> — …"; the raised tier is the one every consumer reads.

## Endpoint and data notes

- [ ] **DIS-MODEL-API-001**: When `GET /api/v1/disease-risk` is called, the system shall return generated-at time, today, the basis of hourly air and humidity on the headline day and its spans over the display window, whether the headline day is approximate, every disease's risk, and data notes, from the same computation the feed's disease recommendations use.
- [ ] **DIS-MODEL-API-002**: If `GET /api/v1/disease-risk` is called with no lawn profile, then the system shall return 404; when the weather record returns the location-not-set outage, 503 with that outage's sentence; and if the record read fails, an error.
- [ ] **DIS-MODEL-API-003**: The response shall include a data note when there are no recent weather observations, when observations end more than a day before today, when today does not reflect current conditions so the headline uses the most recent complete day, and when there is no forecast.
- [x] **DIS-MODEL-API-004**: The data notes shall judge whether today and the latest observed day are usable with the same usable-day rule the headline uses (hours, dawn and afternoon coverage).
- [x] **DIS-MODEL-API-005**: If a disease is left out because no day in its 6-day history is usable, then the response shall include a data note naming that disease.
- [ ] **DIS-MODEL-API-006**: The system shall mark a day's risk approximate when that day's observed hours come from the hourly analysis, shall mark the response approximate when the headline day is, and shall compute an approximate day's score and tier exactly as it does a measured day's.
- [ ] **DIS-MODEL-API-007**: When any observed day in the display window is approximate, the response shall include a data note saying that air temperature and humidity are an hourly estimate for the location, that against measured stations it counts humid hours within about 3 hours a day, and that a sensor at the lawn would replace it.
- [ ] **DIS-MODEL-API-008**: When the basis of hourly air and humidity changes inside the display window, the response shall include a data note giving the date of the change.
