---
parent: disease-risk
prefix: DIS-MODEL
---

# Disease Risk Models

## Context and Design Philosophy

Risk-models turns the lawn's recorded hourly weather and the forecast into one tier per disease: Low, Moderate, High or Severe. It produces a headline tier for "now" and a daily tier for each day of the past week and the outlook. The headline tier is the contract the parent defines: management, the feed and the UI all read it, and none of them reads weather. This leaf also defines `today` for the whole subtree.

Guiding principles:

- **Published models, native scores.** Each disease keeps the score its source defines. Experimental indices are labeled `validated: false` and shown as experimental.
- **Measured where it is near, estimated where it is not, always labeled.** Observed days come from the weather record's hourly air temperature and humidity for the lawn: the owner's sensor, a station within 10 miles, or the hourly analysis for the location, which is approximate. A tier computed from approximate hours is computed the same way and marked approximate. The forecast fills only the hours the record does not hold yet and the days ahead. Lag is disclosed in `data_notes` rather than hidden.
- **Withhold rather than invent.** A day that lacks the hours a model depends on cannot be the headline. When no recent day qualifies, the disease is left out.
- **Weather drives the daily series; history only raises the headline.** Lawn history multiplies susceptibility. It never creates risk out of unfavorable weather, and it never changes a daily bar.

| Owned | Path |
|---|---|
| Window constants, usable-day rule, trailing mean, headline scoring, amplifier, `assess_all` | `backend/src/logic/disease/mod.rs` |
| The five models | `logic/disease/brown_patch.rs`, `dollar_spot.rs`, `pythium.rs`, `gray_leaf_spot.rs`, `red_thread.rs` |
| Forecast days and the lake + forecast join | `logic/disease/weather_days.rs` |
| History fields of `DiseaseContext` (days since overseeding / fertilizer) | `logic/disease/history.rs` |
| Data types (`DailyWeather`, `DailyRisk`, `DiseaseRisk`, `RiskScale`, `RiskFactor`, `Methodology`, `DiseaseRiskResponse`) | `backend/src/models/disease.rs` |
| `GET /disease-risk` assembly: `compute`, `data_notes` | `backend/src/api/disease_risk.rs` |
| Query shape of the per-day disease inputs | `backend/src/datasources/weather/disease.rs` (weather-data owns the client) |

## Today and Day Boundaries

`today` is the lawn's local date, `state.clock.today()` (the app's `Clock`, in the timezone of the lawn's location; `api/disease_risk.rs`, `compute`). Every date in the subtree (the headline day, the display window, residual windows, the outlook, the rotation year, the 365-day log read) is measured from this `today`, including when the record holds no hours.

- **Observed hours** are grouped by the weather record into lawn-local days, daylight-saving aware, on the same clock.
- **Forecast points** are bucketed by `weather_days::forecast_days(points, &clock, after)`: each forecast point goes to the lawn-local date and hour of its own time (`clock.local_date`, `clock.local`), so points on either side of a daylight-saving change land on the right day and the dawn and afternoon flags use local hours.
- **The fetch** starts at `today − LOOKBACK_DAYS`. Recorded days and `today` share one calendar, so no margin day is added.

## Daily Weather Series

`DailyWeather` (`models/disease.rs`) describes one local day in **°C**, because the published models are specified in °C. It is never stored. Its fields:

- mean, min and max temperature, and mean RH;
- `hours_rh90`, and `leaf_wetness_hours` (hours with RH ≥ 90 % or any precipitation);
- precipitation, and peak dew point (Magnus);
- `hours_covered` and `is_forecast`;
- `basis`: what the day's observed hours rest on (sensor, station with its distance, or the approximate hourly analysis); absent on a forecast-only day;
- `covers_dawn` and `covers_afternoon`.

The record's contract counts only hours that have both air temperature and RH from one source, so `hours_covered` counts usable hours only. A day's temperature and humidity always come from the same source; its rain total comes from the record's rain basis.

**Window** (`logic/disease/mod.rs`): the fetch covers `LOOKBACK_DAYS` = 12 observed days, so trailing windows are already full on the first displayed day. The display shows `HISTORY_DAYS` = 6 days before today, today, and `FORECAST_DAYS` = 4 days after, for at most 11 days.

