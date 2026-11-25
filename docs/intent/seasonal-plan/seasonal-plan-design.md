---
parent: high-level-design
prefix: PLAN
---

# Seasonal Plan & Calendar

## Context and Design Philosophy

The seasonal plan answers "what lawn work falls in which weeks of this year, and where do
I stand on each". It is a year-long timeline of planned activities, each with a predicted
date window, a confidence, a status (Upcoming / Active / Completed / Missed) and
supporting detail. The calendar shows the same activities as bars on a month grid, next
to the applications that were actually logged and their follow-ups.

The plan is an **assembly point**, not a single model. It merges three sources into one
list:

1. **10 cm crossing activities** that this component computes itself: windows predicted
   from the lawn's historical 10 cm soil-temperature crossings, computed from the weather
   record (`backend/src/logic/seasonal_plan.rs`). Code comments call these the
   "legacy" or "10 cm version".
2. **Timing-window activities** contributed by the Timing windows component
   (`backend/src/logic/timing/plan.rs`): spring pre-emergent, fall pre-emergent, fall
   seeding and core aeration, dated on 5 cm soil and freeze dates.
3. **Plant tasks** contributed by the Landscape plants component
   (`backend/src/logic/plant_maintenance.rs`).

The governing tenet is **one answer everywhere**: where the Timing windows answer for an
activity, the plan carries the timing version and never the 10 cm one, so the plan and
calendar agree with the Timing page and the feed. The 10 cm activities remain the source
for the dates of everything the windows do not yet cover (spring nitrogen, spring and
fall broadleaf herbicide, grub preventative, the two fall feedings and the winterizer),
and act as the fallback when the timing windows are unavailable. Those seven activities
are also advised by an agronomic rule, so for the season being lived now their status
comes from that rule's state (see Status), exactly as a timing activity's status comes
from its window's state.

This component owns: the 10 cm crossing method and its activities,
the merge in `GET /api/v1/seasonal-plan`, `GET /api/v1/applications/calendar`, and the
Seasonal Plan (`/seasonal-plan`) and Calendar (`/calendar`) pages. It does not own the
timing activities' dates or statuses, nor plant task windows or completion.

The plan is designed for a cool-season lawn (`Mixed` is a cool-season blend); a
warm-season profile is unsupported.

## Data Model

`backend/src/models/seasonal_plan.rs` (mirrored in `frontend/src/types/index.ts`):

| Type | Fields | Notes |
|---|---|---|
| `SeasonalPlan` | `year`, `activities`, `data_years_used`, `estimated_years`, `generated_at`, `today` | `data_years_used` = the complete prior years that voted (PLAN-CROSS-010); `today` = the lawn's local date the statuses were judged on |
| `PlannedActivity` | `id`, `name`, `category`, `description`, `date_window`, `status`, `status_estimated`, `details` | `id` is the merge key; `category` is free text; `status_estimated` marks a typical-date status given because the covering rule could not judge the season |
| `DateWindow` | `predicted_start`, `predicted_end`, `earliest_historical?`, `latest_historical?`, `confidence` | dates are `NaiveDate` |
| `WindowConfidence` | `High` · `Medium` · `Low` | from the sample count of the opening threshold |
| `ActivityStatus` | `Upcoming` · `Active` · `Completed` · `Skipped` · `Missed` | computed per request; `Skipped` ("Not needed this season") only for a covered feeding past the annual nitrogen target |
| `ActivityDetails` | `soil_temp_trigger?`, `product_suggestions[]`, `rate?`, `notes?` | free text |
| `ThresholdCrossing` | `year`, `threshold_name`, `crossing_date`, `avg_soil_temp_f`, `estimated` | one per threshold and year |
| `DailySoilTempAvg` | `date`, `avg_temp_f`, `estimated` | input from the weather record |

The category string `"Plant Maintenance"` is the contract that tells both pages an
activity is a plant task (the Landscape plants component sets it).

