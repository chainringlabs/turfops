-- The soil estimate's current fit for each depth: what it was fitted on and how well.
-- One row per depth; a refit replaces it.
CREATE TABLE soil_estimate_fit (
    depth_cm INTEGER PRIMARY KEY CHECK (depth_cm IN (5, 10)),
    calibration TEXT NOT NULL CHECK (calibration IN ('NearestFive', 'StationAlone')),
    -- The model's eight coefficients, in term order.
    coefficients DOUBLE PRECISION[] NOT NULL CHECK (cardinality(coefficients) = 8),
    -- The stations fitted on, as a JSON list of {station_ref, name, distance_miles}.
    stations TEXT NOT NULL,
    training_days INTEGER NOT NULL,
    rmse_f DOUBLE PRECISION NOT NULL,
    fitted_at TIMESTAMPTZ NOT NULL
);