**Forecast days** (`weather_days::forecast_days`): points at or before `after` are dropped, because those hours are already observed. `after` is the newest usable observed hour the record returned since the fetch start; with no observed hours every point is kept. The remaining points are bucketed into local days, and each point stands for its slot's length (1 h from the default provider, 3 h from OpenWeatherMap), so `hours_covered` is the sum of those lengths and a humid slot counts its whole length. A point counts toward `hours_rh90` when RH ≥ 90 %, and toward leaf wetness when RH ≥ 90 % or precipitation > 0. Forecast temperatures are converted from °F.

**Join** (`weather_days::build_series`):

- A day with 24 observed hours wins outright.
- A partly observed day (usually today) is merged with that day's forecast, which holds only hours after the newest observation. The merge weights means by hours, takes the min of minimums and the max of maximums, sums hours and precipitation, caps `hours_covered` at 24 and ORs the flags.
- Forecast-only days are appended.

The result is sorted ascending by date. Because forecast days hold only hours the record does not hold, a merged day counts no hour twice in any field.

**Usable day** (`is_usable`): a day is usable when all three of these hold:

- `hours_covered ≥ 12`;
- `covers_dawn` (some local hour from 03 to 08);
- `covers_afternoon` (some local hour from 13 to 17).

**Trailing mean** (`trailing_mean`): averages a field over the usable days in a window ending at a given day. That day always counts, even when it is partial. The result is `None` when fewer than `min_days` days contribute.

## Headline and Fallback

`score_window` scores every day in the display window that has a value. Each becomes a `DailyRisk { date, value, tier, is_forecast, partial }`, where `partial = !is_usable`.

The headline is the **latest usable day on or before today**. That is today once enough of it is covered, otherwise the most recent complete day. The search reaches back no further than the start of the display window, 6 days. With no usable day in that span, the model returns `None` and the disease is left out of the response, with a data note naming it. `DiseaseRisk.as_of` names the headline day.

## Risk Models

| Disease (slug) | Model · `validated` | Daily value | Moderate | High | Severe |
|---|---|---|---|---|---|
| Brown patch (`brown-patch`) | Fidanza-Dernoeden E-index · true | `E = −21.5 + 0.15·RH + 1.4·Tmin − 0.033·Tmin²` (that day) | ≥ 0 | ≥ 5 (Vincelli revised) | ≥ 6 (original Fidanza) |
| Dollar spot (`dollar-spot`) | Smith-Kerns logistic · true | 5-day trailing means of temp and RH (≥ 3 usable days); `logit = −11.4041 + 0.0894·RH + 0.1932·T`; 0 % outside 10–35 °C | ≥ 10 % | ≥ 20 % (published action threshold) | ≥ 40 % |
| Pythium blight (`pythium-blight`) | Nutter-criteria threshold score · false | +1 max ≥ 30 °C, +1 min ≥ 20 °C, +1/+2/+3 at ≥ 6/10/14 h RH ≥ 90 %. Capped at 1 when no heat criterion is met. | ≥ 2 | ≥ 3 | ≥ 4 |
| Gray leaf spot (`gray-leaf-spot`) | Suitability index · false | 100 × temp trapezoid (18/25/30/35 °C) × wetness ramp (6→16 h); 0 outside Jul–Oct; 3-day trailing mean | ≥ 25 | ≥ 50 | ≥ 75 |
| Red thread (`red-thread`) | Suitability index · false | 100 × temp trapezoid (4/15/22/27 °C) × wetness ramp (4→12 h, floored at 0.5 on a ≥ 2.5 mm rain day); 5-day trailing mean | ≥ 25 | ≥ 50 | ≥ 75 |

`RiskScale` carries each model's gauge bounds, unit and three tier floors, and `tier_for` maps a value to a tier.

Every model also produces:

- a list of `RiskFactor`s (label, value, Unfavorable / Marginal / Favorable, note);
- a one-sentence summary;
- a `Methodology` (model name, citation, `validated`, summary, steps).

The UI and the feed show these unchanged. Each model calls management with its disease, headline tier, daily tiers, `today` and the context, and embeds the returned plan in its `DiseaseRisk`.

Pythium's peak dew point is shown as a factor but not scored (`pythium.rs`). The tier thresholds were sanity-checked against five seasons of station data, and any retuning repeats that backtest.

## History Amplifiers

