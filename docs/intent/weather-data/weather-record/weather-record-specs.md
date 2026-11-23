# Weather Record — EARS Specs

Design: [weather-record-design.md](weather-record-design.md). Parent: [weather-data](../weather-data-design.md). Prefix `WX-REC`. Facets: `STORE` (what is stored and how it is written), `PLAN` (station catalog and basis plan), `RES` (resolving a value), `BASIS` (basis in the contracts), `API` (query contracts), `FILL` (backfill, what is due, status), `FAIL` (outages).

"Record" is the stored weather for the lawn. "Input" is one of: soil temperature at a depth, soil moisture at a depth, hourly air temperature and humidity, daily air maximum and minimum, rain. "Local day" is a calendar day in the lawn's timezone (the app's `Clock`), daylight-saving aware. "Covers" is defined by WX-REC-RES-002. "Plan" is the basis plan: the ranked list of sources for each input.

## What is stored and how it is written

- [x] **WX-REC-STORE-001**: The record shall keep every stored weather value in a row that belongs to exactly one source, and shall store no row that combines values from more than one source.
- [x] **WX-REC-STORE-002**: The record shall store temperatures in °F, precipitation in millimetres, relative humidity in percent and soil moisture as a fraction from 0 to 1, as handed over by the source clients, and shall convert no unit on read except where a contract states its own unit.
- [x] **WX-REC-STORE-003**: The record shall key an hourly row by its source and the UTC start of its hour, and a daily row by its source and the source's own day.
- [x] **WX-REC-STORE-004**: When rows are written for a source and hour or day the record already holds, the record shall replace the held row with the new one, nulls included.
- [x] **WX-REC-STORE-005**: The record shall compute every daily value of an hourly source on read, by grouping that source's hours into local days, and shall store no daily copy of an hourly source.
- [x] **WX-REC-STORE-006**: When a sensor reading arrives, the record shall store the reading as received and recompute that sensor source's hourly row for the reading's hour from all of that hour's readings: the mean for temperature, humidity and soil moisture, and the total for rain.
- [x] **WX-REC-STORE-007**: The record shall delete stored sensor readings older than 7 days and shall keep the sensor source's hourly rows without limit.
- [x] **WX-REC-STORE-008**: The record shall want, for each source in the plan, this span: for a station that is the basis for an input, the grids at the lawn's point and the soil estimate, 15 complete calendar years before the current year plus the current year; for the hourly analysis, from 93 days before the installation first had a location; for calibration stations and calibration grid points, the span the soil estimate asks for.

## Station catalog and basis plan

- [x] **WX-REC-PLAN-001**: The record shall hold a catalog of the USCRN and SCAN stations in the contiguous United States with, for each, its network, the network's station id, name, state, latitude, longitude, elevation in feet and the inputs it reports.
- [ ] **WX-REC-PLAN-002**: The record shall rebuild the plan when the lawn's location is set or changed, when a sensor is mapped or unmapped, when a station is pinned or unpinned, and when a catalog refresh changes a station that is in the plan.
- [x] **WX-REC-PLAN-003**: The plan shall rank, for each input, the owner's sensor mapped to that input first, then stations, then the input's estimate source.
- [x] **WX-REC-PLAN-004**: For soil temperature and soil moisture at each depth, the plan shall include the nearest USCRN station within 50 miles of the lawn that reports the input, followed by the nearest SCAN station within 50 miles that reports it. A station whose reported inputs the catalog does not yet know shall be taken to report every input until its own data has shown what it reports.
- [x] **WX-REC-PLAN-005**: For hourly air temperature and humidity, the plan shall include the nearest USCRN station within 10 miles of the lawn that reports them, followed by the nearest SCAN station within 10 miles that reports them.
- [x] **WX-REC-PLAN-006**: For daily air maximum and minimum and for rain, the plan shall include no station unless the owner has pinned one.
- [x] **WX-REC-PLAN-007**: While the lawn's elevation is known, the plan shall not include, as a default station, a station whose elevation differs from it by more than 1,000 feet, the lawn's elevation being the one the hourly analysis reports for its point.
- [x] **WX-REC-PLAN-013**: While the lawn's elevation is not known, the plan shall be built without the elevation rule, and when the elevation becomes known the record shall rebuild the plan.
- [x] **WX-REC-PLAN-008**: Where the owner has pinned a station for an input, the plan shall use that station in place of the default stations for that input, whatever its distance or elevation.
- [x] **WX-REC-PLAN-009**: The record shall measure the distance from the lawn to a station as great-circle miles and shall keep it with the station's source.
- [x] **WX-REC-PLAN-010**: The plan's estimate source shall be the soil estimate for soil temperature at 5 cm and 10 cm, the hourly analysis for hourly air temperature and humidity, the PRISM grid followed by the NRCC grid for daily air, and the PRISM grid followed by the hourly analysis for rain; soil temperature at 20, 50 and 100 cm and soil moisture at every depth shall have no estimate source.
- [x] **WX-REC-PLAN-011**: While the lawn has no location, the plan shall be empty.
- [x] **WX-REC-PLAN-012**: When asked to preview a point, the record shall return the plan it would build for that point and the stations near it with their distance, elevation difference from the point and inputs reported, and shall write nothing.

