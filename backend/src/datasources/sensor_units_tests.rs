use super::*;

// @spec WX-SRC-HA-001, WX-SRC-NORM-001
#[test]
fn a_reading_is_converted_from_the_unit_its_sensor_declares() {
    assert_eq!(convert(Input::AirTemp, 20.0, Some("°C")), Some(68.0));
    assert_eq!(convert(Input::SoilTemp10, 61.0, Some("°F")), Some(61.0));
    assert_eq!(convert(Input::Humidity, 55.0, Some("%")), Some(55.0));
    assert_eq!(convert(Input::SoilMoisture10, 23.0, Some("%")), Some(0.23));
    assert_eq!(
        convert(Input::SoilMoisture10, 0.23, Some("m³/m³")),
        Some(0.23)
    );
    assert_eq!(
        convert(Input::SoilMoisture10, 0.23, None),
        Some(0.23),
        "a bare fraction"
    );
    assert_eq!(convert(Input::Rain, 0.5, Some("in")), Some(12.7));
    assert_eq!(convert(Input::Rain, 3.0, Some("mm")), Some(3.0));
}

// @spec WX-SRC-HA-001
#[test]
fn a_unit_the_input_does_not_accept_is_no_reading() {
    assert_eq!(convert(Input::AirTemp, 293.0, Some("K")), None);
    assert_eq!(convert(Input::AirTemp, 20.0, None), None);
    assert_eq!(convert(Input::Humidity, 0.55, None), None);
    assert_eq!(convert(Input::Rain, 3.0, Some("cm")), None);
    assert_eq!(
        convert(Input::DailyAir, 60.0, Some("°F")),
        None,
        "not a sensor's input"
    );
    assert!(!mappable(Input::DailyAir) && mappable(Input::SoilMoisture100));
    assert_eq!(accepted_units(Input::Rain), "mm or in");
}

// @spec WX-SRC-NORM-002
#[test]
fn a_value_outside_what_its_input_can_be_is_not_plausible() {
    assert!(plausible(Input::AirTemp, 150.0) && !plausible(Input::AirTemp, 150.1));
    assert!(plausible(Input::Humidity, 0.0) && !plausible(Input::Humidity, 100.5));
    assert!(plausible(Input::SoilMoisture5, 1.0) && !plausible(Input::SoilMoisture5, 23.0));
    assert!(plausible(Input::Rain, 1234.0) && !plausible(Input::Rain, -0.1));
}
