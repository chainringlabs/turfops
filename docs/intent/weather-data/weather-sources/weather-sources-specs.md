# Weather Sources — EARS Specs

Design: [weather-sources-design.md](weather-sources-design.md). Parent: [weather-data](../weather-data-design.md). Prefix `WX-SRC`. Facets: `NORM` (units, sentinels and time), `REQ` (how requests are made), `CAD` (cadence), `USCRN`, `SCAN`, `GRID` (PRISM and NRCC through RCC-ACIS), `OM` (Open-Meteo), `HA` (Home Assistant), `POST` (posted sensor readings), `FC` (forecast providers), `OWM` (the forecast model and its day helpers, and the OpenWeatherMap provider), `MODEL` (shared units), `CFG` (settings), `ERR` (errors).

"Client" is the code that talks to one outside source. "Record units" are °F, millimetres, percent relative humidity and soil moisture as a fraction from 0 to 1. "Lawn-local" is the app's `Clock`, in the timezone of the lawn's location.

## Units, sentinels and time

- [ ] **WX-SRC-NORM-001**: Every client shall return temperatures in °F, precipitation in millimetres, relative humidity in percent and soil moisture as a fraction from 0 to 1, converting from the units the source declares.
- [ ] **WX-SRC-NORM-002**: Every client shall return as empty any value that is the source's missing marker, any temperature outside −80 to 150 °F, any relative humidity outside 0 to 100, any precipitation that is negative or above 300 mm in an hour, and any soil moisture outside 0 to 1, keeping the rest of the row.
- [ ] **WX-SRC-NORM-003**: Every client shall return as empty any value the source itself flags as failed quality control.
- [ ] **WX-SRC-NORM-004**: Every client shall stamp an hourly row with the UTC start of the hour it describes, and shall leave a daily row on the source's own day.

## How requests are made

- [x] **WX-SRC-REQ-001**: Every outside request shall carry a `User-Agent` naming TurfOps and its version, and shall carry no owner name, address or contact detail.
- [x] **WX-SRC-REQ-002**: Every outside request shall time out after 5 seconds to connect and 30 seconds in total, except a grid request spanning more than one year, which shall time out after 120 seconds in total.
- [x] **WX-SRC-REQ-003**: The clients shall make at most one request at a time to a host, with at least 1 second between requests to the same host and at least 2 seconds between requests to RCC-ACIS.
- [x] **WX-SRC-REQ-004**: When re-requesting a file it has fetched before, a client shall send `If-Modified-Since` with the time of the copy it holds, and shall treat a not-modified answer as a success that returns no rows.
- [x] **WX-SRC-REQ-005**: A client shall not retry a failed request; it shall return the failure to its caller.
- [ ] **WX-SRC-REQ-006**: A point query to an outside source shall send the lawn's coordinates and nothing else about the lawn or its owner.

## Cadence

- [x] **WX-SRC-CAD-001**: Each client shall state, for each product it serves, how often the source may have new data: hourly for USCRN and SCAN hourly data of the current year, the hourly analysis and the forecast; daily for the PRISM grid and for calibration stations' daily data; every 5 minutes for Home Assistant entities; monthly for station catalogs; never for a station-year that has ended.
- [x] **WX-SRC-CAD-002**: The NRCC grid client shall state that its data is wanted only for days the PRISM grid has returned no value for.

## USCRN

- [x] **WX-SRC-USCRN-001**: The USCRN catalog fetch shall return, for each station in the network's station list whose network is USCRN, USRCRN or Alabama-USRCRN, whose country is the United States, whose state is in the contiguous United States and whose operation is Operational, its WBAN number, name, state, latitude, longitude, elevation in feet, year commissioned and the stem of its data file names.
- [x] **WX-SRC-USCRN-002**: The USCRN hourly fetch shall return, for one station and year, each hour's average air temperature, maximum and minimum air temperature, precipitation, relative humidity, and soil temperature and soil moisture at 5, 10, 20, 50 and 100 cm.
- [x] **WX-SRC-USCRN-003**: The USCRN hourly fetch shall stamp each row one hour before the file's UTC time, which marks the end of the observed hour.
- [x] **WX-SRC-USCRN-004**: The USCRN hourly fetch shall return relative humidity as empty when its quality flag is not 0.
- [x] **WX-SRC-USCRN-005**: The USCRN daily fetch shall return, for one station and year, each day's air maximum, minimum and mean, precipitation, and soil temperature at 5 and 10 cm.
- [x] **WX-SRC-USCRN-006**: If the file for a station and year does not exist, then the USCRN client shall return an empty success.