## Resolving a value

- [x] **WX-REC-RES-001**: For an input on a complete local day, the record shall return the value from the highest-ranked source in the plan that covers that day, provided no higher-ranked source is pending for that day (WX-REC-RES-010), and shall return no value otherwise.
- [x] **WX-REC-RES-002**: An hourly source shall cover a local day for an input when it holds at least 12 non-null hourly values of that input on that day; a daily source shall cover a local day when it holds a value assigned to that day.
- [x] **WX-REC-RES-003**: For the live reading and the local day in progress, the record shall take each input from the highest-ranked source in the plan whose newest non-null value of that input is no older than the age limit its caller gives for that kind of source, except that rain shall be taken only from an hourly source, a day's total saying nothing about the day in progress.
- [x] **WX-REC-RES-004**: The live reading shall give, for each input, the speaking source's newest non-null value and the time that value was observed; where the speaking source is daily, the value shall be its newest day's value and the time shall be that day.
- [x] **WX-REC-RES-005**: For the daily disease inputs, the record shall take a local day's air temperature and humidity from one source: the highest-ranked source in the plan for hourly air and humidity that holds at least 12 hours on that day with both values present.
- [x] **WX-REC-RES-006**: Where the soil estimate is the resolved source for days that follow or lie between days resolved from a sensor or a station, the record shall return the soil estimate's anchored value for those days, in every contract and in the live reading, and the same value for a day whatever span is asked for, looking for the measured days that anchor it up to 366 days either side of that span.
- [x] **WX-REC-RES-007**: The record shall assign a PRISM or NRCC maximum and minimum for day D to local day D, a PRISM or NRCC precipitation amount for day D to local day D − 1, and a soil-estimate value for day D to local day D.
- [x] **WX-REC-RES-008**: While PRISM has published no precipitation assigned to a local day, the record shall resolve that day's rain total from the hourly analysis, and when PRISM publishes it, from PRISM.
- [x] **WX-REC-RES-009**: The record shall resolve no value for the lawn from a source whose role is calibration.
- [x] **WX-REC-RES-010**: A sensor or station shall be pending for a local day when its newest non-null value of the input is no older than the age limit for its kind of source (SYNC-AGE-001) and falls on that day or an earlier one and the source does not cover that day; while a source is pending for a day, the record shall resolve that day from no lower-ranked source, except for rain, which no pending source holds back; a grid, the hourly analysis and the soil estimate shall never be pending.

## Basis in the contracts

- [x] **WX-REC-BASIS-001**: Every value a record contract returns shall be accompanied by its basis: the kind (sensor, station or estimate), the source's name, the distance in miles when the source is a station, and whether it is approximate.
- [x] **WX-REC-BASIS-002**: The record shall report a value from the hourly analysis as an estimate that is approximate, and a value from a grid or from the soil estimate as an estimate that is not approximate.
- [x] **WX-REC-BASIS-003**: A contract that returns a series shall return, for each input in the series, the basis as spans of the series' successive days on the same basis, each with its first and last day; a day with no value shall not end a span.
- [x] **WX-REC-BASIS-004**: A contract that returns one figure over a period shall return the basis of the newest day in the period and shall mark the figure as mixed when the period drew on more than one basis.
- [x] **WX-REC-BASIS-005**: The live reading shall carry one basis for each input it fills.

## Query contracts