`history::context_from_history(apps, today)` reads **lawn** applications only. It ignores plant applications (`plant_id` set) and future-dated entries. It sets `days_since_overseed` from the latest `Overseed` entry and `days_since_fertilizer` from the latest feeding, as Applications defines it (`Application::is_feeding()`: a turf `Fertilizer` entry with N unrecorded or above 0), so a potash-only product does not count. Management owns the fungicide fields of the same context.

| Amplifier | Condition | Effect |
|---|---|---|
| Seedlings → gray leaf spot | `Overseed` ≤ 60 days ago | Headline tier raised one step |
| Nitrogen-hungry → red thread (`red_thread::nitrogen_hungry`) | No feeding (`Application::is_feeding()`: a turf `Fertilizer` with N unrecorded or above 0) logged, or the last one > 60 days ago | Headline tier raised one step |

`amplify` raises a tier only when it is already ≥ Moderate, saturating at Severe. Only the headline is raised: `daily` stays weather-only, so its bars agree with the scale's thresholds. When a raise happens, `tier_note` reads "Raised from <tier> — …". The raised tier is the one handed to management and shown everywhere.

## Response and Data Notes

`GET /api/v1/disease-risk` returns `DiseaseRiskResponse { generated_at, today, basis, approximate, diseases[], data_notes[] }`. `basis` is the record's basis for hourly air and humidity on the headline day and its spans over the display window; `approximate` is true when the headline day's hours come from the hourly analysis, and each `DailyRisk` carries the same flag for its day. It responds 404 with no lawn profile and 503 with the weather record's sentence when the location is not set. A record or database error is returned as an error. `compute` builds the series and the context, runs `assess_all` and assembles `data_notes`. It returns the context alongside the response so the feed bridge (management) reuses the same run.

`data_notes` (`api/disease_risk.rs:129-172`) are plain-language caveats about the inputs. "Latest observed" and "today" are judged with the headline's own usable-day rule (`disease::is_usable`: hours, dawn and afternoon), so a note explains every fallback the headline makes:

- no usable observed day ("No recent weather observations are available.");
- observed days resting on the hourly analysis: that air temperature and humidity are an hourly estimate for the location, not a measurement at the lawn; that against measured stations it counts humid hours within about 3 hours a day; and that a sensor at the lawn would replace it, with a link to Sources;
- a change of basis inside the display window, with its date;
- the latest usable observed day is more than a day before today, so more recent days rely on the forecast;
- today is not usable, so current risk reflects the most recent complete day;
- no forecast, so no outlook days are shown;
- for each disease in `Disease::ALL` missing from the response on a cool-season lawn: "<Disease> is left out: no recent day has enough weather data to score it.";
- on a warm-season lawn, "Disease models cover cool-season turf only." (no disease is scored, and none is named as left out).

