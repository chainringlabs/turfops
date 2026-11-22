-- The lawn's location: the one weather setting. Latitude and longitude are set and cleared
-- together; the timezone is derived from them by the application.
ALTER TABLE lawn_profiles
    ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION,
    ADD COLUMN IF NOT EXISTS place_name TEXT,
    ADD COLUMN IF NOT EXISTS timezone TEXT;

ALTER TABLE lawn_profiles
    ADD CONSTRAINT chk_lawn_location_complete
    CHECK ((latitude IS NULL) = (longitude IS NULL) AND (latitude IS NULL) = (timezone IS NULL));
