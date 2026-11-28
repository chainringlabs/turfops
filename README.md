# TurfOps

A containerized web application for tracking lawn care activities and providing data-driven agronomic recommendations. Built with a Rust/Axum backend serving a React SPA frontend. It ships as a container image; `docker-compose.yml` is an example stack (app + PostgreSQL).

## Screenshots

![TurfOps dashboard](docs/screenshots/dashboard.png)
<p align="center"><em>Dashboard — live soil and ambient conditions, GDD, per-disease risk, seeding/pre-emergent status, nitrogen budget, alerts, and recent applications</em></p>

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/disease-risk.png" alt="Dollar spot disease risk detail page"></td>
    <td width="50%"><img src="docs/screenshots/timing-windows.png" alt="Seeding and pre-emergent timing windows page"></td>
  </tr>
  <tr>
    <td align="center"><em>Disease Risk — 11-day dollar spot trend and outlook with FRAC-rotation-aware fungicide programs</em></td>
    <td align="center"><em>Seed &amp; Pre-Em Timing — windows from the lawn's own 5 cm soil temperature and freeze record</em></td>
  </tr>
</table>

## Features

- **Application Tracking**: Log fertilizer, pre-emergent, fungicide, mowing, and other lawn treatments
- **Environmental Data**: Soil temperature and moisture, air, humidity and rain for the lawn's location, from nearby stations, daily grids, an hourly analysis, a soil-temperature estimate and your own sensors; every value says what it rests on (Station / Sensor / Estimate)
- **Setup and Sources**: Set the lawn's location, see which stations and sources it gets, pin a different station, choose the forecast provider and map your own sensors, all in the app
- **Smart Recommendations**: 10 agronomic rules, plus the disease-risk models and the seeding/pre-emergent timing windows, provide data-driven alerts for optimal treatment timing
- **Disease Risk**: Per-disease risk from published models (brown patch, dollar spot, Pythium blight, gray leaf spot, red thread) — each with a Low/Moderate/High/Severe tier, an 11-day trend and outlook, contributing factors, a "how this is calculated" breakdown, and preventative/curative guidance that knows what you've already sprayed
- **Seeding & Pre-Emergent Timing**: Fall/spring/dormant seeding and spring/fall pre-emergent windows from the lawn's 5 cm soil temperature (a nearby station, your sensor, or the soil estimate), its freeze dates and GDD — typical dates with their year-to-year spread, where this season actually stands, and seed-vs-pre-emergent conflicts from your application log. See [Seeding & Pre-Emergent Timing](#seeding--pre-emergent-timing).
- **Calendar View**: Visualize application history and seasonal plan activity windows with colored indicators
- **Seasonal Plan Integration**: Calendar overlays predicted activity windows from the seasonal plan alongside actual applications
- **Landscape Maintenance** *(optional)*: Track shrubs, trees, and perennials alongside turf. Enter a plant by common or scientific name and get a homeowner-level care plan (pruning windows, fertilizing, mulching) that overlays Calendar, Seasonal Plan, and Recommendations. Powered by an LLM through OpenRouter and cached per plant.
- **FRAC Rotation**: Fungicide resistance management — the suggested class for each disease avoids the one you used last, and a logged application counts as protection for its residual window
- **Background fetching**: a schedule inside the app keeps the weather record current (your sensors every 5 minutes, stations and the forecast hourly); pages read the record, each value labeled with what it rests on. **Refresh Now** asks for a pass at once

## Architecture

```
┌──────────────────────────────────────────────────────┐
│  Docker Compose                                       │
│                                                       │
│  ┌────────────────────────────────────────────────┐  │
│  │  turfops-app (port 3000)                        │  │
│  │  ┌─────────────┐   ┌────────────────────────┐  │  │
│  │  │ React SPA   │   │ Axum API Server        │  │  │
│  │  │ (static)    │◄──│  /api/v1/* endpoints   │  │  │
│  │  └─────────────┘   │  10 rules + disease    │  │  │
│  │                     │  weather fetch sched.  │  │  │
│  │                     └───────────┬────────────┘  │  │
│  └─────────────────────────────────┼───────────────┘  │
│                                    │                   │
│  ┌─────────────────┐               │                   │
│  │ PostgreSQL 18   │◄──────────────┘                   │
│  │ (app data)      │                                   │
│  └─────────────────┘                                   │
└───────────────────────────────────────────────────────┘
        │                    │                │
  Weather services     Home Assistant    Forecast provider
  (USCRN, SCAN, PRISM, (your sensors)    (Open-Meteo, NWS
   NRCC, Open-Meteo)                      or OpenWeatherMap)
```

## Tech Stack

- **Backend**: Rust + Axum + sqlx (PostgreSQL)
- **Frontend**: React 19 + TypeScript + Vite
- **Database**: PostgreSQL 18 (app data)
- **Weather data**: fetched by the app from public weather services into PostgreSQL
- **Deployment**: container image; example Docker Compose stack

## Docker Image

Pre-built Docker images are published to GitHub Container Registry on every push to `main`.

```bash
docker pull ghcr.io/chainringlabs/turfops:latest
```

Tagged releases are also available by version (e.g., `ghcr.io/chainringlabs/turfops:1.0.0`).

## Quick Start

### Prerequisites