## SCAN

- [x] **WX-SRC-SCAN-001**: The SCAN catalog fetch shall return, for each SCAN station in the contiguous United States, its station id, name, state, latitude, longitude, elevation in feet, fixed UTC offset, first date of record, and the hourly elements it reports with their depths.
- [x] **WX-SRC-SCAN-002**: The SCAN hourly fetch shall return, for one station and a span of at most one year, air temperature, relative humidity, precipitation, and soil temperature and soil moisture at 2, 4, 8, 20 and 40 inches as the 5, 10, 20, 50 and 100 cm depths, for the elements the station reports.
- [x] **WX-SRC-SCAN-003**: The SCAN hourly fetch shall convert each row's time from the station's fixed UTC offset to UTC.

## PRISM and NRCC grids

- [x] **WX-SRC-GRID-001**: The grid fetch shall return, for one point, one grid (PRISM or NRCC) and any span of days, each day's air maximum and minimum and precipitation, in one request.
- [x] **WX-SRC-GRID-002**: The grid fetch shall return the latitude and longitude of the grid cell the values are for.
- [x] **WX-SRC-GRID-003**: The grid fetch shall treat days at the end of the span that the grid returns no value for as not yet published and return the earlier days; if a day before the last day with a value has no value, then the grid client shall return an outage and no rows.

## Open-Meteo

- [x] **WX-SRC-OM-001**: The hourly-analysis fetch shall return, for one point, each past hour's air temperature, relative humidity and precipitation for up to 92 days back.
- [x] **WX-SRC-OM-002**: No Open-Meteo request shall ask for a soil temperature or soil moisture variable.
- [x] **WX-SRC-OM-003**: The hourly-analysis fetch shall return the elevation Open-Meteo reports for the point, in feet.
- [x] **WX-SRC-OM-004**: The place search shall return, for a typed name or postal code, the candidate places in the United States with their name, state, latitude and longitude, sending the typed text and nothing else.

## Home Assistant

- [x] **WX-SRC-HA-001**: When reading Home Assistant, the system shall read each mapped entity independently and return, for each whose state is a number, the value converted from the entity's own unit of measurement and the time the entity last reported: its `last_reported` time, or its `last_updated` time where Home Assistant gives no `last_reported`.
- [x] **WX-SRC-HA-002**: If a mapped Home Assistant entity's state is not numeric, then the system shall log a warning, return no reading for that sensor, and still return the other sensors' readings.
- [x] **WX-SRC-HA-003**: When a mapped entity's reported time (WX-SRC-HA-001) is not later than its sensor's previous reading, the system shall return no new reading for that sensor.
- [x] **WX-SRC-HA-004**: The Home Assistant entity listing shall return every entity in the `sensor` domain whose state is a number, with its entity id, friendly name, state, unit of measurement, device class, state class and reported time (WX-SRC-HA-001).
- [x] **WX-SRC-HA-005**: The Home Assistant probe shall report whether one authenticated request to the given address succeeded and, when it did not, whether the address did not answer or the token was refused.
- [x] **WX-SRC-HA-006**: For a sensor mapped to rain as a counter, the system shall return as the rain amount the increase in the entity's value since the previous reading, and zero when the value has decreased.
- [x] **WX-SRC-HA-007**: For a sensor mapped to rain as an amount, the system shall return the entity's value as the rain amount.
- [x] **WX-SRC-HA-008**: If a mapped Home Assistant entity reports a unit its sensor's input does not accept, or a value that once converted is outside that input's plausible range (WX-SRC-NORM-002), then the system shall log a warning naming the sensor and the problem, return no reading for that sensor, and still return the other sensors' readings.
- [x] **WX-SRC-HA-009**: When a sensor mapped to rain as a counter has no previous reading, the system shall keep its value as the counter's starting point and return no rain amount.
- [x] **WX-SRC-HA-010**: If the value of a sensor mapped to rain as a counter has risen more than 300 mm since its previous reading, then the system shall return zero as the rain amount and log a warning naming the sensor.
- [x] **WX-SRC-HA-011**: The system shall keep each sensor's previous reading, its converted value and its reported time, in the database with the sensor's mapping, so that WX-SRC-HA-003, WX-SRC-HA-006 and WX-SRC-HA-010 hold across a restart.
- [x] **WX-SRC-HA-012**: When a sensor's input or rain kind is changed, the system shall discard that sensor's previous reading.
- [x] **WX-SRC-HA-013**: If a mapped Home Assistant entity is not found in Home Assistant, then the system shall return no reading for that sensor and still return the other sensors' readings.
- [x] **WX-SRC-HA-014**: If a mapped Home Assistant entity's reported time is more than 5 minutes after the current time, then the system shall log a warning naming the sensor, return no reading for that sensor, and still return the other sensors' readings.

