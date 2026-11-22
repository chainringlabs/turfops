# Setup and Sources — EARS Specs

Design: [setup-sources-design.md](setup-sources-design.md). Prefix `SETUP`. Facets: `FLOW` (the setup flow), `LOC` (the location step), `PLAN` (what we found, pins and the forecast provider), `HA` (connecting Home Assistant), `MAP` (mapping sensors), `PAGE` (the Sources page), `API` (the endpoints), `CRED` (stored credentials), `FAIL` (failures).

"Setup" is the flow at `/setup`. "Input" is a weather input as the weather record defines it. "Basis plan" is the record's ranked list of sources for each input.

## The setup flow

- [ ] **SETUP-FLOW-001**: While the lawn profile has no location, every page whose content depends on weather shall show one notice, "Tell TurfOps where your lawn is", linking to setup and saying that dates use UTC until the location is set, in place of that content, and pages that do not depend on weather shall work unchanged.
- [ ] **SETUP-FLOW-002**: Setup shall offer five steps in order (location, what we found, your sensors, your lawn, loading history), shall save a step when the owner continues from it, and shall keep what was saved when the owner leaves partway.
- [ ] **SETUP-FLOW-003**: Setup shall require only the location step; the owner shall be able to skip every other step.
- [ ] **SETUP-FLOW-004**: When setup is opened from the Sources page for a step, it shall open on that step with the current values filled in.
- [ ] **SETUP-FLOW-005**: The loading-history step shall show, per source, the years held against the years wanted, and shall say which features work already and which wait for the history.

## The location step

- [ ] **SETUP-LOC-001**: The location step shall let the owner search by place name or postal code and choose from the candidate places, or type a latitude and longitude.
- [ ] **SETUP-LOC-002**: The location step shall state, beside the search box, that the typed text is sent to the place-search service.
- [ ] **SETUP-LOC-003**: Before saving a location, the step shall show the place name, the coordinates rounded as they will be stored, the timezone that follows from them, and the nearest soil station with its distance.
- [ ] **SETUP-LOC-004**: If the chosen or typed point is outside the contiguous United States, then the step shall refuse it and say that no data is available there yet.

## What we found, pins and the forecast provider

- [ ] **SETUP-PLAN-001**: The what-we-found step shall show, before anything is saved, the basis plan the weather record would build for the point: for each input, the source that will speak, its distance when it is a station, and what stands behind it, and shall say in words which inputs are measured, which are estimated and which have no source.
- [ ] **SETUP-PLAN-002**: The owner shall be able to pin, for an input that a station can supply, any station within 150 miles that reports that input, chosen from a list giving each station's network, distance, elevation difference from the lawn and years of record.
- [ ] **SETUP-PLAN-003**: When the owner pins or unpins a station in the step, the step shall show the basis plan again with the change.
- [ ] **SETUP-PLAN-004**: The step shall offer Open-Meteo, the National Weather Service and OpenWeatherMap as the forecast provider, with Open-Meteo chosen unless the owner changes it, and shall ask for a key when OpenWeatherMap is chosen and no key is stored.
- [ ] **SETUP-PLAN-005**: When the owner continues from the step, the system shall save the location, the pins and the forecast provider in one request (SETUP-API-012).

## Connecting Home Assistant

- [ ] **SETUP-HA-001**: The sensors step shall let the owner enter a Home Assistant address and access token and test them, reporting on success what answered and how many numeric sensor entities it has.
- [ ] **SETUP-HA-002**: While a Home Assistant connection is stored, the sensors step shall show it as connected with a way to change it, and go directly to the entity list.
- [ ] **SETUP-HA-003**: The system shall accept a Home Assistant address only when its scheme is `http` or `https`.

## Mapping sensors

- [ ] **SETUP-MAP-001**: The sensors step shall list Home Assistant's numeric sensor entities with friendly name, current value, unit and last update, placing first those whose device class is temperature, humidity, moisture or precipitation, and shall let the owner filter the list by text.
- [ ] **SETUP-MAP-002**: The step shall suggest, for an entity, humidity when its device class is humidity; soil moisture when it is moisture; rain as a counter when it is precipitation with state class "total increasing"; rain as an amount when it is precipitation otherwise; and no input when it is temperature.
- [ ] **SETUP-MAP-003**: The owner shall choose, for each sensor mapped, exactly one input: air temperature, humidity, rain as a counter or as an amount, or soil temperature or soil moisture at one of 5, 10, 20, 50 or 100 cm.
- [ ] **SETUP-MAP-004**: Before a mapping is saved, the step shall show the sensor's current value converted to the input's unit beside the value the lawn would otherwise use for that input, with that value's basis.
- [ ] **SETUP-MAP-005**: If the sensor's unit is not one its input accepts, or its newest reading is older than the age limit for its input (SYNC-AGE-001; rain has none), then the step shall say so and shall not save the mapping.
- [ ] **SETUP-MAP-006**: If the sensor's current value is outside the plausible range for its input, then the step shall say so and shall let the owner save the mapping.
- [ ] **SETUP-MAP-007**: If the owner maps a sensor to an input that already has a sensor, then the system shall ask for confirmation naming both sensors and, on confirmation, replace the earlier mapping.
- [ ] **SETUP-MAP-008**: The system shall refuse to map one Home Assistant entity to more than one input.
- [ ] **SETUP-MAP-009**: When a sensor is unmapped, the system shall keep the readings it has stored and return its input to the next source in the basis plan.
- [ ] **SETUP-MAP-010**: The owner shall be able to create a sensor that is not in Home Assistant by naming it and choosing its input, after which the step shall show the request that sends a reading to it; while `SENSOR_TOKEN` is not set, the step shall say that posted readings need it.
- [ ] **SETUP-MAP-011**: When the server starts with `HA_TEMPERATURE_ENTITY` or `HA_HUMIDITY_ENTITY` set and no sensor mapped, the system shall map those entities to air temperature and humidity.