## 10 cm Crossing Method

### Input

The weather record's `fetch_daily_soil_temp_averages` returns the lawn's resolved daily
10 cm soil temperature for a span of years, ordered by day, with the basis of each day:
measured (the owner's probe, or a station within 50 miles on a day with at least 12 valid
hours) or the soil estimate. This is a Weather data access contract; a year with fewer
than 30 such days is not analyzed (`analyze_year`,
`backend/src/logic/seasonal_plan/crossings.rs`).

### Crossing detection

`find_threshold_crossings` (`backend/src/logic/seasonal_plan/crossings.rs`) computes a trailing
7-row mean over the returned days (stamped on the 7th row) and looks for **held runs**:
consecutive calendar days on which the mean is at or past a threshold in its direction.
A run is a crossing once it holds for 5 days, dated on its first day; a mean back across
the threshold, a missing date, or the end of the year's data before the fifth day ends it
without a crossing, and the search moves on. Each threshold records only its **first**
held crossing of the year:

| Threshold name | °F | Direction | Searched when |
|---|---|---|---|
| `soil_50f_rising` | 50 | mean ≥ T | run starts in Jan–Jun |
| `soil_55f_rising` | 55 | rising | Jan–Jun |
| `soil_60f_rising` | 60 | rising | Jan–Jun |
| `soil_65f_rising` | 65 | rising | Jan–Jun |
| `soil_75f_rising` | 75 | rising | Jan–Jun |
| `soil_65f_falling` | 65 | mean ≤ T | run starts in Jul–Dec |
| `soil_55f_falling` | 55 | falling | Jul–Dec |
| `soil_50f_falling` | 50 | falling | Jul–Dec |
| `soil_45f_falling` | 45 | falling | Jul–Dec |

A run that starts on the year's first mean is not a crossing (there is no evidence the
threshold was crossed rather than already past). A run whose first day immediately
follows a missing date is dropped, leaving that threshold with no crossing for the year:
it dates the end of a sensor outage, not a soil change. Where the estimate fills a
measured series' gap there is no missing date, and the record supplies the estimate
anchored to the measurements on each side.

A crossing is **estimated** when any of its five held days rests on the estimate. An
estimated crossing is kept only when the soil estimate may date that threshold for this
lawn (`datable`, 10 cm); otherwise that threshold has no crossing for the year. Both rules are the Timing
windows' (`after_gap`, 5-day hold), so the two methods read the record the same way. The
mean itself is over rows, not calendar days (PLAN-CROSS-002).

### Aggregation across years

Only **complete prior years** vote: the at most 10 most recent years before the plan
year that are **settled** and have crossings. A year is settled once it is over on the
lawn's calendar and the weather record holds a day dated Dec 31 of that year or later for
10 cm soil. The plan year never votes on its own prediction, and a partial year (the one
in progress, or one the record does not yet hold through Dec 31) never votes.
`data_years_used` is the number of voting years, and `estimated_years` how many of them
contributed an estimated crossing.

`aggregate_crossings(crossings, name, target_year)` maps each voting crossing of that
threshold onto the target year by **month and day** (Feb 29 becomes Feb 28 in a non-leap
target year), sorts, and returns `median` (the element at index `n/2`, i.e. the upper
median for even `n`), `earliest`, `latest` and `sample_count`. Mapping by month and day
keeps a leap year's dates on their calendar day and keeps late-December crossings in the
target year. Confidence (`confidence_from_count`): ≥5 samples High, 3–4 Medium,
otherwise Low.

### Activities

Window start/end are medians of the listed thresholds unless noted; a missing end
threshold falls back to a fixed offset from the start. "Completed" is judged against the
requested year's application log (calendar year), counting **turf applications only**
(`Application::is_turf()`): a plant insecticide does not complete grub preventative, and
a plant feeding does not complete a lawn feeding. A "feeding" is `Application::is_feeding()`
(a turf `Fertilizer` with nitrogen unrecorded or above 0), as defined by Applications.