- [x] **WX-REC-API-001**: No record contract shall publish a "today" of its own; each shall take the lawn's date or instant from its caller.
- [x] **WX-REC-API-002**: `fetch_range(start, end)` shall return hourly readings newest first, each input resolved per local day, and for a local day on which an input's source is daily, one reading of that input at local noon.
- [x] **WX-REC-API-003**: `fetch_summary(now)` shall return the live reading, the means of 10 cm soil temperature, air temperature and humidity and the rain total over the 168 hours ending at `now`, and the soil trend; a local day whose source is daily shall contribute its value to a mean for each of its hours inside those 168, and to the rain total in proportion to those hours.
- [x] **WX-REC-API-004**: The soil trend shall compare the mean 10 cm soil temperature of the 24 hours ending at the newest value with that of the 24 hours before: more than +2 °F is Rising, less than −2 °F is Falling, otherwise Stable; where the newest value is a daily one it shall compare the newest day with the day before; where either period has no value, or the two periods rest on different sources, it shall be Unknown.
- [x] **WX-REC-API-005**: `fetch_daily_soil_temp_averages` shall return, for each complete local day in the span asked for that has one, the resolved 10 cm soil temperature, in ascending date order.
- [x] **WX-REC-API-006**: `fetch_daily_paired_averages` shall return, for each local day that has both, the resolved daily air mean and the resolved 10 cm soil temperature, in ascending date order.
- [x] **WX-REC-API-007**: The daily air mean shall be the mean of the day's hours where the daily air source is hourly, and half the sum of the maximum and minimum where it is a grid.
- [x] **WX-REC-API-008**: `fetch_daily_gdd(start, end)` shall return, for each local day with a resolved daily maximum and minimum, those two values and `gdd50`, the greater of zero and their mean minus 50 °F.
- [x] **WX-REC-API-009**: `fetch_gdd_ytd(today)` shall return the sum of `gdd50` from January 1 of `today`'s year through `today`, and nothing when no day in that span has a value.
- [x] **WX-REC-API-010**: `fetch_daily_disease_inputs(start)` shall return, per local day from `start`, the count of usable hours, air mean, minimum and maximum, humidity mean, hours at humidity of 90 % or more, leaf-wetness hours (humidity of 90 % or more, or precipitation above zero in that hour from the same source), the day's rain total from the rain basis (zero where the rain basis has no value for the day), the peak Magnus dew point, and whether the day has an hour in the dawn hours and in the afternoon hours its caller supplies, with temperatures in °C; a usable hour is one with both air temperature and humidity.
- [x] **WX-REC-API-011**: `fetch_climate_record(start)` shall return, per local day from `start`, the resolved 5 cm soil temperature, the resolved daily air minimum and mean, and `gdd50`, keeping a day that has only some of them with the others empty.
- [x] **WX-REC-API-012**: The primary soil moisture shall be the 10 cm value, else 5 cm, else 20 cm.

## Backfill, what is due, status

- [x] **WX-REC-FILL-001**: The record shall keep, for each source and product, the span it wants, the span it holds, and the times of the last attempt, the last success and the last failure.
- [x] **WX-REC-FILL-002**: `due(now)` shall report, for each fetched source in the plan, a current fetch when its product's cadence says new data may exist, and a backfill fetch for each calendar year of its wanted span it does not yet hold, with every current fetch before any backfill and backfill newest first; it shall report no fetch for a sensor or for the soil estimate.
- [x] **WX-REC-FILL-003**: When a fetch succeeds, the record shall write its rows and extend the held span in one step; when a fetch fails, the record shall record the failure and leave the held span unchanged.
- [x] **WX-REC-FILL-004**: `due(now)` shall report, with each station fetch, a re-read of that station's last 3 days, and with each daily grid fetch, a re-read of the grid's last 7 days.
- [x] **WX-REC-FILL-005**: `status()` shall report, for each source in use, its name and kind, the inputs it is in the plan for, its last success and last failure, and the span held against the span wanted.
- [x] **WX-REC-FILL-006**: When the plan is rebuilt, the record shall stop reporting fetches for sources that left it and shall start each new source with an empty held span.
- [x] **WX-REC-FILL-007**: When asked to clear out, the record shall delete every source that is not in the plan, with its rows, once every fetched source in the plan holds its wanted span, and shall delete no sensor and no calibration source.
- [x] **WX-REC-FILL-008**: `due(now)` shall report a network's station catalog for refresh when the record holds no station of that network or last refreshed it 30 or more days ago, and the record shall answer the same question on its own for a lawn that has no plan yet.

## Outages

- [x] **WX-REC-FAIL-001**: While the lawn has no location, every record contract that answers for the lawn shall return the location-not-set outage.
- [x] **WX-REC-FAIL-002**: While any source in the plan for 5 cm soil temperature, 10 cm soil temperature or daily air does not yet hold its wanted span, `fetch_climate_record`, `fetch_daily_soil_temp_averages` and `fetch_daily_paired_averages` shall return the history-still-loading outage with the years held and wanted, the years held being the fewest complete years any of those sources holds; a sensor, which has no history to load, shall not be counted.
- [x] **WX-REC-FAIL-003**: While history is still loading, `fetch_latest`, `fetch_summary`, `fetch_range`, `fetch_daily_gdd`, `fetch_gdd_ytd` and `fetch_daily_disease_inputs` shall answer from the days held.
- [x] **WX-REC-FAIL-004**: If a source in the plan cannot be fetched, then the record shall keep answering from the rows it holds for that source.
- [x] **WX-REC-FAIL-005**: The record shall return a missing value as empty, and shall neither substitute zero for it nor carry an earlier value forward in its place.
