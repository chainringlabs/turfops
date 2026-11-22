pub mod data_sync;
pub mod disease;
pub mod follow_up;
pub mod gdd;
pub mod inventory;
pub mod plant_maintenance;
pub mod rules;
pub mod seasonal_plan;
// Consumed by the weather record, the outlooks and the scoring command as they land.
#[allow(dead_code)]
pub mod soil_estimate;
pub mod soil_temp_prediction;
pub mod soil_test_recommendations;
pub mod soil_test_thresholds;
pub mod timing;
