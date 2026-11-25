-- The seasonal plan's 10 cm crossings are computed from the weather record on request and
-- held in memory; nothing reads these tables (PLAN-CROSS-016).
DROP TABLE IF EXISTS seasonal_threshold_crossings;
DROP TABLE IF EXISTS seasonal_crossing_years;
