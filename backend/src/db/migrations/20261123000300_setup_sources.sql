-- What the owner chooses about the lawn's weather sources (SETUP).

-- One row: the forecast provider. No row means the default, Open-Meteo.
CREATE TABLE weather_setting (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    forecast_provider TEXT NOT NULL
        CHECK (forecast_provider IN ('OpenMeteo', 'NationalWeatherService', 'OpenWeatherMap'))
);

-- The owner's sensors: a Home Assistant entity read on a schedule, or one whose readings
-- are posted to TurfOps. Each is mapped to one input; unmapping keeps the row and its
-- readings.
CREATE TABLE sensor (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    way_in TEXT NOT NULL CHECK (way_in IN ('HomeAssistant', 'Posted')),
    entity_id TEXT,
    input TEXT NOT NULL,
    -- For a rain sensor: a counter that only rises, or an amount since its last reading.
    rain_kind TEXT CHECK (rain_kind IN ('Counter', 'Amount')),
    mapped_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    unmapped_at TIMESTAMPTZ,
    CONSTRAINT chk_sensor_entity CHECK ((way_in = 'HomeAssistant') = (entity_id IS NOT NULL)),
    CONSTRAINT chk_sensor_rain CHECK ((input = 'rain') = (rain_kind IS NOT NULL))
);
-- One sensor per input, and one input per Home Assistant entity, among those mapped.
CREATE UNIQUE INDEX sensor_one_per_input ON sensor (input) WHERE unmapped_at IS NULL;
CREATE UNIQUE INDEX sensor_one_per_entity ON sensor (entity_id)
    WHERE unmapped_at IS NULL AND entity_id IS NOT NULL;

-- A station the owner chose for an input, in place of the default stations.
CREATE TABLE station_pin (
    input TEXT PRIMARY KEY,
    station_id BIGINT NOT NULL REFERENCES weather_station(id) ON DELETE CASCADE,
    pinned_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- A connection the owner entered or that was imported once from the environment. The
-- secret is never returned by any endpoint. Forgetting a connection clears its address
-- and secret and keeps the row, so a restart does not import it again.
CREATE TABLE service_credential (
    service TEXT PRIMARY KEY CHECK (service IN ('HomeAssistant', 'OpenWeatherMap')),
    address TEXT,
    secret TEXT,
    set_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