## Posted sensor readings

- [x] **WX-SRC-POST-001**: While `SENSOR_TOKEN` is set, the system shall accept `POST /api/v1/sensor-readings` with a body `{"readings": [...]}` of one or more readings, each with a sensor name (`sensor`), a value (`value`), a unit (`unit`) and optionally the time observed (`observed_at`), and shall require the token as a bearer token.
- [x] **WX-SRC-POST-002**: While `SENSOR_TOKEN` is not set, the system shall not mount `/api/v1/sensor-readings`.
- [x] **WX-SRC-POST-003**: If a request to `/api/v1/sensor-readings` lacks the bearer token or carries a wrong one, then the system shall answer 401 and store nothing.
- [x] **WX-SRC-POST-004**: If a posted reading names no mapped sensor, names more than one mapped sensor, names a sensor that takes its readings from Home Assistant, gives no unit or a value that is not a number, gives a time observed that is not an RFC 3339 time with a UTC offset, gives a unit its sensor's input does not accept, gives a value outside that input's plausible range (WX-SRC-HA-008), or is dated more than 5 minutes after the current time, then the system shall reject that reading with a message naming the sensor and the problem and shall still take the other readings in the request.
- [x] **WX-SRC-POST-005**: When a posted reading gives no time observed, the system shall take the time the request was received.
- [x] **WX-SRC-POST-006**: The system shall treat a posted reading for a sensor mapped to rain as WX-SRC-HA-006, WX-SRC-HA-007, WX-SRC-HA-009 and WX-SRC-HA-010 treat a Home Assistant reading.
- [x] **WX-SRC-POST-007**: The system shall match a posted reading's sensor name to a mapped sensor's name ignoring letter case and surrounding space.
- [x] **WX-SRC-POST-008**: The system shall take each sensor's readings in a request oldest first, whatever order they were sent in.
- [x] **WX-SRC-POST-009**: When a posted reading is dated at or before its sensor's previous reading, the system shall store nothing for it and report it as unchanged, not as rejected.
- [x] **WX-SRC-POST-010**: The system shall answer a request to `/api/v1/sensor-readings` with the readings stored, those unchanged and those rejected with their messages, with status 200 unless every reading in the request was rejected and 400 then.
- [x] **WX-SRC-POST-011**: When the system takes a posted reading, it shall record against its sensor's source a success when the reading was stored or unchanged, and otherwise a failure with the rejection's message.
- [x] **WX-SRC-POST-012**: The system shall log at INFO a posted sensor's first stored reading since the server started, and at WARN a rejected posted reading unless that sensor has had a rejection logged since it last stored a reading.
- [x] **WX-SRC-POST-013**: The system shall store an accepted posted reading whether or not the lawn has a location.
- [x] **WX-SRC-POST-014**: The system shall take the requests to `/api/v1/sensor-readings` one at a time.
- [x] **WX-SRC-POST-015**: If a request's body is not a JSON object whose `readings` is a list of at least one entry, then the system shall answer 400 and store nothing.

## Forecast providers

