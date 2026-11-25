//! The live reading against PostgreSQL.

use super::lab::*;
use super::live::{LiveReading, LiveValue};
use crate::db::weather_queries::{days, sensors};
use crate::models::weather_basis::{Basis, BasisKind};
use crate::models::weather_record::{HourlyRow, Input};
use chrono::Duration;
use chrono_tz::America::New_York;
use sqlx::PgPool;

// @spec WX-REC-RES-003, WX-REC-RES-004, WX-REC-BASIS-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn each_input_takes_its_newest_value_from_the_source_that_still_reports_it(pool: PgPool) {
    let lab = Lab::new(pool).await;
    lab.station_hours(19, 24, 70.0).await;
    // The probe skipped the station's last hour; the reading is the hour before.
    let last = days::start_of_day(New_York, d(20));
    lab.write_hours(
        STATION,
        &[HourlyRow {
            observed_at: last,
            ..Default::default()
        }],
    )
    .await;
    lab.analysis_hours(20, 11, 81.0, 55.0, 0.0).await;
    let reading = lab.record.fetch_latest(noon(20)).await.unwrap().unwrap();

    let soil = reading.get(Input::SoilTemp10).unwrap();
    assert_eq!(soil.value, 70.0);
    assert_eq!(soil.observed_at, last - Duration::hours(1));
    assert_eq!(soil.basis.kind, BasisKind::Station);
    assert_eq!(reading.get(Input::SoilTemp5).unwrap().value, 72.0);

    let air = reading.get(Input::AirTemp).unwrap();
    assert_eq!((air.value, air.basis.approximate), (81.0, true));
    assert_eq!(air.observed_at, last + Duration::hours(10));
    assert_eq!(reading.get(Input::Humidity).unwrap().value, 55.0);
    assert!(
        reading.get(Input::SoilMoisture10).is_none(),
        "nothing measures it"
    );
}

// @spec WX-REC-RES-003, WX-REC-RES-004, SYNC-AGE-002, SYNC-AGE-004
#[sqlx::test(migrations = "src/db/migrations")]
async fn past_a_stations_age_limit_the_estimate_speaks_with_its_newest_day(pool: PgPool) {
    let lab = Lab::new(pool).await;
    lab.station_hours(16, 24, 70.0).await;
    lab.estimate(18, 60.0).await;
    lab.estimate(19, 61.0).await;
    let reading = lab.record.fetch_latest(noon(20)).await.unwrap().unwrap();
    let soil = reading.get(Input::SoilTemp10).unwrap();
    assert_eq!(soil.value, 61.0);
    assert_eq!(
        soil.observed_at,
        days::start_of_day(New_York, d(19)),
        "stamped with its day"
    );
    assert_eq!(soil.basis.source, "Soil temperature estimate");
}

// @spec WX-REC-RES-003, WX-REC-FAIL-005
#[sqlx::test(migrations = "src/db/migrations")]
async fn an_input_whose_every_source_is_too_old_has_no_current_value(pool: PgPool) {
    let lab = Lab::new(pool).await;
    lab.station_hours(10, 24, 70.0).await;
    lab.estimate(15, 60.0).await;
    lab.analysis_hours(20, 8, 81.0, 55.0, 0.0).await; // newest hour 07:00, five hours old
    assert_eq!(lab.record.fetch_latest(noon(20)).await.unwrap(), None);
    // The estimate's day ended at midnight on the 16th: 72 hours later it is still current.
    let reading = lab.record.fetch_latest(noon(18)).await.unwrap().unwrap();
    assert_eq!(reading.values.len(), 2, "5 and 10 cm soil: {reading:?}");
}

// @spec WX-REC-RES-003, WX-REC-RES-004, SYNC-AGE-001
#[sqlx::test(migrations = "src/db/migrations")]
async fn the_owners_sensor_speaks_with_its_last_reading_until_it_goes_quiet(pool: PgPool) {
    let lab = Lab::new(pool).await;
    let sensor = lab.map_sensor(Input::AirTemp).await;
    lab.analysis_hours(20, 11, 81.0, 55.0, 0.0).await;
    let now = noon(20).at.now;
    for (minutes_ago, value) in [(50, 77.0), (20, 78.5)] {
        let at = now - Duration::minutes(minutes_ago);
        sensors::record_reading(&lab.pool, sensor.id, Input::AirTemp, at, value)
            .await
            .unwrap();
    }
    let reading = lab.record.fetch_latest(noon(20)).await.unwrap().unwrap();
    let air = reading.get(Input::AirTemp).unwrap();
    assert_eq!(
        (air.value, air.observed_at),
        (78.5, now - Duration::minutes(20))
    );
    assert_eq!(
        (air.basis.kind, air.basis.source.as_str()),
        (BasisKind::Sensor, "Patio")
    );
    assert_eq!(
        reading.get(Input::Humidity).unwrap().basis.kind,
        BasisKind::Estimate
    );

    let mut later = noon(20);
    later.at.now = now + Duration::minutes(45); // the reading is now 65 minutes old
    lab.analysis_hours(20, 12, 81.0, 55.0, 0.0).await;
    let reading = lab.record.fetch_latest(later).await.unwrap().unwrap();
    assert_eq!(
        reading.get(Input::AirTemp).unwrap().basis.kind,
        BasisKind::Estimate
    );
}

// @spec WX-REC-API-012
#[test]
fn the_primary_soil_moisture_is_10_cm_then_5_then_20() {
    let value = |value| LiveValue {
        value,
        observed_at: noon(20).at.now,
        basis: Basis {
            kind: BasisKind::Station,
            source: "A".into(),
            distance_miles: None,
            approximate: false,
        },
    };
    let reading = |inputs: &[(Input, f64)]| LiveReading {
        values: inputs
            .iter()
            .map(|(input, v)| (*input, value(*v)))
            .collect(),
    };
    let all = reading(&[
        (Input::SoilMoisture20, 0.3),
        (Input::SoilMoisture5, 0.2),
        (Input::SoilMoisture10, 0.1),
    ]);
    assert_eq!(all.primary_soil_moisture().unwrap().value, 0.1);
    let no_ten = reading(&[(Input::SoilMoisture20, 0.3), (Input::SoilMoisture5, 0.2)]);
    assert_eq!(no_ten.primary_soil_moisture().unwrap().value, 0.2);
    let deep = reading(&[(Input::SoilMoisture50, 0.3)]);
    assert!(deep.primary_soil_moisture().is_none());
}
