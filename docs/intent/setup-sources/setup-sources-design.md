---
parent: high-level-design
prefix: SETUP
---

# Setup and Sources

## Context and Design Philosophy

TurfOps needs one fact to describe a lawn's weather, its location, and can use several more the owner may have: their own sensors, a preferred station, a forecast provider. This component is where the owner supplies them and where they can see what the app made of them. It owns the first-run **setup flow**, the **Sources page**, the API both are built on, and the storage of the owner's weather choices and of credentials entered in the app.

It decides nothing about weather. The weather record chooses stations and resolves the basis, the source clients talk to the outside, the soil estimate publishes its accuracy, and data sync reports status. This component asks them and shows the answers, and writes the owner's choices where they read them.

Principles:

- **Nothing to look up.** The owner never types a station number, an entity id or a timezone. The app finds the candidates and the owner picks.
- **Show before saving.** A location shows the sources it would get before it is saved; a sensor shows its current reading beside the value it would replace. A wrong choice is visible while it is still cheap.
- **Only the location is required.** Every other step can be skipped and returned to. A lawn with a location and nothing else is fully working.
- **The Sources page is the standing answer to "where does this number come from".** Every basis label in the app links to it.

## When Setup Appears

- The lawn profile has no location: every weather-dependent page shows one notice, "Tell TurfOps where your lawn is", linking to setup and saying that dates use UTC until the location is set, in place of its weather content. The application log, inventory, soil tests and landscape plants work as usual.
- Setup can be reopened at any time from the Sources page. It opens on the step asked for, with current values filled in.

## The Setup Flow

Route `/setup`, with the step in the address (`/setup?step=location`, `?step=found`, …) so the Sources page can open the step it means. Five steps; each saves when the owner continues, so leaving partway keeps what was done. Location and What we found are one save: the location is not stored until the owner continues from What we found, because the plan is built from the location, the pins and the provider together. Opened on a step, setup fills in what is stored: the location, the pinned stations, the forecast provider.

### 1. Location

- **Search** by place name or postal code. The typed text goes to the place-search client (weather sources) only when the owner presses Search, never while typing; the step says so beside the box. Results list the place, state and coordinates.
- **Or type** latitude and longitude.
- A point outside the contiguous United States is refused with the reason: no data is available there yet.
- Before saving, the step shows the place name, the coordinates rounded as they will be stored, the timezone that follows from them, and the nearest soil station with its distance, as a check that the right place was found.
- When the lawn already has a location and the owner picks a different point, the step warns that station and grid history for the old point will be replaced and reloaded, and that sensor history is kept.

### 2. What we found

The basis plan the weather record would build for this point, counting the sensors already mapped, one row per input: the source that will speak, its distance if it is a station, and what stands behind it. In words under the table: what is measured, what is estimated, and what has no source here (soil moisture without a station).

- **Pin a different station** for any input: a list of the stations that report it within 150 miles, with network, distance, elevation difference from the lawn, and years of record, opened in place under the input's row. Choosing one shows the same table again with the change; a pinned input can be unpinned the same way.
- **Forecast provider:** Open-Meteo unless changed. Choosing OpenWeatherMap asks for its key when none is stored; when one is, the step says so and offers to replace it.
- Continuing saves the location, the pins and the provider in one request. The basis plan is built once, from all three, and only then does the fetch schedule start, so history is never downloaded for a station the owner has just replaced.

### 3. Your sensors (optional)

**Connect Home Assistant.** Address and a long-lived access token, then Test, which makes one real request and reports what answered and how many sensor entities it has. Save stores the connection, and is offered only once a Test of the same address and token has answered, so a mistyped token is never stored. When a connection is already stored, the step shows it as connected, with Change, and goes straight to the list. Forget removes the stored connection after a confirmation saying that its sensors stop being read; their mappings and history are kept.

**Pick and map.** Numeric sensor entities are listed with friendly name, current value, unit and last update; those whose device class is temperature, humidity, moisture or precipitation come first, and a search box filters the rest. Map opens a form under the entity: the name (the friendly name to start with), the input (the suggestion to start with), and for rain whether it is a counter or an amount.

| Entity looks like | The app suggests | The owner confirms |
|---|---|---|
| Humidity | Humidity | Nothing more |
| Temperature | Nothing | Air or soil; for soil, the depth |
| Moisture | Soil moisture | The depth |
| Precipitation, state class "total increasing" | Rain, as a counter | That it is a counter |
| Precipitation, otherwise | Rain, as an amount | That it is an amount |

Soil depth is chosen from the record's five depths; the owner picks the one nearest their probe.

