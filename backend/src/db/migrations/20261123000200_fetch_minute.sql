-- The minute of the hour this installation makes its hourly and daily weather fetches
-- at. Chosen at random when the lawn first gets a location and kept, so installations
-- do not all call a public host at the top of the hour.
ALTER TABLE lawn_profiles ADD COLUMN IF NOT EXISTS fetch_minute SMALLINT
    CHECK (fetch_minute BETWEEN 0 AND 59);