- Docker and Docker Compose
- (Optional) Home Assistant, for your own sensors (air, humidity, rain, soil temperature or moisture)
- (Optional) OpenWeatherMap API key, only if you choose it as the forecast provider (Open-Meteo and the National Weather Service need none)
- (Optional) [OpenRouter](https://openrouter.ai) API key to enable the Landscape Maintenance feature

### 1. Configure

You can either use the pre-built image from GHCR or build from source.

**Option A: Use the pre-built image (recommended)**

```bash
# Download docker-compose.yml and .env.example
curl -O https://raw.githubusercontent.com/chainringlabs/turfops/main/docker-compose.yml
curl -O https://raw.githubusercontent.com/chainringlabs/turfops/main/.env.example
cp .env.example .env
```

Update `docker-compose.yml` to use the pre-built image instead of building locally — replace the `build:` block under the `app` service with:

```yaml
image: ghcr.io/chainringlabs/turfops:latest
```

**Option B: Build from source**

```bash
git clone git@github.com:chainringlabs/turfops.git
cd turfops
cp .env.example .env
```

Edit `.env` with your values (see [Environment Variables](#environment-variables) below).

### 2. Start the Application

```bash
docker compose up -d
```

This starts two containers:
- **app** — TurfOps web application on port 3000
- **db** — PostgreSQL 18 database with persistent volume

### 3. Open in Browser

Navigate to `http://localhost:3000`. On first open TurfOps asks where the lawn is. Setup then walks through **Location → What we found** (the stations and sources the location gets, and the forecast provider) **→ Your sensors → Your lawn → Loading history**. Weather history downloads in the background; current conditions and the forecast are ready within minutes.

### 4. Stop

```bash
docker compose down
```

Data persists in the `turfops_data` Docker volume. To fully reset, add `-v`:

```bash
docker compose down -v
```

## Environment Variables

All configuration is done through environment variables. When running with Docker Compose, set these in a `.env` file in the project root.

### Database

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_HOST` | App PostgreSQL hostname | `localhost` |
| `DATABASE_PORT` | App PostgreSQL port | `5432` |
| `DATABASE_NAME` | App database name | `turfops` |
| `DATABASE_USER` | App database user | `turfops` |
| `DATABASE_PASSWORD` | App database password | **required** |
| `DB_PASSWORD` | Password used by the PostgreSQL container (Docker Compose). **Required** — compose refuses to start without it | — |
| `DB_MAX_CONNECTIONS` | Maximum database connection pool size | `10` |

> **Note**: When using Docker Compose, `DATABASE_HOST`, `DATABASE_PORT`, `DATABASE_NAME`, and `DATABASE_USER` are pre-configured in `docker-compose.yml`. You only need to set `DB_PASSWORD` in your `.env` to change the database password (it flows to both the PostgreSQL container and the app's `DATABASE_PASSWORD`).

### Lawn Profile

These set the default lawn profile created on first startup. They can be changed later via the Settings page (and the location in Setup).

| Variable | Description | Default |
|----------|-------------|---------|
| `LAWN_NAME` | Display name for your lawn | `Main Lawn` |
| `LAWN_GRASS_TYPE` | Grass species | `TallFescue` |
| `LAWN_USDA_ZONE` | USDA hardiness zone | `7a` |
| `LAWN_SOIL_TYPE` | Soil type | `Loam` |
| `LAWN_SIZE_SQFT` | Lawn area in square feet | `5000` |
| `LAWN_IRRIGATION_TYPE` | Irrigation system type | `InGround` |
| `LAWN_LATITUDE`, `LAWN_LONGITUDE` | The lawn's location (contiguous United States). Also gives a location to an existing profile that has none; otherwise set it in Setup | *(empty)* |

Valid grass types: `KentuckyBluegrass`, `TallFescue`, `PerennialRyegrass`, `FineFescue`, `Mixed` (a cool-season blend), `Bermuda`, `Zoysia`, `StAugustine`

Valid soil types: `Clay`, `ClayLoam`, `Loam`, `SiltLoam`, `SandyLoam`, `Sandy`

Valid irrigation types: `InGround`, `Hose`, `None`

The defaults shown are the example `docker-compose.yml`'s; the app itself sets no lawn size.

### Weather

TurfOps fetches its own weather into PostgreSQL for the lawn's location, set in the app's setup: nearby NOAA USCRN and USDA SCAN stations, the PRISM and NRCC daily grids, an hourly analysis, a soil-temperature estimate, and your own sensors. There is nothing to mount or configure. `DATALAKE_ROOT`, `WEATHER_SILVER_PATH`, `WEATHER_GOLD_PATH`, `NOAA_STATION_WBANNO` and `HA_TEMPERATURE_UNIT` no longer have any effect; each is named once in the startup log if set.

### Home Assistant

Connect Home Assistant in **Setup → Your sensors** (or **Connect** on the Sources page) and map any of its numeric sensors to an input: air temperature, humidity, rain, soil temperature at 5–100 cm, or soil moisture. The address and token are stored in the app; the Sources page shows each sensor's status, and a sensor Home Assistant no longer has can be unmapped or remapped there. The variables below only seed a fresh database: they are copied in once, on a first run that finds no connection (or no sensor) ever stored, and the app's settings win after that.

| Variable | Description | Default |
|----------|-------------|---------|
| `HA_URL` | Home Assistant base URL, copied into the app once | *(empty)* |
| `HA_TOKEN` | Long-lived access token, copied into the app once | *(empty)* |
| `HA_TEMPERATURE_ENTITY` | Mapped once as "Patio temperature" (air) on a first run with no sensors | `sensor.temp_humidity_sensor_temperature` |
| `HA_HUMIDITY_ENTITY` | Mapped once as "Patio humidity" on a first run with no sensors | `sensor.temp_humidity_sensor_humidity` |

A token or key entered in the app is stored in TurfOps's database, so backups of that database contain it.

To generate a long-lived access token: Home Assistant → Profile → Long-Lived Access Tokens → Create Token.

### Forecast provider

The forecast drives the forecast-based rules (rain delay, heat stress warnings, optimal application windows) and supplies today's remaining hours plus the outlook for disease risk. It is fetched hourly for the lawn's location from the provider chosen in setup (step 2, or **Change** beside Forecast on the Sources page): Open-Meteo by default, the National Weather Service, or OpenWeatherMap. Only OpenWeatherMap needs a key; it is entered in setup and stored in the app.

| Variable | Description | Default |
|----------|-------------|---------|
| `OWM_API_KEY` | OpenWeatherMap API key, copied into the app on a first run that finds none stored; it does not choose OpenWeatherMap | *(empty)* |

`OWM_LATITUDE`, `OWM_LONGITUDE` and `OWM_ENABLED` are no longer read; when one is set, startup logs a line saying so. Sign up for a free OpenWeatherMap key at [openweathermap.org](https://openweathermap.org/api).

### OpenRouter (Optional — Landscape Maintenance and Product Identification)

Enables plan generation on the **Landscape** page and product identification on the **Inventory** page. Landscape generates a homeowner-level maintenance plan for each plant you add (pruning windows, fertilizing, mulching, deadheading, winter protection). Plans are generated once per plant through an LLM on [OpenRouter](https://openrouter.ai) and cached in Postgres, so there is no recurring per-view cost — only on plant creation or a manual "Regenerate plan" click.

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENROUTER_API_KEY` | OpenRouter API key | *(empty — feature disabled if not set)* |
| `OPENROUTER_MODEL` | Model identifier | `anthropic/claude-haiku-4.5` |
| `OPENROUTER_ENABLED` | Enable/disable the feature | `true` |
| `OPENROUTER_BASE_URL` | API base URL | `https://openrouter.ai/api/v1` |

> **Feature flag behavior**: The **presence of `OPENROUTER_API_KEY` is the feature flag** — there is no separate toggle. `OPENROUTER_ENABLED` is a second kill-switch you can flip without rotating the key. When disabled:
>
> - The `/landscape` page still loads, but adding a plant or regenerating a plan returns a clear `503 — OpenRouter not configured` error.
> - The `/inventory` page keeps working: products are saved by hand (you pick the category) and "Generate profile" returns the same 503.
> - **Existing cached plans stay visible.** Plant rows you added earlier still appear on the Landscape page, and their maintenance windows still overlay Calendar, Seasonal Plan, and Recommendations. Only *new* plan generation and *regenerating* existing plans are blocked.
> - All turf features continue to work unchanged.

### MCP Server (Optional — Ask Claude About Your Lawn)

TurfOps can serve a read-only [Model Context Protocol](https://modelcontextprotocol.io) server at `/mcp` (streamable HTTP, in the same process), so an LLM client such as Claude can pull the lawn's snapshot, recommendations, conditions and application log as tools and answer questions grounded in them. It is stateless with JSON responses; no tool writes anything.

| Variable | Description | Default |
|----------|-------------|---------|
| `MCP_TOKEN` | Bearer token clients must send (`Authorization: Bearer …`); at least 32 characters (`openssl rand -hex 32`) | *(empty — `/mcp` not mounted)* |

Tools: `lawn_snapshot` (call first — profile, conditions, full recommendation feed, recent applications, timing windows, disease headlines, nitrogen totals), `lawn_profile`, `current_conditions`, `weather_history`, `sources` (where each value comes from), `recommendations`, `timing_windows`, `seasonal_plan`, `gdd`, `soil_temp_outlook`, `nitrogen_budget`, `disease_risk`, `disease_detail`, `applications`, `soil_tests`, `soil_test_advice`, `products`, `shopping_list`, `plants`. Resources: `turfops://docs/agronomy` ([docs/agronomy-methods.md](docs/agronomy-methods.md)) and `turfops://docs/timing-windows`. `GET /api/v1/health` reports `mcp: "enabled" | "disabled"`. Try it with `npx @modelcontextprotocol/inspector` against `http://localhost:3000/mcp` and the bearer header.

### Posted sensor readings

Any hub or script can send readings from sensors that are not in Home Assistant. Map the sensor in the app under a name and an input, then post to `/api/v1/sensor-readings`:

```bash
curl -X POST http://localhost:3000/api/v1/sensor-readings \
  -H "Authorization: Bearer $SENSOR_TOKEN" -H "Content-Type: application/json" \
  -d '{"readings": [{"sensor": "Garden probe", "value": 61.2, "unit": "°F", "observed_at": "2026-11-24T15:00:00Z"}]}'
```

`observed_at` is optional (default: when the request arrives) and must carry a UTC offset. Units: °F or °C; mm or in; % for humidity; % or a fraction for soil moisture. The answer lists the readings `stored`, `unchanged` (already held, so resending is safe) and `rejected` with the reason; it is 200 unless every reading was rejected.

| Variable | Description | Default |
|----------|-------------|---------|
| `SENSOR_TOKEN` | Bearer token senders must use; at least 32 characters (`openssl rand -hex 32`) | *(empty — the route is not mounted)* |

### Server

| Variable | Description | Default |
|----------|-------------|---------|
| `SERVER_HOST` | Bind address | `0.0.0.0` |
| `SERVER_PORT` | HTTP port | `3000` |
| `STATIC_DIR` | Path to frontend static files | `/app/static` (in container) |
| `RUST_LOG` | Log level (`error`, `warn`, `info`, `debug`, `trace`) | `info` |

### Example `.env` File

```env
# Database (DB_PASSWORD flows to both PostgreSQL container and app)
DB_PASSWORD=my_secure_password
# These are pre-configured in docker-compose.yml but can be overridden:
# DATABASE_HOST=db
# DATABASE_PORT=5432
# DATABASE_NAME=turfops
# DATABASE_USER=turfops

# Lawn profile
LAWN_NAME=Front Yard
LAWN_GRASS_TYPE=TallFescue
LAWN_USDA_ZONE=7a
LAWN_SOIL_TYPE=ClayLoam
LAWN_SIZE_SQFT=8000
LAWN_IRRIGATION_TYPE=InGround

# Home Assistant
HA_URL=http://192.168.1.50:8123
HA_TOKEN=your_long_lived_access_token_here
HA_TEMPERATURE_ENTITY=sensor.temp_humidity_sensor_temperature
HA_HUMIDITY_ENTITY=sensor.temp_humidity_sensor_humidity

# OpenWeatherMap key (optional; choose the provider in setup)
OWM_API_KEY=your_api_key_here

# OpenRouter (optional — enables the Landscape Maintenance feature)
OPENROUTER_API_KEY=your_openrouter_key_here
# OPENROUTER_MODEL=anthropic/claude-haiku-4.5
# OPENROUTER_ENABLED=true

# MCP server for LLM clients (optional; at least 32 characters, e.g. `openssl rand -hex 32`)
# MCP_TOKEN=your_generated_token_here

# Posted sensor readings from hubs and scripts (optional; at least 32 characters)
# SENSOR_TOKEN=your_generated_token_here

# Logging
RUST_LOG=info
```

## Data Sources

| Source | Data Provided | Connection |
|--------|--------------|------------|
| **Weather record** | Hourly soil temperature (5/10/20/50/100 cm) and moisture from NOAA USCRN and USDA SCAN stations; daily air and rain from the PRISM and NRCC grids; hourly air, humidity and rain from Open-Meteo's analysis; a soil-temperature estimate where no station measures | Fetched by the app for the lawn's location and kept in PostgreSQL |
| **Your sensors** | Any input you map: air, humidity, rain, soil temperature or moisture | Home Assistant (read every 5 minutes; connection stored in the app) or posted readings (`SENSOR_TOKEN`) |
| **Forecast** (Open-Meteo, National Weather Service or OpenWeatherMap) | Hourly or 3-hourly forecast (temp, rain, humidity, wind) — also fills in today and the outlook for disease risk | Provider chosen in setup; OpenWeatherMap's key stored in the app |

## API Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/api/v1/health` | Version, database, each weather source in use, the forecast's status, and whether `/mcp` is on |
| `GET` | `/api/v1/today` | The lawn's local date and timezone |
| `GET` | `/api/v1/dashboard` | Composite dashboard (profile, env summary, alerts, recent apps, source status) |
| `GET` | `/api/v1/profile` | Current lawn profile |
| `PUT` | `/api/v1/profile` | Update lawn profile (a new location rebuilds the sources and starts fetching) |
| `GET` | `/api/v1/setup/places?q=` | Candidate places for a name or postal code (the text goes to Open-Meteo's place search) |
| `POST` | `/api/v1/setup/apply` | Save the location, station pins and forecast provider together |
| `GET` | `/api/v1/sources` | Each input with its plan, value in use and basis; each source's status, history and credit; forecast provider; connections (never a secret); location |
| `GET` | `/api/v1/sources/preview?lat=&lon=&pins=` | The sources a point would get; saves nothing |
| `GET` | `/api/v1/sources/stations?input=` | Stations within 150 miles that may supply an input |
| `PUT` / `DELETE` | `/api/v1/sources/pins/:input` | Pin a station for an input / remove the pin |
| `PUT` | `/api/v1/sources/forecast-provider` | Choose the forecast provider (with OpenWeatherMap's key) |
| `GET` / `PUT` / `DELETE` | `/api/v1/sources/home-assistant` | Whether a connection is stored (never the token) / store one / forget it |
| `POST` | `/api/v1/sources/home-assistant/test` | Try a connection; says whether nothing answered or the token was refused |
| `GET` | `/api/v1/sources/home-assistant/entities` | Home Assistant's numeric sensors, each with a suggested input |
| `GET` / `POST` | `/api/v1/sources/sensors` | The mapped sensors / map one |
| `POST` | `/api/v1/sources/sensors/check` | A Home Assistant sensor's reading beside the value in use; saves nothing |
| `PUT` / `DELETE` | `/api/v1/sources/sensors/:id` | Rename or remap a sensor / unmap it (its readings are kept) |
| `GET` | `/api/v1/applications?type=&scope=turf\|landscape&limit=&offset=` | List applications |
| `POST` | `/api/v1/applications` | Create new application |
| `PUT` | `/api/v1/applications/:id` | Update application |
| `DELETE` | `/api/v1/applications/:id` | Delete application |
| `GET` | `/api/v1/applications/calendar?year=Y&month=M` | Applications grouped by date |
| `GET` | `/api/v1/environmental` | Current conditions and the 7-day summary from the weather record, each value with its basis |
| `POST` | `/api/v1/environmental/refresh` | Ask for a refresh pass now (at most once a minute); waits up to 20 s and returns the summary |
| `GET` | `/api/v1/recommendations` | Active recommendations: rules engine + disease risk (High/Severe) + timing windows (pre-emergent, seeding) + plant maintenance and follow-ups (`?include_inactive=true` adds answered ones) |
| `PATCH` | `/api/v1/recommendations/:id` | Mark recommendation addressed/dismissed |
| `GET` | `/api/v1/disease-risk` | Per-disease risk: tier, score, 11-day series, contributing factors, methodology, management plan |
| `GET` | `/api/v1/timing-windows` | Seeding and pre-emergent windows: typical dates, this season's status, freeze dates, soil chart series |
| `GET` | `/api/v1/gdd?year=Y` | GDD accumulation + crabgrass germination model |
| `GET` | `/api/v1/historical?range=7d\|30d\|90d` | Time-series environmental data for trend charts |
| `GET` | `/api/v1/nitrogen-budget` | Annual nitrogen applied vs. grass-type target |
| `GET` | `/api/v1/soil-temp-forecast` | Predicted soil temperatures and threshold crossings |
| `GET` | `/api/v1/seasonal-plan?year=Y` | Full year of predicted activity windows (turf + plants) |
| `GET` | `/api/v1/soil-tests` | List soil tests |
| `POST` | `/api/v1/soil-tests` | Record a soil test |
| `PUT` | `/api/v1/soil-tests/:id` | Update a soil test |
| `DELETE` | `/api/v1/soil-tests/:id` | Delete a soil test |
| `GET` | `/api/v1/soil-tests/recommendations` | pH and nutrient recommendations from the latest test |
| `GET` | `/api/v1/plants` | List plants for the active profile |
| `POST` | `/api/v1/plants` | Add a plant — backend calls OpenRouter and caches the plan |
| `GET` | `/api/v1/plants/:id` | Single plant with cached maintenance plan |
| `PUT` | `/api/v1/plants/:id` | Update plant metadata (name, location, notes) |
| `DELETE` | `/api/v1/plants/:id` | Delete a plant |
| `POST` | `/api/v1/plants/:id/refresh-plan` | Regenerate the cached plan via OpenRouter |
| `GET` | `/api/v1/products?category=&include_archived=` | Shelf inventory for the active profile |
| `POST` | `/api/v1/products` | Add a product — the assistant identifies it when configured (`assist: false` or no key → saved by hand, `category` required) |
| `GET` | `/api/v1/products/:id` | One product with its facts and optional assistant profile |
| `PUT` | `/api/v1/products/:id` | Replace the user-owned fields (name, brand, stock status, facts, notes, archived) |
| `DELETE` | `/api/v1/products/:id` | Delete a product |
| `POST` | `/api/v1/products/:id/refresh-profile` | Regenerate the assistant profile; returns suggested facts + which fields differ |
| `POST` | `/api/v1/products/:id/link-applications` | Link past applications logged under this product's name |
| `GET` | `/api/v1/inventory/suggestions` | Product names in your log that are not on the shelf |
| `GET` | `/api/v1/inventory/shopping-list` | What to buy or restock, from the active recommendations |
| `GET` | `/api/v1/frac-classes?product=` | FRAC class options for the application form, and the classes a product name resolves to |
| `POST` | `/api/v1/sensor-readings` | Readings from the owner's hubs and scripts — only when `SENSOR_TOKEN` is set; requires `Authorization: Bearer $SENSOR_TOKEN` |
| `POST` | `/mcp` | MCP server (streamable HTTP, read-only tools) — only when `MCP_TOKEN` is set; requires `Authorization: Bearer $MCP_TOKEN` |

## Pages

| Page | Description |
|------|-------------|
| **Dashboard** | Gauges for soil temp, ambient temp, humidity, and soil moisture. GDD, disease risk, seeding/pre-emergent timing, nitrogen budget, and soil-temp forecast widgets. Active alerts and recent applications. Auto-refreshes every 30 seconds. |
| **Applications** | Filterable table of all lawn treatments including mowing. Add new applications with type, product, rate, and notes. The product is picked from your Inventory (filtered to what fits the type) and pre-fills N-P-K, FRAC class and rate; a name can still be typed for something not on the shelf. |
| **Inventory** | What's on the shelf, grouped by category (fertilizer, supplements, fungicides, herbicides, insect control, seed, amendments, surfactants). Type a product name and the assistant fills in actives, N-P-K, FRAC class, timing and targets — every fact stays editable and a suggested label rate is flagged "verify against your label". No quantities: a click on the stock pill cycles In stock → Low → Out. Products save by hand when the assistant is off. Names you have logged under Applications but never catalogued are offered for one-click adding, and "Link past applications" attaches earlier log entries to a product. A "Needs buying" list at the top collects what active recommendations call for that is not on the shelf, plus Low / Out products they need; the dashboard shows the top of it. |
| **Calendar** | Month grid view with colored dots for applications and status-colored bars for seasonal plan activity windows. Plant-maintenance windows render as outlined bars (distinct from filled turf bars). Click any date to see details grouped into Applications, Turf Activities, and Plant Maintenance. |
| **Landscape** | Add plants by common or scientific name to get a homeowner-level care plan (pruning, fertilizing, mulching, etc.) per plant. Each card shows the plan summary, task windows, warnings, and a "Regenerate plan" button. **Requires `OPENROUTER_API_KEY`** — see [OpenRouter](#openrouter-optional--landscape-maintenance). |
| **Environmental** | Current conditions, soil depth readings, 7-day trends and averages; every value labeled with its basis, and each chart's source credited. |
| **Recommendations** | Active recommendations from the rules engine, including a new **Plant Maintenance** category when a plant's care window is open. Diseases at High or Severe risk appear here too. Mark as addressed or dismiss. Each card that calls for a product carries an inventory chip — On hand, running low, Need to buy, or Partly on hand — and the detail pane lists which shelf products cover it. |
| **Disease Risk** | One row per disease with its score, tier meter, and Low/Moderate/High/Severe badge. Select a disease for its 11-day chart (observed + forecast), what to do now, cultural practices, preventative and curative fungicide programs, contributing factors, and how the score is calculated. See [Disease Risk](#disease-risk). |
| **Seed & Pre-Em Timing** | A card per window (fall seeding, spring and fall pre-emergent, plus spring and dormant seeding) with its state badge, a timeline of the typical window against today, and a trigger table showing typical dates (median, earliest–latest) beside where this season stands (observed / tentative / forecast / typical). Tiles for the current 5 cm soil mean and the lawn's freeze dates, each with its basis, a soil chart of this year vs last year vs the typical year with the season's thresholds, and a "how this is calculated" panel. See [Seeding & Pre-Emergent Timing](#seeding--pre-emergent-timing). |
| **Soil Tests** | Record lab results and get pH (lime/sulfur) and nutrient recommendations. |
| **Seasonal Plan** | Full-year timeline of predicted activity windows. When plants are configured, a **All / Turf / Plants** filter appears so you can view them separately. |
| **Sources** | Each input (soil temperature and moisture by depth, air, humidity, rain) with the source in use and what stands behind it; every source's status, history and credit; the forecast provider; the Home Assistant connection and mapped sensors. Change the location, pin a station, or switch provider from here. |
| **Setup** | Where the lawn is, the stations and forecast provider it gets, your sensors, the lawn profile, and history loading. Opens on first run. |
| **Settings** | Edit lawn profile (grass type, zone, soil type, size, irrigation). |

## Development

### Prerequisites

- Rust 1.88+
- Node.js 20.19+ or 22.12+
- PostgreSQL 18 (or use `docker compose up db` for just the database)

### Backend

```bash
cd backend

# Set up environment
cp .env.example ../.env
source ../.env  # or use dotenvy

# The app reads DATABASE_HOST / DATABASE_PORT / DATABASE_NAME / DATABASE_USER /
# DATABASE_PASSWORD (not DATABASE_URL). With `docker compose up db`:
export DATABASE_HOST=localhost DATABASE_PORT=5433 DATABASE_PASSWORD="$DB_PASSWORD"
cargo run  # migrations run automatically on startup

# Development commands
cargo build          # Build
cargo test           # Run tests (no database needed)
# All tests, including the PostgreSQL-backed ones (needs a test database):
# DATABASE_URL=postgres://postgres@127.0.0.1:5434/postgres cargo test --features db-tests
cargo fmt            # Format code
cargo clippy         # Lint
RUST_LOG=debug cargo run  # Run with debug logging
```

### Frontend

```bash
cd frontend

npm install          # Install dependencies
npm run dev          # Dev server with API proxy (port 5173 → backend 3000)
npm run build        # Production build to dist/
npx tsc -b           # Type check (--noEmit checks nothing: the root tsconfig is a solution file)
npm test             # Vitest
npm run lint         # Biome lint + format check
```

During development, Vite proxies `/api` requests to the backend at `http://localhost:3000`.

### Docker Build

```bash
docker compose build   # Build containers
docker compose up -d   # Start full stack
docker compose logs -f app  # Follow app logs
```

The Dockerfile uses a multi-stage build:
1. **Node (Alpine)** — builds the React frontend
2. **Rust (slim)** — compiles the backend binary
3. **Chainguard `glibc-dynamic`** — minimal distroless runtime image (glibc for the binary), runs as non-root

## Agronomic Rules

TurfOps includes 10 rules that evaluate environmental conditions and generate actionable recommendations. Rules are divided into current-condition rules (using the weather record's current values) and forecast-based rules (using the forecast from the provider chosen in setup). Every weather value a rule cites names its source; the source never changes a threshold. Turf diseases are not handled by rules — see [Disease Risk](#disease-risk). Neither are pre-emergent and seeding timing: those recommendations come from the timing windows — see [Seeding & Pre-Emergent Timing](#seeding--pre-emergent-timing).

### Current-Condition Rules

#### Pre-Emergent Timing
No longer a rule. Spring and fall pre-emergent recommendations come from the timing windows (measured 5 cm soil, 5-day sustained crossings, GDD) — see [Seeding & Pre-Emergent Timing](#seeding--pre-emergent-timing).

#### Spring Nitrogen Timing
**Purpose**: Prevent damage from fertilizing too early in spring

| Condition | Severity | Action |
|-----------|----------|--------|
| Soil <50°F | Info | Wait - too cold for fertilizer |
| Soil 50-55°F | Info | Almost ready - continue waiting |
| Soil 55-65°F, no spring fert yet | Advisory | Ready for light spring nitrogen |
| Fertilized while soil <55°F | Warning | Applied too early - avoid more nitrogen |

**Key Points**: Wait until soil reaches 55°F (7-day average). Spring nitrogen should be light (0.5 lb N/1000 sqft) — save heavy feeding for fall.

#### Grub Control Timing
**Purpose**: Prevent grub damage through preventative insecticide

| Condition | Severity | Action |
|-----------|----------|--------|
| May 15 - Jul 4, soil 60-75°F | Advisory | Apply preventative grub control |
| <14 days remaining in window | Warning | Apply soon - window closing |
| Soil >75°F | Info | Late but may still be effective |

**Active**: May 15 through July 4, or from April 1 once GDD reaches 500 (until 1,000) | **Products**: Chlorantraniliprole (GrubEx) or imidacloprid.

#### Fertilizer Stress Block
**Purpose**: Prevent fertilizer burn during heat or moisture stress

| Condition | Severity | Action |
|-----------|----------|--------|
| Ambient temp >85°F | Warning | Avoid nitrogen application |
| Ambient temp >90°F | Critical | Do NOT apply any fertilizer |
| Soil moisture <0.10 | Warning | Drought stress - irrigate first |
| Soil moisture <0.05 | Critical | Severe drought - delay fertilizer |
| Soil moisture >0.40 | Warning | Saturated - fertilizer will leach |

#### Broadleaf Herbicide Timing
**Purpose**: Target broadleaf weeds during optimal control windows

| Condition | Severity | Action |
|-----------|----------|--------|
| GDD 50–150 (or March), soil 45-55°F rising | Advisory | Spring window — target winter annuals |
| Late Sept - Oct, soil 50-65°F | Warning | Fall window — best for perennial weeds |

**Active**: GDD 50–150 or March (spring) and September 20 - October 31 (fall). Blocked if overseeded within 60 days.

#### Mowing Height
**Purpose**: Seasonal mowing guidance for TTTF

| 7-Day Avg Temp | Height | Severity |
|-----------------|--------|----------|
| 50-75°F | 2.5-3.5" | Info |
| 75-85°F | 3-4" | Advisory |
| >85°F | 3.5-4" | Warning |

Never remove more than 1/3 of the blade at once.

#### Core Aeration
No longer a rule. Aeration shares the **fall seeding window** (cores are pulled right before seed goes down), so the recommendation is derived from that window — see [Seeding & Pre-Emergent Timing](#seeding--pre-emergent-timing) — instead of its own Aug 15 – Oct 15 / 50–65°F (10 cm) gate, which opened weeks after seeding did and stayed open after it closed.

| Condition | Severity |
|-----------|----------|
| Fall seeding window Open / Ideal / Closing, not aerated in 12+ months | Advisory |
| …and Clay / Clay Loam soil | Warning |
| Aerated within 12 months | Info |

Silent once you have aerated or **seeded** this fall, and when a **fall pre-emergent** is logged (coring breaks the barrier).

### Fall Program Rules

#### Fall Overseeding
No longer a rule. The seeding recommendation comes from the fall seeding window, which is anchored to the lawn's first-freeze climatology rather than a fixed Aug 15 – Oct 31 — see [Seeding & Pre-Emergent Timing](#seeding--pre-emergent-timing). The recommendation still sizes the seed needed at 4 lbs per 1000 sqft for overseeding.

#### Fall Fertilization Program
**Purpose**: Build root reserves for winter survival and spring green-up

| Phase | Timing | Nitrogen Rate | Purpose |
|-------|--------|---------------|---------|
| Early Fall | September | 1.0 lb N/1000 sqft | Recovery from summer stress |
| Mid Fall | October | 0.75 lb N/1000 sqft | Primary fall feeding (most important) |
| Late Fall | November | 1.0 lb N/1000 sqft | Winterizer - stores for spring |

### Forecast-Based Rules

These rules use the forecast from the provider chosen in setup.

#### Rain Delay
Prevents wasted chemical applications before rain.

| Condition | Severity | Action |
|-----------|----------|--------|
| Within 48 h: ≥ 30% chance or ≥ 0.2" | Advisory | Consider timing carefully |
| Within 24 h: ≥ 50% chance or ≥ 0.1" | Warning | Delay applications if possible |
| Within 12 h: ≥ 70% chance or ≥ 0.1" | Critical | Do NOT apply any products |

A window with no slot at 50% or more and under 0.1 mm in total is skipped. The first level that matches wins.

#### Irrigation Forecast
Recommends irrigation when drought conditions are developing.

| Condition | Severity | Action |
|-----------|----------|--------|
| Under 0.1" forecast over 5 days + moisture 0.15-0.20 | Advisory | Monitor and prepare to irrigate |
| Under 0.1" forecast over 5 days + moisture 0.10-0.15 | Warning | Irrigate within 1-2 days |
| Under 0.1" forecast over 5 days + moisture <0.10 | Critical | Water immediately |
| No soil-moisture source, grass growing (7-day mean ≥ 50°F), under ½" of rain in the last 7 days | Advisory | Check the soil; plan a deep watering |
| …and under 0.2" (5 mm) in the last 7 days | Warning | Irrigate within 1-2 days |

The forecast amount counts only over days that carry one, and at least 3 of the 5 must. Without a soil-moisture source (no station within 50 miles and no probe) the rule is judged on rain and never goes past Warning; it says so.

#### Heat Stress Warning
Prepares for upcoming heat stress conditions.

| Forecasted Max Temp | Severity | Action |
|---------------------|----------|--------|
| 85-90°F in next 3 days | Advisory | Raise mowing height, water early |
| 90-95°F in next 3 days | Warning | Avoid fertilizer, skip mowing |
| >95°F in next 3 days | Critical | Accept dormancy, minimize all stress |

#### Optimal Application Window
Identifies the best day in the next five for chemical applications: a full day with a mean of at least 50°F and a high of at most 85°F, dry that day and the next, and dry the day before (for the first day, under an inch of rain in the last week). Rain that isn't known is never counted as dry. Wind and humidity add to the score.

### Cross-rule checks

- **Nitrogen guard.** After the rules run, any "apply nitrogen" recommendation (fall feedings, spring nitrogen) is checked against the rest: it is put **on hold** (Info) while *Fertilizer Stress Block* or a Warning-level heat-stress forecast is active, it shows the **remaining annual nitrogen budget** for your grass type, is capped to that remainder when less than a full feeding is left, and turns into "annual nitrogen target reached" when none is.
- **Fall broadleaf herbicide follows the seeding decision.** Overseed logged this fall → no herbicide recommendation. Fall pre-emergent logged (so you are not seeding) or the seeding window over → unchanged. Undecided while the seeding window is still usable → Advisory, prefixed "Only if you are NOT seeding this fall".

## Seeding & Pre-Emergent Timing

`GET /api/v1/timing-windows` times the start- and end-of-season jobs from the lawn's weather record instead of the calendar. Soil thresholds are 5-day means of the **5 cm** soil temperature from the weather record (a station's or your sensor's measurement, or the soil estimate where neither has one, marked as estimated), and a crossing only counts once it has held for 5 days.

| Window | Opens | Ideal | Closes |
|--------|-------|-------|--------|
| **Fall seeding / overseeding** | Soil cools to 75°F (from Aug 15) | until 45 days before the typical first fall freeze | 30 days before that freeze, or soil at 55°F — whichever is first |
| **Spring pre-emergent** | Soil warms to 45°F | from 50°F | Soil at 55°F sustained, or 200 GDD — whichever is first |
| **Fall pre-emergent** | Soil cools to 70°F | until 65°F | Soil cools to 55°F |
| **Spring seeding** *(secondary)* | Soil warms to 50°F | — | Soil warms to 65°F |
| **Dormant seeding** *(optional)* | Soil cools to 40°F | — | Soil warms back to 45°F in spring |

Each boundary is found in every historical year (up to 15) and reported as a median with its earliest/latest spread, then resolved for this season as **observed**, **tentative** (held fewer than 5 days), **forecast** (the soil outlook over the forecast's first 10 full days) or **typical**. A trigger that is past its usual date but has not happened is reported as running late — it is never given an invented date. First and last freeze dates come from the lawn's daily air minimums in the weather record (≤ 32°F; your air sensor where one is mapped, else the PRISM grid at the lawn).

Seed and pre-emergent never share a season: a logged `Overseed` blocks that season's pre-emergent and a logged `PreEmergent` blocks seeding. Establishment buffers depend on grass type (Kentucky bluegrass 60/45 days, fescues 45/30, perennial ryegrass 35/21). No application rates are given — the label governs.

**In the feed, plan and calendar.** The same windows drive the pre-emergent and seeding recommendations (Opening soon → Info; Open/Ideal → Advisory; Closing → Warning; a just-missed spring pre-emergent → Warning for 3 weeks; fall pre-emergent stays at Info/Advisory because it is the alternative to seeding) and the Seasonal Plan's Spring Pre-Emergent, Fall Pre-Emergent, Fall Seeding and Core Aeration activities, which the Calendar overlays. A window ruled out by your log is left off the plan. If the weather record cannot answer (no location yet) the plan falls back to its 10 cm soil windows and the feed simply has no timing recommendations.

Full method, thresholds and how it differs from the lawn-answers.com tools it was modeled on: [docs/timing-windows.md](docs/timing-windows.md).

## Disease Risk

Each disease is scored independently by its own model and then read off on a common **Low / Moderate / High / Severe** scale. The native scores (a probability, an index, a point score) are not comparable to each other, so there is deliberately no single blended "disease pressure" number.

| Disease | Model | Inputs | Tiers (Moderate / High / Severe) |
|---------|-------|--------|----------------------------------|
| **Brown patch** | Fidanza-Dernoeden E-index — `E = −21.5 + 0.15·RH + 1.4·Tmin − 0.033·Tmin²` (Phytopathology 86:385–390, 1996) | Daily min air temp (°C), daily mean RH | E ≥ 0 / ≥ 5 / ≥ 6 |
| **Dollar spot** | Smith-Kerns logistic — `logit = −11.4041 + 0.0894·MEANRH + 0.1932·MEANAT`, inactive outside 10–35°C (PLOS ONE 13(3):e0194216, 2018) | 5-day moving averages of daily mean air temp and RH | ≥ 10% / ≥ 20% (published action threshold) / ≥ 40% |
| **Pythium blight** | 0–5 score on the Nutter et al. criteria (Plant Disease 67:1126–1128, 1983): +1 max ≥ 86°F, +1 min ≥ 68°F, +1 each at 6 / 10 / 14 h of RH ≥ 90%; capped at 1 without a heat criterion | Daily max/min air temp, hours RH ≥ 90% | 2 / 3 / 4 |
| **Gray leaf spot** *(experimental)* | 0–100 suitability = temperature × leaf wetness, 3-day mean, July–October only (drivers per Uddin et al., Phytopathology 93:336–343, 2003) | Daily mean air temp, estimated leaf wetness | 25 / 50 / 75 |
| **Red thread** *(experimental)* | 0–100 suitability = cool temperature × leaf wetness, 5-day mean | Daily mean air temp, estimated leaf wetness, rainfall | 25 / 50 / 75 |

Brown patch and dollar spot are peer-reviewed, field-validated models. Pythium uses published thresholds combined into a heuristic score. Gray leaf spot and red thread have no validated turf forecasting model, so they are composite indices and are labeled **experimental** in the UI — use them alongside scouting.

**Lawn history modifies risk.** Overseeding within 60 days raises gray leaf spot one tier (seedlings are highly susceptible); no fertilizer in 60 days raises red thread one tier (it is a low-nitrogen disease). Only the headline tier is raised, never from Low, and the page says why. The daily chart stays weather-only.

**Where the data comes from.** Observed days are aggregated per local day from the weather record's hourly air, RH and precipitation, one source per day (your sensor, a station within 10 miles, or the hourly analysis, whose days are marked approximate). Leaf wetness is estimated as hours with RH ≥ 90% or measurable precipitation; dew point uses the Magnus formula. Today's remaining hours and the outlook come from the forecast. When too little of today is covered (under 12 hours), the headline reflects the most recent complete day and the page shows a note saying so. While no forecast is held there is no outlook.

### What to do: preventative and curative guidance

Every disease page turns its tier into an action:

| Tier | Action |
|------|--------|
| Low | No action |
| Moderate | Monitor — cultural practices only, with a heads-up if the 3-day outlook reaches High or Severe |
| High / Severe | Apply a preventative — or **Protected** if a logged fungicide still covers this disease |

- **Cultural practices** come first for every disease (nitrogen, irrigation timing, mowing, airflow).
- A **preventative program** and a **curative program** ("if you see symptoms") list FRAC classes with example products, efficacy, and interval. TurfOps cannot see your lawn, so the curative program is always shown for you to apply on your own judgment.
- **Protection window**: a logged fungicide that is at least *Good* on a disease counts as protection for 21 days (single-site systemic) or 14 days (contact / phosphonate), shortened by 7 days under Severe pressure. Efficacy is per disease: azoxystrobin (Heritage) applied for brown patch does **not** count as dollar spot protection — it has no useful dollar spot activity and can make it worse. A strobilurin does count as Pythium protection at High risk, but not at Severe, where only Pythium-specific chemistry (cyazofamid, propamocarb, mefenoxam) is trusted.
- **How a logged fungicide is understood.** When you log a Fungicide, the form shows a **FRAC class** picker. It pre-fills from the product name when TurfOps knows the product (actives and common trade names, including premixes such as Headway = FRAC 11 + 3), and warns when it doesn't — pick the class(es) from the label. An unrecognized product with no class recorded earns **no** protection and no rotation credit, so the page would keep saying "apply". A premix counts for every class it contains.
- **Red thread** is treated with nitrogen, not fungicide.
- **No application rates are given.** Rates are product-specific; always read and follow the product label.
- **Where the efficacy ratings come from.** They were checked against the University of Kentucky's [PPA-1, *Chemical Control of Turfgrass Diseases 2024*](https://publications.mgcafe.uky.edu/files/PPA1.pdf), which rates each active ingredient 1–4 per disease: 4 / 3.5 → Excellent, 3 / 2.5 → Good, 2 / 1.5 → Fair. Those ratings come largely from golf-turf trials, so treat them as relative efficacy, not a home-lawn prescription. Ratings are kept per FRAC *class*, but actives within a class can differ (pyraclostrobin is good on dollar spot while azoxystrobin is not); where a class is split like that it is left out of that disease's options, which errs toward not claiming protection.

Diseases at High or Severe also appear as recommendations on the Dashboard and Recommendations page, generated from the same model output so the two always agree.

### FRAC Rotation System

TurfOps tracks fungicide application history and provides rotation-aware recommendations to prevent resistance development.

| FRAC Class | Type | Common Products |
|------------|------|-----------------|
| FRAC 1 | Thiophanates | thiophanate-methyl, Cleary's 3336 |
| FRAC 3 | DMIs/Triazoles | propiconazole, Banner MAXX, myclobutanil, Eagle |
| FRAC 4 | Phenylamides (Pythium) | mefenoxam, Subdue MAXX — resistance risk; don't use back to back |
| FRAC 7 | SDHI | fluxapyroxad, Xzemplar, penthiopyrad, Velista |
| FRAC 11 | Strobilurins | azoxystrobin, Heritage, pyraclostrobin, Insignia |
| FRAC 12 | Phenylpyrroles | fludioxonil, Medallion |
| FRAC 14 | Aromatics | PCNB, Turfcide |
| FRAC 21 | QiI (Pythium) | cyazofamid, Segway |
| FRAC 28 | Carbamates (Pythium) | propamocarb, Banol |
| FRAC P07 | Phosphonates, formerly FRAC 33 (Pythium, preventative only) | fosetyl-Al, Signature, phosphite |
| FRAC M3 | Multi-site (dithiocarbamates) | mancozeb |
| FRAC M5 | Multi-site (chloronitriles) | chlorothalonil, Daconil — not labeled for residential lawns in the US; tracked for protection, never suggested |

**Rotation**: for each disease, the suggested class is the most effective option for *that disease* that is not the single-site class you used last; the last-used class is tagged "rotate". A season-level warning appears after two consecutive applications of the same single-site class or three or more fungicide applications in a year. Multi-site fungicides (M3, M5) are excluded from rotation calculations (low resistance risk). FRAC classes come from the classes picked on the application form, or else from the product name, so pick them from the label or use a recognizable product or active-ingredient name.

## Lawn Profile Defaults

- **Grass Type**: Turf Type Tall Fescue (TTTF), USDA zone 7a, unless `LAWN_*` says otherwise
- **Location**: none until you set one in Setup (or with `LAWN_LATITUDE` / `LAWN_LONGITUDE`). TurfOps then chooses the nearby stations and sources, which you can change on the Sources page.

## Agronomic Thresholds (TTTF Zone 7a)

| Metric | Threshold | Meaning |
|--------|-----------|---------|
| Soil temp 5cm (5-day mean, held 5 days) | 45 / 50 / 55°F rising | Spring pre-emergent: worthwhile / ideal / closed (or 200 GDD) |
| Soil temp 5cm (5-day mean, held 5 days) | 70 / 65 / 55°F falling | Fall pre-emergent: opens / late / closed |
| Soil temp 5cm + first fall freeze | ≤75°F from Aug 15 → 30 days before the typical freeze (or 55°F) | Fall seeding window; ideal until 45 days before the freeze |
| Soil temp 10cm | 60-75°F | Grub control window |
| Ambient temp | >85°F | Fertilizer stress risk |
| Soil moisture | <0.20 / <0.15 / <0.10 | Irrigation: Advisory / Warning / Critical (with no real rain forecast) |
| Rain, last 7 days (no soil-moisture source) | <½" / <0.2" | Irrigation: Advisory / Warning, while the grass is growing |
| Soil moisture | >0.40 | Saturated - avoid fertilizer |
| Humidity | >80% | Reference line on the Environmental humidity chart |
| Brown patch E-index | ≥5 | Brown patch warning (High) |
| Dollar spot probability | ≥20% | Smith-Kerns action threshold (High) |

## Contributing

Issues and pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.

## License

Released under the [MIT License](LICENSE).

### Data bundled with the application

The lawn's timezone is found from its coordinates using timezone boundaries compiled into the
binary. That boundary data is © OpenStreetMap contributors, made available under the
[Open Database License (ODbL) 1.0](https://opendatacommons.org/licenses/odbl/1-0/), assembled
by [timezone-boundary-builder](https://github.com/evansiroky/timezone-boundary-builder) and
packaged by [tzf-rs](https://github.com/ringsaturn/tzf-rs). It is included unmodified.
