-- The weather record: what TurfOps fetched, one row per source, never blended.
-- Units are fixed at the door: °F, mm, percent relative humidity, soil moisture 0–1.

-- The networks' station lists (contiguous United States).
CREATE TABLE weather_station (
    id BIGSERIAL PRIMARY KEY,
    network TEXT NOT NULL CHECK (network IN ('Uscrn', 'Scan')),
    -- The network's own id: a WBAN number for USCRN, a station id for SCAN.
    station_id TEXT NOT NULL,
    name TEXT NOT NULL,
    state TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    elevation_ft DOUBLE PRECISION,
    -- Inputs the station reports, as `Input` names.
    inputs TEXT[] NOT NULL DEFAULT '{}',
    first_year INTEGER,
    last_year INTEGER,
    -- What the network needs to address the station's data (USCRN: the file-name stem).
    locator TEXT,
    -- SCAN reports in a fixed local standard time.
    utc_offset_hours DOUBLE PRECISION,
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (network, station_id)
);

-- A source in use for the lawn. `key` is its identity, e.g. `station:Uscrn:3761`,
-- `grid:Prism:39.8612,-75.7866`, `analysis:39.8612,-75.7866`, `estimate:5:…`, `sensor:7`.
CREATE TABLE weather_source (
    id BIGSERIAL PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK (kind IN ('Sensor', 'Station', 'Grid', 'Analysis', 'Estimate')),
    name TEXT NOT NULL,
    station_ref BIGINT REFERENCES weather_station(id) ON DELETE CASCADE,
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION,
    -- Great-circle miles from the lawn, for a station.
    distance_miles DOUBLE PRECISION,
    role TEXT NOT NULL DEFAULT 'Basis' CHECK (role IN ('Basis', 'Calibration')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- One row per source and UTC hour, stamped at the start of the hour.
CREATE TABLE weather_hourly (
    source_id BIGINT NOT NULL REFERENCES weather_source(id) ON DELETE CASCADE,
    observed_at TIMESTAMPTZ NOT NULL,
    air_temp_f DOUBLE PRECISION,
    air_temp_max_f DOUBLE PRECISION,
    air_temp_min_f DOUBLE PRECISION,
    rh_pct DOUBLE PRECISION,
    precip_mm DOUBLE PRECISION,
    soil_temp_5_f DOUBLE PRECISION,
    soil_temp_10_f DOUBLE PRECISION,
    soil_temp_20_f DOUBLE PRECISION,
    soil_temp_50_f DOUBLE PRECISION,
    soil_temp_100_f DOUBLE PRECISION,
    soil_moisture_5 DOUBLE PRECISION,
    soil_moisture_10 DOUBLE PRECISION,
    soil_moisture_20 DOUBLE PRECISION,
    soil_moisture_50 DOUBLE PRECISION,
    soil_moisture_100 DOUBLE PRECISION,
    PRIMARY KEY (source_id, observed_at),
    CONSTRAINT chk_weather_hourly_on_the_hour CHECK (
        (observed_at AT TIME ZONE 'UTC') = date_trunc('hour', observed_at AT TIME ZONE 'UTC')
    )
);

-- One row per source and day, for sources that publish daily values only. The day is the
-- source's own.
CREATE TABLE weather_daily (
    source_id BIGINT NOT NULL REFERENCES weather_source(id) ON DELETE CASCADE,
    day DATE NOT NULL,
    air_max_f DOUBLE PRECISION,
    air_min_f DOUBLE PRECISION,
    air_mean_f DOUBLE PRECISION,
    precip_mm DOUBLE PRECISION,
    soil_temp_5_f DOUBLE PRECISION,
    soil_temp_10_f DOUBLE PRECISION,
    PRIMARY KEY (source_id, day)
);

-- Sensor readings as they arrive; folded into the sensor source's hourly row and kept 7 days.
CREATE TABLE weather_sensor_reading (
    source_id BIGINT NOT NULL REFERENCES weather_source(id) ON DELETE CASCADE,
    input TEXT NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (source_id, input, observed_at)
);

-- The ranked sources for each input.
CREATE TABLE weather_basis_plan (
    input TEXT NOT NULL,
    rank INTEGER NOT NULL,
    source_id BIGINT NOT NULL REFERENCES weather_source(id) ON DELETE CASCADE,
    pinned BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (input, rank)
);

-- What each source's product is wanted for, what is held, and how its fetches have gone.
CREATE TABLE weather_fetch_state (
    source_id BIGINT NOT NULL REFERENCES weather_source(id) ON DELETE CASCADE,
    product TEXT NOT NULL,
    wanted_from DATE NOT NULL,
    held_from DATE,
    held_through DATE,
    last_attempt TIMESTAMPTZ,
    last_success TIMESTAMPTZ,
    last_failure TIMESTAMPTZ,
    last_error TEXT,
    PRIMARY KEY (source_id, product),
    CONSTRAINT chk_weather_fetch_held CHECK ((held_from IS NULL) = (held_through IS NULL))
);