| Condition | Page behavior |
|---|---|
| Location not set, or the record cannot be read | Error response (feed behavior: see management) |
| Hours from the hourly analysis | Tiers computed as usual, marked approximate, with the data note |
| Neither a sensor, a near station nor the hourly analysis has recent hours | No usable observed day: the headline falls back, then diseases are left out, as below |
| Forecast missing | Observed days only, with a note; no outlook days |
| The record lagging | Headline falls back to the last usable day (≤ 6 days back), with a note |
| No usable day in the window | That disease is left out, with a note naming it. If all are left out, the list is empty and the first note explains why. |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Diseases covered | Brown patch, dollar spot, Pythium blight, gray leaf spot, red thread | Adding large patch, anthracnose | Large patch is a warm-season disease and anthracnose is a disease of Poa and bentgrass greens. Neither fits a tall-fescue-type lawn in Zone 7a. |
| Dollar spot model | Published Smith-Kerns (5-day moving averages, 20 % action threshold), checked against the UW Turfgrass Diagnostic Lab worked example | The reference site's DSV-style variant | The reference variant is a homebrew that pegs at 100 %. The published model is validated, including at a Pennsylvania site. |
| Hourly weather without a nearby instrument | The hourly analysis for the location, marked approximate | The nearest airport's observations; no disease risk without a sensor or near station | Against measured stations the analysis and an airport 4 to 20 miles away were both off by about 3 humid hours a day; the analysis needs no station list and covers every lawn. Withholding would leave most owners with no disease advice (weather-data decision). |
| A tier from approximate hours | The same tier scale and the same recommendations, marked approximate | Cap at Moderate; show risk but raise no recommendation | "Always labeled" (HLD). The analysis under-counts humid hours slightly (about 1 hour a day), so its error leans toward a missed alert, not a false one; a cap would make that worse. |
| Same-day data gap | Recorded hours joined with forecast hours after the newest one, falling back to the last usable day, with a data note | Forecast only for today | Measured or analysed hours are preferred to forecast ones, and the lag is disclosed. |
| Usable day | ≥ 12 h **and** dawn (03–08) **and** afternoon (13–17) hours | Hours alone | [inferred] A forecast-only afternoon has 12 h but no dawn low or humid night, which drive most scores. A day that ends at noon has no high. Both spans are wider than the longest forecast slot (3 h), so a covered span always contains a point. |
| Headline day | Latest usable day ≤ today, at most 6 days back, else omit | Always today; any day in the lookback | [inferred] A partial today would understate or overstate risk. Older than the display window is no longer "current". |
| Amplifier scope | Raise only the headline tier; the daily series stays weather-only | Amplify every day | Lawn history raises only the headline tier (`docs/agronomy-methods.md`). The bars must match the scale's thresholds. |
| Amplifier floor | Raise only at ≥ Moderate | Raise from Low too | [inferred] History is a susceptibility multiplier. It should not create risk when the weather is unfavorable (`mod.rs` `amplify`). |
| Pythium dew point | Shown for context, not scored | Score it as a criterion | [inferred] A backtest on station data found it nearly redundant with a warm night, and it inflated High/Severe days (`pythium.rs`). |
| Merged partial day | Forecast points fill only hours the record does not hold | Sum both sources and cap hours at 24 | Summing double-counts humid hours and inflates Pythium and gray leaf spot; capping only `hours_covered` hides it. |
| Data notes | Judge "today usable" with the headline's usable-day rule, and name any disease left out | Hours alone; silent omission | A note must explain every fallback and every missing disease, or the page reads as if the risk were Low. |
| Model temperature unit | °C inside `DailyWeather`, which is never stored | °F, as stored data is | [inferred] The published formulas are specified in °C. Values are converted to °F only for display. |
| Day boundary | The lawn's local date (the app's `Clock`) for `today`, observed hours and forecast points alike | UTC date; a station's local date | Every agronomic date is a lawn-local calendar day, and one clock gives every component the same day (HLD). A fixed offset misplaces points across a daylight-saving change. |

## Open Questions & Future Decisions

### Resolved

1. ✅ The disease set is five diseases. Large patch and anthracnose are excluded (owner decision).

### Deferred

1. **A merged day reads as forecast.** `merge_day` ORs `is_forecast` (`weather_days.rs:98`), so a partly observed day topped up with forecast hours reports `is_forecast = true` and its bar is drawn as a forecast.
2. **Recomputed per request.** The dashboard triggers two full computations, one through the feed and one through the widget, each with a record read (`api/disease_risk.rs`). There is no memo.
3. **Leaf wetness is a proxy.** It is approximated as RH ≥ 90 % or precipitation. Station solar radiation and surface temperature, which the record does not store, could improve it.
4. **Tier thresholds were checked on station data only.** They have not been re-run on the hourly analysis, whose humid-hour count runs about an hour a day low.
5. **Tech debt** against the 50-line limit: `api::disease_risk::compute` (~80 lines), `brown_patch::factors` (~72), `pythium::factors` (~65).

## References

- Parent: `docs/intent/disease-risk/disease-risk-design.md`; siblings `management/`, `disease-ui/`
- `docs/agronomy-methods.md` § Disease risk: owner-facing method, kept in sync with these thresholds
- Fidanza, Dernoeden & Grybauskas (1996), Phytopathology 86:385–390 (brown patch E-index)
- Smith, Kerns et al. (2018), PLOS ONE 13(3):e0194216 (dollar spot)
- Nutter, Cole & Schein (1983), Plant Disease 67:1126–1128 (Pythium criteria)
- Uddin, Serlemitsos & Viji (2003), Phytopathology 93:336–343 (gray leaf spot drivers)
- Weather data access (WX): the record's `fetch_daily_disease_inputs` and its basis, the forecast
