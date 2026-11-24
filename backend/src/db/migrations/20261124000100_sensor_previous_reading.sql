-- Each sensor's previous reading (WX-SRC-HA-011): its value in record units (a rain
-- counter's total, not an amount) and its reported time. It decides whether a new reading
-- is new and how far a counter rose, across restarts. Cleared when the mapping changes.
ALTER TABLE sensor
    ADD COLUMN previous_value DOUBLE PRECISION,
    ADD COLUMN previous_at TIMESTAMPTZ,
    ADD CONSTRAINT chk_sensor_previous CHECK ((previous_value IS NULL) = (previous_at IS NULL));