**Check.** Before a mapping is saved the step shows the sensor's current value, converted, beside the value the lawn would otherwise use for that input, with its basis: "Your sensor 61 °F · Estimated for your location 58 °F". A value outside the plausible range for the input, a unit the input does not accept, or a reading older than the input's age limit (data sync: 1 hour for air and humidity, 6 hours for soil; rain has none) is called out, and the last two prevent saving.

**Sensors that are not in Home Assistant.** The owner names a sensor and its input; the step then shows the exact request to send readings to it, addressed to this server and with `$SENSOR_TOKEN` standing for the token, which the app never shows. It needs `SENSOR_TOKEN` to be set, and says so when it is not.

**Mapped sensors.** The step lists the sensors mapped, each with its input and way in, and Unmap. Unmapping keeps the sensor's history, so it asks for no confirmation. Replacing a mapped sensor is confirmed in the step itself, naming both sensors.

**Rules.** One sensor per input: mapping a second to the same input replaces the first after a confirmation naming both. One entity maps to one input. No two mapped sensors share a name, letter case and surrounding space aside, because a posted reading names its sensor; a sensor being replaced gives up its name to the one replacing it. Unmapping returns the input to its station or estimate and keeps the sensor's history.

### 4. Your lawn

The lawn profile form (applications), the same form Settings shows, filled in from the stored profile: grass type, size, soil type, irrigation. Saving it never changes the location, which only the location step sets.

### 5. Loading history

Progress of the backfill per source: years held against years wanted. While any source is still loading, the step says what works already (current conditions, the forecast, disease risk for recent days) and what waits for the history (typical dates on the Timing page, the seasonal plan); once everything is held, it says so. The step reads the progress when it opens and again when the owner asks (Check again); it does not poll, since a backfill takes minutes and the Sources page shows the same progress afterwards. Finishing offers the dashboard, with the Sources page beside it.

## The Sources Page

Route `/sources`, in the navigation as "Sources".

**By input.** One card per input the record resolves:

| Shown | From |
|---|---|
| The basis in use now, with the shared basis label | The record's basis plan and live reading |
| What stands behind it, in order | The basis plan |
| Newest value and when it was observed | The live reading |
| For an estimate: its scored accuracy in plain words ("typically within 2 to 3 days of a measured station for soil-temperature crossings; in one year in five, off by 12 days or more"), and what it may not date | The soil estimate's published figures |
| For soil moisture with no source: that nothing measures it here and what would (a probe) | The basis plan |
| Change: pin a station, map or unmap a sensor | Steps 2 and 3 of setup, opened on that step |