| Id | Window | End fallback | Completed when (in `year`) |
|---|---|---|---|
| `pre_emergent` | 50 rising → 60 rising | start + 30 d | any `PreEmergent` Jan–Jun |
| `spring_nitrogen` | 55 rising → 65 rising | start + 21 d | a feeding Jan–May |
| `spring_herbicide` | 50 rising − 7 d → 55 rising + 7 d (edges moved into `year` by year only) | start + 28 d | any `PostEmergent` Jan–May |
| `grub_preventative` | max(60 rising, May 15) → 75 rising | Jul 4 | any `GrubControl` or `Insecticide` |
| `core_aeration` | 65 falling → 50 falling | start + 30 d | any `Aeration` |
| `fall_overseeding` | 65 falling → 50 falling | start + 28 d | any `Overseed` |
| `early_fall_fert` | 65 falling → +21 d | — | a feeding in Sep |
| `mid_fall_fert` | 65 falling + 21 d → +21 d | — | a feeding in Oct |
| `winterizer` | 45 falling → +14 d | — | a feeding Nov–Dec |
| `fall_herbicide` | 65 falling → 55 falling | start + 28 d | any `PostEmergent` Sep–Dec |

Each window's historical range runs from the opening threshold's earliest crossing to
the closing threshold's latest, shifted by the window's offsets; `spring_nitrogen`'s ends
at the latest 65 °F-rising crossing like any other closing threshold.

The fallback `pre_emergent` (shown only when the Timing windows return nothing) counts
only a January–June `PreEmergent`, so a fall pre-emergent cannot complete the spring
activity; the timing path separates the two seasons the same way.

`spring_herbicide`'s edges are moved into `year` by changing only their year: the month
and day are kept, so an edge on the 29th–31st stays there.

`fall_herbicide` is omitted only when it is not Completed and an `Overseed` falls within
the 60 days before its window opens (the 65 °F-falling median): new seedlings cannot take
a broadleaf herbicide that soon. A seeding on or after the median leaves the activity in
place, and a completed one is never hidden. An activity whose opening threshold has no
crossing is omitted. Each carries trigger text naming "7-day avg at 10cm",
product suggestions and notes.

`ActivityDetails.rate` follows the project's rates rule:

| Activity kind | `rate` |
|---|---|
| Feedings (`spring_nitrogen`, `early_fall_fert`, `mid_fall_fert`, `winterizer`) | nutrient amount in lb N/1,000 sq ft |
| Seeding (`fall_overseeding`) | seed per 1,000 sq ft |
| Pesticides (`pre_emergent`, `spring_herbicide`, `grub_preventative`, `fall_herbicide`) | the words "Label rate", with no number |
| `core_aeration` | none |

Nutrient and seed amounts are agronomy and belong in the plan. A pesticide's amount is
set by its label, so the plan points there instead of stating one.

### Status

For the **season being lived now**, each activity an agronomic rule also covers takes
its status from that rule's state, not from its dates:

| Activity | Rule (Agronomic rules) |
|---|---|
| `spring_nitrogen` | spring nitrogen |
| `early_fall_fert`, `mid_fall_fert`, `winterizer` | fall fertilization |
| `spring_herbicide`, `fall_herbicide` | broadleaf herbicide |
| `grub_preventative` | grub control |

The rules expose each covered activity's state (`logic/rules/plan_state.rs`
`season_states`, owned and defined by Agronomic rules, RULE-ENG-010 to 015), judged on
the same date, conditions, application log and assembled feed the recommendations come
from. `apply_rule_states` maps it:

