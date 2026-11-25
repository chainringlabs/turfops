# Environmental Views — EARS Specs

Design: [environmental-views-design.md](environmental-views-design.md). Parent: [weather-data](../weather-data-design.md). Prefix `WX-VIEW`. Facets: `API` (`/environmental`, `/environmental/refresh`, `/historical`), `BASIS` (how a basis is shown, on every page), `STATUS` (the shape of source status), `PAGE` (the Environmental page, gauges and trend charts).

"Basis" is what a weather value rests on, as the weather record returns it: a station with its distance, the owner's sensor, or an estimate for the location, which may be approximate.

## Endpoints

- [x] **WX-VIEW-API-001**: When `GET /api/v1/environmental` is called, the system shall return data sync's environmental summary; `POST /api/v1/environmental/refresh` shall ask data sync to refresh current data and return the summary that results.
- [x] **WX-VIEW-API-002**: When `GET /api/v1/historical` is called with `range` 7d, 30d or 90d (default 7d), the system shall return the weather record's resolved hourly series for that span in ascending time (10 cm soil temperature, air temperature, RH, 10 cm moisture, precipitation ≥ 0) with the basis spans of each series, leaving a missing value out of its series rather than filling it with zero.
- [x] **WX-VIEW-API-003**: `GET /api/v1/historical` shall return every hour for 7d, and for 30d and 90d shall group hours into 6-hour and 24-hour time buckets, summing precipitation and averaging the other series over the hours present.
- [x] **WX-VIEW-API-004**: `GET /api/v1/historical` shall include a running GDD series for the range, accumulated from the weather record's daily `gdd50` from Jan 1 and reset at each Jan 1.
- [x] **WX-VIEW-API-005**: `GET /api/v1/historical` shall take "today" and the GDD accumulation year from the lawn's local date (the app's `Clock`).
- [x] **WX-VIEW-API-006**: If `GET /api/v1/historical` is called with any other range, then the system shall return 400; if the weather record returns the location-not-set or history-still-loading outage, 503 with that outage's sentence.
- [x] **WX-VIEW-API-007**: The environmental summary shall carry `current_bases`, one entry for each input a live reading can fill giving its request name, its standing (`InUse`, `NoCurrentValue` or `NoSource`) and, while `InUse`, its basis and observation time, and `week_bases`, keyed by the name of each 7-day figure that has a value, giving its basis and whether it is mixed.

## How a basis is shown

- [ ] **WX-VIEW-BASIS-001**: Every page that shows a weather-dependent value shall show its basis with one shared label component: "Measured · *station name*, *N* mi" for a station, "Your sensor · *sensor name*" for the owner's sensor, "Estimated for your location" for an estimate, with "· approximate" added when the basis is approximate and "· mixed" when a period drew on more than one basis.
- [x] **WX-VIEW-BASIS-002**: The basis label shall carry a mark as well as its words (a solid dot for a station, a solid square for a sensor, a hollow dot for an estimate), never distinguishing a basis by color alone, and shall link to the Sources page everywhere but on the Sources page itself.
- [x] **WX-VIEW-BASIS-003**: A chart of a weather series shall draw measured stretches as a solid line and estimated stretches as a dotted line of the same hue, mark each change of basis with a vertical marker, list the spans in its caption, and give its table view a basis column; a dashed line shall mean forecast only.
- [x] **WX-VIEW-BASIS-004**: Where an input has no source in the lawn's basis plan, a page shall show "No source for this location" with a link to the Sources page in place of the value; where the input has a source but no current value, it shall show `--`.

## Source status

- [x] **WX-VIEW-STATUS-001**: Source status shall be a list with one entry per source in use, each giving the source's id, name and kind, whether its most recent scheduled fetch succeeded (empty while it has never been fetched), the times of its last success and last failure, the owner sentence for a failure newer than its last success, the inputs the plan ranks it for, and, for a fetched source, the history held against the history wanted.
- [x] **WX-VIEW-STATUS-002**: Source status shall list a source that is in the basis plan but is the basis for no input with an empty list of inputs.

## Environmental page

- [x] **WX-VIEW-PAGE-001**: The Environmental page shall load current conditions and the soil outlook together, hide the outlook section if its request fails, and poll every 30 seconds only while the tab is visible, re-fetching when it becomes visible again.
- [x] **WX-VIEW-PAGE-002**: When the owner clicks "Refresh Now", the Environmental page shall call the force-refresh endpoint and show the result.
- [x] **WX-VIEW-PAGE-003**: The Environmental page shall show "Last updated" and the soil trend, four gauges (Soil Temp 10 cm 30–100°F, warn 75 / critical 85; Ambient Temp 0–110°F, 85 / 95; Humidity 0–100%, 80 / 90; Soil Moisture 10 cm 0–50%, 40 / 45) each with its basis label and its number (`--` when missing), four 7-day summary cards, a soil profile table with one row per depth that has a source, and forecast cards headed by the provider's name for the first 7 daily summaries with the rest shown on request (high/low, RH, precipitation or "not forecast", maximum probability).
- [x] **WX-VIEW-PAGE-004**: The Environmental page shall show precipitation in inches, converting from the API's millimetres only in the UI.
- [x] **WX-VIEW-PAGE-005**: The Environmental page shall offer a 7d / 30d / 90d selector over six trend charts (10 cm soil temperature, air temperature, RH, soil moisture, precipitation, GDD), each showing "No data available" when its series is empty.
- [x] **WX-VIEW-PAGE-006**: If the historical request fails, then the Environmental page shall show a plain-language error for the trend section and clear the charts rather than keep the previous range's data.
- [x] **WX-VIEW-PAGE-007**: The Environmental page's trend charts shall draw reference lines only at air 85°F "Heat stress", RH 80% "Humid (80%)" and moisture 0.10 "Drought", with no pre-emergent or crabgrass line.
- [ ] **WX-VIEW-PAGE-008**: The Environmental page shall show when the soil value in use was observed (for an estimate, the day it is for), and, when the summary's `soil_stale` flag is set, the stale-soil notice of WX-VIEW-PAGE-013.
- [x] **WX-VIEW-PAGE-013**: The stale-soil notice, wherever it is shown, shall say that the weather station has not published new soil readings and since what date, that the network publishes late from time to time and the owner has nothing to do, that TurfOps keeps checking, and that soil readings are hidden and soil-based advice is paused meanwhile; it shall be presented as a notice, not as an error.
- [x] **WX-VIEW-PAGE-009**: The Environmental page shall mark a forecast card "partial day" when its daily summary is not a full day.
- [x] **WX-VIEW-PAGE-010**: The soil moisture gauge shall also warn on dry soil: amber below 0.15 and red below 0.10, alongside its wet warnings.
- [x] **WX-VIEW-PAGE-011**: Each gauge in a warning or critical state shall say so in text (for example "High", "Dry") beside its colored bar, never by color alone.
- [x] **WX-VIEW-PAGE-012**: The Environmental page shall show, below its content, the name of every source it drew on with the credit that source's terms require.