Inputs are grouped as air (temperature, humidity, rain, the daily high and low), soil temperature by depth, and soil moisture by depth. Values are shown as the rest of the app shows them: °F, percent for humidity and soil moisture, inches for rain (the newest hour's amount). Each value carries the time it was observed in the lawn's timezone and how long ago that was. The estimate's accuracy reads, for example, "Daily values typically within 1.8 °F of a measured station. Crossing dates typically within 2 to 3 days; one year in five, off by 12 days or more. Not dated by the estimate: 70 °F falling." The range covers the crossings scored at that depth under the calibration the estimate was fitted on that it may date; the crossings it may not date are named. Inputs with no source are not given a card each: one line per group names them and what would measure them ("Nothing measures soil moisture at 20, 50 or 100 cm here; a probe you map as a sensor would"; for soil temperature, a station within 50 miles as well). Rain's newest value is the newest hour's amount, which is what the record holds. The daily error is worded as "typically within", not by its statistic. The basis label on this page does not link, since the page it would link to is this one. Connections and location show their state only until the setup steps they open exist: Home Assistant connected or not set up, the forecast provider's name, posted readings on or off with what turns them on; the place, the coordinates to four decimals, the timezone and the OpenStreetMap credit. The page loads once and does not poll. While the lawn has no location the page says so in place of the inputs, since none has a source.

**By source.** A table of every source in use: name, kind, the inputs it speaks for, last successful fetch, last failure with the owner sentence, history held against wanted, and its attribution line. A source that is failing is at the top; the rest follow by kind (your sensors, stations, grids, the hourly analysis, the estimate). History reads "Complete" once everything wanted is held, "Loading: 2019–2026 of 2011–2026" while it arrives, and is left blank for a sensor, which keeps no history to download. The inputs a source speaks for are listed in the page's order, soil depths together ("Air temperature, humidity, soil temperature at 5, 10 and 20 cm"). A Home Assistant sensor whose entity Home Assistant no longer has carries its sensor's id (`missing_sensor`), and its row offers Unmap (the history is kept) and Remap (the sensors step).

**Connections.** Home Assistant (connected or not set up), the forecast provider with when its forecast was last fetched or, while it is failing or cannot be used, why, and whether posted readings are enabled. Home Assistant has Test, which tries the stored connection in place, and Change, which opens Your sensors; the forecast provider has Change, which opens What we found; posted readings say what turns them on. A line under them says that a token or key entered in the app is stored in its database, so the database's backups contain it.

**Location.** The place, coordinates and timezone, with Change, and the credit the timezone boundary data requires (© OpenStreetMap contributors, ODbL). Changing it warns that station and grid history for the old point will be replaced and reloaded, and that sensor history is kept.

## API

All under `/api/v1`. Reads return the current state; writes return the state after the change.

| Method | Path | Purpose |
|---|---|---|
| GET | `/setup/places?q=` | Candidate places for a search |
| GET | `/sources/preview?lat=&lon=` | The basis plan the point would get, without saving anything. It asks the hourly analysis for the point's elevation first, so the preview drops the same stations the saved plan will |
| POST | `/setup/apply` | Save a location, pins and a forecast provider together; any of the three may be omitted |
| GET | `/sources` | Inputs with basis plan, live basis and accuracy (the estimate's daily error, and each crossing scored at its depth with its median and one-in-five error in days and whether the estimate may date it); sources with status, history, attribution and, for a sensor Home Assistant no longer has, `missing_sensor`; connections, including whether posted readings are enabled; location |
| GET | `/sources/stations?input=` | Stations that report the input within 150 miles of the lawn |
| PUT / DELETE | `/sources/pins/{input}` | Pin a station for an input; remove the pin |
| PUT | `/sources/forecast-provider` | Choose the provider; carries the key when the provider needs one |
| GET / PUT / DELETE | `/sources/home-assistant` | Whether a connection is set and where from; set the address and token; forget them |
| POST | `/sources/home-assistant/test` | Try the connection, saved or as given in the request |
| GET | `/sources/home-assistant/entities` | Numeric sensor entities with suggested inputs |
| GET / POST | `/sources/sensors` | The mapped sensors; map one |
| POST | `/sources/sensors/check` | The comparison shown before saving a mapping |
| PUT / DELETE | `/sources/sensors/{id}` | Change a mapping; unmap |

The location is a field of the lawn profile (applications). Setup saves it through `/setup/apply`; `PUT /profile` also accepts it, and a point changed there rebuilds the plan and wakes the schedule as setup does.

**Where a step leads.** Walked through from the location step, each step goes on to the next when the owner saves or skips it, and the last step ends at the dashboard, with the Sources page offered beside it. What we found has no Skip: it is where the location is saved, and saving it with what was found unchanged is the skip. Your sensors, Your lawn and Loading history each have Skip. Opened from the Sources page on one step, setup returns there once that step is saved or left. After a save of the location and what we found, the Sources page says the new sources start downloading now and that history can take several minutes.

**After any write** that changes what the record should use (a pin, a mapping, the provider, the Home Assistant connection) the record rebuilds its basis plan and the fetch schedule is woken, before the response.

**Errors** are owner sentences: an address that does not answer ("Home Assistant did not answer at that address"), a token it refuses ("Home Assistant refused the token"), a place search that fails ("Place search is unavailable; enter coordinates instead"), a pin on a station that does not report the input.

## Storage

| Table | Holds |
|---|---|
| `weather_setting` | The forecast provider. No row means the default, Open-Meteo. (The installation's fetch minute and the lawn's elevation are kept with the lawn profile.) |
| `sensor` | Name, way in (Home Assistant entity or posted), entity id, input, depth, rain kind, when mapped and unmapped, and its previous reading (weather sources) |
| `station_pin` | Input and the pinned station |
| `service_credential` | Service (Home Assistant, OpenWeatherMap), address, secret, when set |

**Credentials are write-only.** A secret is accepted by a PUT, used by its client, and never returned by any endpoint, written to a log, or included in an error. Reads report only that one is set. Forgetting a connection deletes its address and secret.

**One source.** The stored row is the only connection the app uses. `HA_URL` and `HA_TOKEN` together, and `OWM_API_KEY`, are copied into it on a start that finds no row for that service, and are not read again; forgetting a connection in the app is not undone by a restart: forgetting deletes the address and the secret and keeps the emptied row, which is the mark that the service was once set.

**First run.** `HA_TEMPERATURE_ENTITY` and `HA_HUMIDITY_ENTITY`, when set and no sensor is mapped, become mappings to air temperature and humidity.

## Failure and Degradation

| Failure | Effect |
|---|---|
| Place search unavailable (the service fails, the network is down, or the request times out) | The step says so in the one sentence, whatever the cause; coordinates can still be typed |
| Home Assistant does not answer or refuses the token | The Test result says which; nothing is saved by Test |
| Home Assistant connection lost after sensors are mapped | The mappings stay; the sensors' inputs fall to the next basis after data sync's age limit; the Sources page shows the failure |
| An entity disappears from Home Assistant | Its sensor's row in the sources table says it was not found in Home Assistant and offers Unmap and Remap |
| A network's station list is not loaded yet | The preview or station list loads every network's list the catalog holds no station of first: setup runs before the lawn has a location for the schedule to act on. If one cannot be loaded, step 2 shows the station network's outage sentence and offers to try again; a failed save offers nothing extra, since Save is the way to try it again |
| No stations within 150 miles for a pin | The list is empty and says so; the estimate remains |

## Decisions & Alternatives

| Decision | Chosen | Alternatives Considered | Rationale |
|---|---|---|---|
| Where the owner's weather choices are made | In the app, with a guided flow | Environment variables; a configuration file | HLD: the choices depend on what the app can find (stations by distance, a Home Assistant's entities) and must change without a restart. |
| Required steps | Location only | Require sensors or a profile before the app works | Half the value arrives with the location alone; every further step is an improvement the owner can make later. |
| Entering a location | Place search, or typed coordinates | A map to click; the browser's location | A map needs a tile service, one more outside dependency and one that sees every pan. Browser location needs a secure origin, which a self-hosted app on a home network usually lacks. |
| Home Assistant credentials | Entered in the app and stored write-only; environment values are imported once | Environment only; environment overriding the app | HLD decision: a new owner should not edit a configuration file and restart before setup can find their sensors. An override would need a precedence rule, a "fixed" state on the page and a refusal when the owner tries to change it; a one-time import carries an existing installation over with none of that. |
| Secret at rest | Stored as given in the application database | Encrypted with a key from the environment | A key in the environment restores the requirement to edit configuration first, and protects only against someone who has the database but not the host. Database backups therefore contain the secret, which the page states. |
| Mapping a sensor | The owner picks from discovered entities; the app suggests the input from device class and state class | Automatic mapping by device class; typing entity ids | Device class cannot tell air from soil or give a depth, and a wrong automatic mapping would feed a garage thermometer into fertilizer advice. Typing ids is the lookup the flow exists to remove. |
| Check before saving | Show the sensor beside the value it replaces; block a wrong unit or a reading older than its input's age limit | Save and let the owner notice | The comparison is the one moment a mis-mapped sensor is obvious. |
| Sensor names | Unique among mapped sensors, ignoring case, checked when mapping and renaming | Unique per way in; a unique index | A posted reading finds its sensor by name, and two sensors called "Garden probe" would make the owner guess which one is meant on the Sources page too. A database index would fail its migration on an installation that already holds two such names; the handler's check refuses new clashes and leaves old ones for the owner to rename. |
| One sensor per input | Yes | Average several sensors | An average of two instruments is a series neither recorded, and hides the one that is wrong. |
| Pinning range | Stations within 150 miles | Only within the cut-off; any station | The owner may reasonably prefer a farther station in the same valley; past 150 miles a neighbour's soil dates are no better than the calendar. |
| Where source status and accuracy are shown | One Sources page, linked from every basis label | On Settings; on each feature page | One place answers "where does this come from" for every number; feature pages stay about their feature. |
| Location change | Allowed, with a warning that station and grid history reloads | Fixed after setup | People move, and a first guess can be wrong. |

## Open Questions & Future Decisions

### Deferred

1. **The app has no login.** Anyone who can reach it can change the location, repoint Home Assistant or replace a token. They cannot read a stored secret. This is the app's existing exposure, widened by what can now be changed.
2. **The Home Assistant address is fetched by the server.** An owner-entered address is the intended use; the same field lets anyone with access make the app request an address on its network. Only `http` and `https` are accepted.
3. **Uncalibrated soil-moisture probes.** The check compares units, not calibration; a raw percentage will be accepted and drive irrigation advice.
4. **Home Assistant areas** are not shown in the entity list; the states API does not carry them.
5. **No bulk import** of mappings for an owner with many probes.

## References

- `docs/high-level-design.md`: the setup and Sources goals, the configuration decision.
- `docs/intent/weather-data/weather-record/`: the basis plan and its preview, stations, `status()`.
- `docs/intent/weather-data/weather-sources/`: place search, the Home Assistant client and its probe, posted readings, attribution.
- `docs/intent/weather-data/soil-estimate/`: the accuracy figures shown.
- `docs/intent/weather-data/environmental-views/`: the shared basis label.
- `docs/intent/data-sync/`: source status, backfill progress, waking the schedule.
- `docs/intent/applications/`: the lawn profile, which holds the location.
- `docs/intent/web-ui/`: routing, navigation, error presentation.