## The Sources page

- [ ] **SETUP-PAGE-001**: The Sources page (`/sources`, in the navigation as "Sources") shall show, for each input, the basis in use with the shared basis label, the sources that stand behind it in order, and the newest value with the time it was observed.
- [ ] **SETUP-PAGE-002**: For an input whose basis is an estimate, the Sources page shall state the estimate's scored accuracy in plain words and any crossing it may not date, taking both from the soil estimate's published figures.
- [ ] **SETUP-PAGE-003**: For an input with no source, the Sources page shall say that nothing measures it at this location and what would.
- [ ] **SETUP-PAGE-004**: The Sources page shall list every source in use with its name, kind, the inputs it speaks for, its last successful fetch, its last failure with that failure's owner sentence, the history held against wanted, and its attribution, with failing sources first.
- [ ] **SETUP-PAGE-005**: The Sources page shall show the state of the Home Assistant connection (connected or not set up), the forecast provider, and whether posted readings are enabled, each with a way to test and change it where it can be changed.
- [ ] **SETUP-PAGE-006**: The Sources page shall show the lawn's place, coordinates and timezone, and before a change of location is saved shall warn that station and grid history for the old point will be replaced and reloaded and that sensor history is kept.
- [ ] **SETUP-PAGE-008**: The Sources page shall credit the timezone boundary data to OpenStreetMap contributors under the Open Database License.
- [ ] **SETUP-PAGE-007**: From each input on the Sources page, the owner shall be able to pin or unpin a station and map or unmap a sensor.

## The endpoints

- [ ] **SETUP-API-001**: `GET /api/v1/setup/places?q=` shall return the candidate places for the search text.
- [ ] **SETUP-API-002**: `GET /api/v1/sources/preview?lat=&lon=` shall return the basis plan the point would get and shall save nothing.
- [ ] **SETUP-API-003**: `GET /api/v1/sources` shall return the inputs with their basis plan, basis in use and accuracy; the sources with status, history and attribution; the connections; and the location.
- [ ] **SETUP-API-004**: `GET /api/v1/sources/stations?input=` shall return the stations within 150 miles of the lawn that report that input.
- [ ] **SETUP-API-005**: `PUT` and `DELETE /api/v1/sources/pins/{input}` shall pin a station for the input and remove the pin; a pin on a station that does not report the input shall be a 400 saying so.
- [ ] **SETUP-API-006**: `PUT /api/v1/sources/forecast-provider` shall set the forecast provider and, when the request carries one, its key.
- [ ] **SETUP-API-007**: `GET`, `PUT` and `DELETE /api/v1/sources/home-assistant` shall report whether a connection is set, set the address and token, and forget them; `POST /api/v1/sources/home-assistant/test` shall try the saved connection or the one given in the request and save nothing.
- [ ] **SETUP-API-008**: `GET /api/v1/sources/home-assistant/entities` shall return the numeric sensor entities with their suggested inputs.
- [ ] **SETUP-API-009**: `GET` and `POST /api/v1/sources/sensors`, `PUT` and `DELETE /api/v1/sources/sensors/{id}`, and `POST /api/v1/sources/sensors/check` shall list the mapped sensors, map one, change a mapping, unmap one, and return the comparison of SETUP-MAP-004 without saving.
- [ ] **SETUP-API-010**: After a write that changes a pin, a sensor mapping, the forecast provider or the Home Assistant connection, the system shall have the weather record rebuild its basis plan and wake the fetch schedule before it responds.
- [ ] **SETUP-API-012**: `POST /api/v1/setup/apply` shall save, all or none, whichever of a location, a set of pins and a forecast provider the request carries, and shall then have the weather record rebuild its basis plan once and wake the fetch schedule once, before it responds.

## Stored credentials

- [ ] **SETUP-CRED-001**: The system shall store a Home Assistant token or an OpenWeatherMap key entered in the app, and no endpoint shall return it, no log line shall contain it, and no error shall include it.
- [ ] **SETUP-CRED-002**: When the server starts and no Home Assistant connection has ever been stored, the system shall store `HA_URL` and `HA_TOKEN` as the connection when both are set; when no OpenWeatherMap key has ever been stored, it shall store `OWM_API_KEY` when set; it shall read those variables at no other time, and a connection or key the owner has forgotten shall not be restored from them.
- [ ] **SETUP-CRED-003**: When the owner forgets a connection, the system shall delete its stored address and secret.
- [ ] **SETUP-CRED-004**: The Sources page shall state that a credential entered in the app is stored in the application database and is therefore contained in backups of it.

## Failures

- [ ] **SETUP-FAIL-001**: If the place search fails, then the location step shall say "Place search is unavailable; enter coordinates instead" and shall still accept typed coordinates.
- [ ] **SETUP-FAIL-002**: If Home Assistant does not answer at the address, or refuses the token, then the test shall say which of the two happened.
- [ ] **SETUP-FAIL-003**: If a mapped entity is no longer found in Home Assistant, then the Sources page shall show its sensor as "not found in Home Assistant" with ways to unmap and to remap it.
- [ ] **SETUP-FAIL-004**: If the weather record cannot preview a plan because the station catalog is not yet loaded, then the what-we-found step shall say the station list is still loading and try again.