| Rule state | Plan status |
|---|---|
| Done (the rule's own log check finds the work) | Completed |
| Due (the activity's recommendation is in the feed) | Active |
| Not needed (that recommendation is a feeding past the annual nitrogen target) | Skipped |
| Passed (the rule's window is over) | Missed |
| Not yet | Upcoming |
| Unknown (an input the rule needs is missing) | the typical-date status, with `status_estimated` |

The plan therefore never shows Active while the feed is silent on that work, or the
reverse. The rules are triggered by live soil, GDD, calendar gates and the nitrogen guard,
which a 10 cm climatology cannot reproduce; the plan keeps its 10 cm dates as the
expected window and lets the rule say where the season actually stands.

The handler (`rule_states`, `backend/src/api/seasonal_plan.rs`) runs only when the plan
year is the lawn's current year: it takes the environmental summary (refreshed when
stale, as the feed does) and `recommendation_feed::all_with_history`, the dashboard's
feed with the owner's answers flagged but not removed, so a dismissed or addressed alert
still counts as Due: an answer silences an alert, it is not a log entry. When the summary
or the feed cannot be produced, every covered activity keeps its typical-date status with
`status_estimated` set. On a warm-season lawn only grub control has a covering rule.

Every other activity, and every activity in any other plan year, uses
`compute_status(completed, start, end, today)`: Completed if logged, else Missed if
`today > end`, Active if `today >= start`, otherwise Upcoming. `today` is the lawn's
local date from the app clock (`backend/src/clock.rs`), which
the handler passes to `build_seasonal_plan`. The same date picks the default `year`, the
years read and the settled judgment in the handler.

## Crossings Are Computed, Not Stored

Per request the handler reads the daily 10 cm series for `current_year − 10 ..=
current_year` from the weather record in one call, runs `analyze_year` on each year, and
judges which are settled. The result is memoized in memory for one hour and discarded
when the record's basis plan changes, the same rule as the timing windows' climate
record. Nothing derived is written to Postgres.

The tables `seasonal_threshold_crossings` and `seasonal_crossing_years` are dropped by a
migration.

When the record cannot answer (location not set, history still loading, a read error)
there are no crossings, and the plan is built from the other two sources.

## The Merge (`GET /api/v1/seasonal-plan`)

`backend/src/api/seasonal_plan.rs`, query `?year=` (default: server-local current year):

1. Compute the crossings (above), or take the memoized ones.
2. Load the default lawn profile (404 if none) and the applications dated in
   `[Jan 1 year, Jan 1 year+1)`.
3. `build_seasonal_plan(year, crossings, applications, data_years, grass)` → 10 cm
   activities.
4. **Timing swap.** `api::timing::plan_activities(state, profile, year)` returns the
   Timing windows' activities, or `None` when the weather record cannot answer, the climate
   record fails, or no window could be dated. When `Some`, every id in
   `timing::plan::owned_ids(grass)` is removed from the plan and the timing activities
   are appended. Removal goes by `owned_ids`, not by what came back, so a window the log
   has ruled out (seed vs. pre-emergent) leaves no activity and its 10 cm twin cannot
   reappear. When `None`, the 10 cm activities stay.
5. **Plant overlay.** Plants for the profile plus the plant-linked applications from
   Oct 1 of the prior year through the end of the latest window judged for the year
   (`plant_log_range`, which runs into the next year when a window wraps the new year;
   Landscape plants, PLANT-PLAN-005) go to `build_plant_activities(plants,
   applications, year, today)`, whose output is appended unchanged.
6. Sort all activities by `predicted_start` (stable) and return the plan with `today`,
   the server-local date its statuses were judged on.

Resulting activity sets:

| Condition | 10 cm activities | Timing activities |
|---|---|---|
| Windows available | `spring_nitrogen`, `spring_herbicide`, `grub_preventative`, `early_fall_fert`, `mid_fall_fert`, `winterizer`, `fall_herbicide` | `pre_emergent`, `fall_pre_emergent`, `fall_overseeding`, `core_aeration` (minus any the log blocks) |
| Windows unavailable | all ten 10 cm activities | none |

The timing activities' windows are typical opens → closes medians; for the season being
lived now their status follows the window's resolved state, evaluated from the same
inputs as the Timing page, forecast included (TIME-PLAN-007), so the plan's status
matches the window's state on the page. Both are the Timing windows component's contract
(`backend/src/logic/timing/plan.rs`).

Error shape: Postgres failures and an invalid `year` (no Jan 1 for it) propagate as the
standard `TurfOpsError` responses (500 / 400). Weather-record failures never fail the request.

## Calendar (`GET /api/v1/applications/calendar`)

`backend/src/api/calendar.rs`, query `?year=&month=` (defaults: server-local today):
returns `{ year, month, days }` where `days` maps `YYYY-MM-DD` → applications.
It selects the profile's applications whose `application_date` **or** `follow_up_date`
falls in the month (`get_applications_or_follow_ups_in_range`) and groups them by
`application_date`, so a follow-up for an application logged in an earlier month appears
under that earlier date key. Invalid month → 400; no profile → 404. The calendar
endpoint does not read the plan; the page fetches both.

## Pages

### Seasonal Plan (`frontend/src/pages/SeasonalPlan.tsx`)

- Year selector: the server's year −1, that year, +1. The first fetch omits `year` and
  the page adopts the returned `year`; later fetches pass `?year=`; a newer request
  aborts the previous one. Countdowns count from the plan's `today`. The browser's clock
  is never used: the lawn's local date judges every status, and
  a browser in another timezone, or near midnight, would otherwise count from a
  different day.
- Subtitle: "Predicted activity windows based on N years of soil temperature data"
  (N = `data_years_used`).
- Status summary chips (Active, Upcoming, Completed, Skipped, Missed) with symbol +
  label (● ○ ✓ – ✗; Skipped reads "Not needed").
- Turf / Plants / All filter, shown only when any activity has category
  `"Plant Maintenance"`.
- Vertical timeline of cards: status symbol + label, category, date range, name,
  description, a countdown ("Starts in N days", "N days remaining", "Window has passed",
  "Done", "Not needed this season"), confidence dots, and an expandable Details block
  (trigger, rate, products, notes, historical range). An estimated status reads
  "· estimate" after its label.
- Empty state distinguishes "no activities could be projected" from an empty filter.
- A failed load shows `describeError(e)` (Web UI's human-readable errors): a missing lawn
  profile points to Settings, a timeout says to try again; never a status code or JSON body.

### Calendar (`frontend/src/pages/Calendar.tsx`)

- Month grid with prev / next navigation, starting on the month the calendar endpoint
  returns when called without `year` and `month` (the server's local today, never the
  browser's clock); the plan is fetched per year and fails silently (the grid still
  shows applications). A failed calendar load shows `describeError(e)`: a missing lawn
  profile points to Settings, a timeout says to try again; never a status code or JSON body.
- Per day: filled dots for applications logged that day, ring dots for follow-ups due
  that day (both colored by application type), and one bar per plan activity whose
  `[predicted_start, predicted_end]` contains the day, colored by status. Plant
  activities draw as thin outlined bars. Days inside any activity window get a tinted
  background.
- Legend: Applications, Follow-up due, and the five status colors, each with its symbol
  and label.
- Selecting a day opens a detail panel: Applications, Follow-ups Due, Turf Activities
  (status badge with symbol and label, window, confidence, trigger, products, notes) and
  Plant Maintenance (status, window, description, notes). The calendar does not show
  `rate`.
- Status is never color alone: each bar's title and accessible name read "<name> —
  <symbol> <label>", with "(estimate)" when `status_estimated` is set.
- Both pages take status symbols, labels, colors, countdowns and month/year navigation
  from `frontend/src/components/plan/activityStatus.ts`.

## Degradation

| Failure | Effect |
|---|---|
| Location not set, or history still loading | No 10 cm activities and no timing swap; the plan has plant tasks only, and the page says the weather history is not available yet |
| The record cannot be read | The memoized crossings while under an hour old, else none; the plan still returns |
| 10 cm soil estimated for the location | Activities are built from estimated crossings; the plan says its typical dates rest on an estimate |
| Climate record / timing evaluation fails | 10 cm activities for pre-emergent, seeding and aeration remain; no `fall_pre_emergent` |
| No crossings and no timing | Plan has only plant tasks (or nothing); page shows the empty state |
| Plan request fails on Calendar | Applications still render, no bars |
| Conditions or feed unavailable for the current season | Rule-covered activities keep their typical-date status, marked `status_estimated` |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Source of pre-emergent, seeding and aeration on the plan | Timing windows' activities, swapped in by id | Keep the plan's own 10 cm versions; show both | One answer everywhere: the plan must agree with the Timing page and feed. Owner chose a dedicated timing page that feeds the plan rather than growing `seasonal_plan.rs`. |
| Removing 10 cm twins | Remove everything in `owned_ids(grass)` | Remove only the ids the timing side returned | A window the log rules out returns nothing; removing by returned ids would resurrect its 10 cm twin. |
| Fallback when timing is unavailable | Keep the 10 cm activities | Drop the activities | A weather-record outage should degrade, not blank the plan [inferred] |
| Aeration | Shares the fall seeding window | Its own 65 → 50 °F 10 cm window | Owner decision: aeration follows the fall seeding window. |
| Amounts in `ActivityDetails.rate` | lb N/1,000 sq ft on feedings, seed per 1,000 sq ft on seeding, "Label rate" (no number) on pesticides | No amounts at all; typical label rates for every product | Nutrient and seed amounts are agronomy, independent of the product, and the plan cannot express a feeding schedule without them. A pesticide's amount is set by its label, the legal authority, so the plan never states one (HLD non-goal "Label rates"). |
| Clock for "today" and the plan year | The app clock's local date | UTC date; a station's local date; browser date | Every agronomic date is a lawn-local calendar day, and one clock gives every component the same day (HLD key decision). |
| Where crossings live | Computed per request from the weather record, memoized one hour in memory | A Postgres table of crossings per year with settled markers | The record is in the same database, so a stored copy saves a query and adds a second source of truth that must be invalidated on a late revision, a newly mapped probe, a pinned station or a change of location. |
| Estimated years | Vote, when the estimate may date the threshold; the count is shown | Measured years only | Most lawns have no measured 10 cm soil at all. Scored against stations the estimate dates these thresholds within 3 to 5 days in a typical year, against 5 to 6 for the calendar. |
| Crossing signal | First 5-day-held run of a 7-row trailing mean of 10 cm daily means; runs starting right after a gap dropped | 5 cm (as Timing); one step across the threshold with no hold | These activities' thresholds are defined at 10 cm, and no 5 cm thresholds are defined for these activities [inferred]. A single step across dates a passing warm or cold spell, and a run after a hole dates a sensor outage; reading the record as Timing does keeps the two methods consistent. |
| Which years vote | At most 10 most recent complete (settled) years before the plan year | Every year read, including the plan year and the year in progress | A prediction must not include the outcome it predicts, and a partial year has only its early crossings, which biases late thresholds early. Ten years bounds the sample. |
| Settlement | A year votes once it is over on the lawn's calendar **and** the record holds its Dec 31 | Calendar alone | The record trails real time, so the calendar alone would let a year vote before its last days arrive and lose a late crossing (45 °F falling). |
| Mapping history onto the plan year | Month and day | Day-of-year ordinal | Ordinals shift every date after Feb 28 by a day across leap years, and ordinal 366 has no place in a non-leap year. |
| Status of rule-covered activities this season | From the covering agronomic rule's state | Date-based status from 10 cm medians | One answer everywhere: the plan must not say Active while the feed is silent, or the reverse; the rules see live soil, GDD and the nitrogen guard. |
| A feeding past the annual nitrogen target | Its own status, Skipped ("Not needed this season") | Treat it as rule-silent (Upcoming, then Missed); keep it Active | Missed reads as forgotten work and Active contradicts the feed's "skip this feeding"; not needing it is a distinct outcome. |
| Owner's answers and plan status | Ignored: an answered alert still counts as Due | Addressed → Completed | An answer silences an alert and expires; only the log records work done. |
| A covered activity the rule cannot judge | Typical-date status flagged `status_estimated` | Hide the status; Upcoming | The dates still carry information; the flag says it is the calendar's guess, as the timing windows label theirs. |
| Completion source | Turf applications only | Any application of the type | Plant work (a shrub insecticide, a plant feeding) does nothing for the lawn. |
| `fall_herbicide` and seeding | Hidden only by a seeding in the 60 days before the window, never when completed | Hidden by any seeding in the year | Only a seeding shortly before the window makes the herbicide unsafe for seedlings; a completed activity is history and stays visible. |
| Pages' clock | Server's date from the API (`today`, default `year` / month) | Browser clock | Statuses are judged on the server's local date; countdowns and the default year must agree with them. |
| Plant tasks | Overlaid into the same plan response | Separate endpoint | Calendar and plan show turf and plant work on one timeline with one fetch [inferred] |
| Calendar data | Applications endpoint returns logged + follow-up rows; page also fetches the plan | Server-side merged calendar | Keeps the calendar endpoint a plain log query; the plan is reused as-is [inferred] |

## Open Questions & Future Decisions

### Resolved

1. ✅ Pre-emergent, fall pre-emergent, fall seeding and core aeration come from the
   Timing windows; their 10 cm versions never appear alongside them.
2. ✅ The year in progress never votes; it is re-read with every computation.
3. ✅ `ActivityDetails.rate` carries agronomic amounts (lb N/1,000 sq ft, seed per
   1,000 sq ft) and the Seasonal Plan page shows them in its "Rate:" row; pesticide
   activities say only "Label rate".
4. ✅ 10 cm statuses, the default year and the years read use the lawn's local date.

### Deferred

1. **No single-flight on a memo miss.** Concurrent misses each read the record.
2. **Size limits.** `frontend/src/pages/Calendar.tsx` is over 650 lines and
   `SeasonalPlan.tsx` near 480; `backend/src/db/queries.rs` (shared) is over 600 lines.
3. **Stringly-typed contracts.** Activity ids are the merge key and `"Plant
   Maintenance"` is the plant discriminator; neither is an enum.
4. **Concurrent first loads.** Two requests filling the same unsettled year both run
   `replace_threshold_crossings`; the second insert can hit the `(year, threshold_name)`
   key, the error is only logged as a warning and that year is skipped for that request.

## References

- `backend/src/logic/seasonal_plan.rs` — thresholds, crossing detection, aggregation, 10 cm activities, status
- `backend/src/models/seasonal_plan.rs` — plan types
- `backend/src/api/seasonal_plan.rs` — the crossing computation and the merge
- `backend/src/api/calendar.rs` — calendar endpoint
- `backend/src/db/queries.rs` — `get_applications_or_follow_ups_in_range`
- `docs/intent/weather-data/weather-record/` (`fetch_daily_soil_temp_averages`), `docs/intent/weather-data/soil-estimate/` (`datable`)
- `backend/src/datasources/weather.rs` — `fetch_daily_soil_temp_averages` (Weather data access)
- `backend/src/logic/timing/plan.rs`, `backend/src/api/timing.rs` (`plan_activities`) — Timing windows contract
- `backend/src/logic/plant_maintenance.rs` — `build_plant_activities` (Landscape plants contract)
- `backend/src/mcp/tools/season.rs` — `seasonal_plan` MCP tool (consumer)
- `frontend/src/pages/SeasonalPlan.tsx`, `frontend/src/pages/Calendar.tsx`, `frontend/src/types/index.ts`
- `docs/timing-windows.md` — "Recommendations and the seasonal plan"
- `docs/high-level-design.md`
