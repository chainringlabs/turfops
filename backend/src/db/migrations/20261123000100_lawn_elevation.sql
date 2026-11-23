-- The elevation of the lawn's point, in feet, as the hourly analysis reports it.
-- Unknown until that source's first fetch.
ALTER TABLE lawn_profiles ADD COLUMN IF NOT EXISTS elevation_ft DOUBLE PRECISION;