- [ ] **WX-SRC-FC-001**: The system shall fetch the forecast from the provider the owner has chosen: Open-Meteo by default, the National Weather Service, or OpenWeatherMap.
- [x] **WX-SRC-FC-002**: Every forecast point shall carry its slot's length in hours: 1 for Open-Meteo and the National Weather Service, 3 for OpenWeatherMap.
- [x] **WX-SRC-FC-003**: Where a provider gives no forecast of precipitation amount, precipitation probability, gust, feels-like temperature or cloud cover for a slot, the forecast point shall hold that quantity as empty, not zero; a slot for which the provider gives no temperature, relative humidity or wind speed shall not be returned.
- [x] **WX-SRC-FC-004**: If the chosen provider cannot be used because it needs a key and none is set, then the system shall fetch no forecast, shall not fetch one from another provider, and shall report the reason as that provider's status.
- [x] **WX-SRC-FC-010**: The OpenWeatherMap forecast shall be fetched for the point and with the key it is given, and shall read no point or key from a setting.
- [x] **WX-SRC-FC-005**: The Open-Meteo forecast shall return one point for each hour from the hour in progress to the end of the 16-day span, taking temperature, relative humidity, feels-like temperature, wind speed, cloud cover and condition from the values stamped at the hour's start and precipitation amount, precipitation probability and gust from the values stamped at its end.
- [x] **WX-SRC-FC-006**: The National Weather Service forecast shall return one point for each hour from the hour in progress, giving every hour of a span the value the grid forecast states for that span, except a precipitation amount, which it shall divide evenly across the hours of its span.
- [x] **WX-SRC-FC-007**: The National Weather Service forecast shall ask the service which forecast grid cell holds the point, sending the coordinates to four decimal places, and shall reuse the answer for later fetches for the same point.
- [x] **WX-SRC-FC-008**: A fetched forecast shall name the provider it came from.
- [x] **WX-SRC-FC-009**: A forecast point's condition shall be the provider's weather for the slot mapped to the shared conditions; for the National Weather Service, a slot with no weather forecast, or only a chance or slight chance of it, shall be Clouds when sky cover is at least 50%, Clear when it is lower, and Other when no sky cover is given.

## The forecast model and its day helpers

- [x] **WX-SRC-OWM-001**: The forecast shall carry every point its provider gives and daily summaries with high, low, mean RH, total precipitation (empty unless every slot of the day carries an amount), maximum probability over the slots that carry one (empty when none does), the condition covering the most hours, mean wind, maximum gust, and hours covered (the sum of the day's slot lengths); an OpenWeatherMap point's precipitation shall be its rain plus snow in mm.
- [x] **WX-SRC-OWM-002**: When grouping forecast points into daily summaries, the system shall use each point's lawn-local date, never a UTC date or the forecast location's offset.
- [x] **WX-SRC-OWM-003**: The forecast shall treat a daily summary as a full day when it covers at least 18 hours, and `full_days()` shall return only full days, for every consumer that needs a daily mean or judges a whole day.
- [x] **WX-SRC-OWM-004**: `next_hours(n)` shall include every point up to now + n hours, including the current slot; `next_days(n)` every summary up to today + n, partial days included; `rain_expected_within(h, mm)` shall report rain when the amounts the points in the next h hours carry total at least mm or any of those points has probability ≥ 0.5; `max_temp_next_days(n)` the highest daily high over `next_days(n)`.
- [x] **WX-SRC-OWM-005**: No error, log line or response produced by the forecast reader shall contain the OpenWeatherMap API key.
- [x] **WX-SRC-OWM-006**: If the forecast request fails, then the reader's error message shall be plain language naming the forecast as unavailable, keeping upstream response bodies for the log only.

## Shared units

- [x] **WX-SRC-MODEL-003**: Every backend weather contract shall carry precipitation in millimetres; only the UI converts to inches.

## Settings

- [x] **WX-SRC-CFG-002**: The system shall use the Home Assistant address and token stored in the app, and shall treat Home Assistant as not configured when none is stored.
- [x] **WX-SRC-CFG-003**: The system shall use the OpenWeatherMap key stored in the app; a stored key shall not by itself choose OpenWeatherMap as the forecast provider.
- [x] **WX-SRC-CFG-004**: The station and grid clients shall take the station or point as an argument and shall read no setting.

## Errors

- [ ] **WX-SRC-ERR-001**: If a request fails in transport, returns a non-success status, or returns a body that does not parse, then the client shall return an outage of its source whose owner sentence names the source and what depends on it, and shall write the URL, status, body or parse error to the log only.
- [ ] **WX-SRC-ERR-002**: No response, log line or error shall contain a stored credential or one read from the environment.
