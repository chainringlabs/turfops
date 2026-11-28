use super::thresholds::*;
use super::Rule;
use crate::clock::Moment;
use crate::models::{
    Application, EnvironmentalSummary, LawnProfile, ProductCategory, ProductNeed, Recommendation,
    RecommendationCategory, Severity,
};

/// Irrigation forecast rule: water when no real rain is forecast for 5 days and the soil is
/// dry. With soil moisture, below 0.20 (Advisory), 0.15 (Warning) or 0.10 (Critical).
/// Without it, judged on rain while the grass grows: under ½ in this week (Advisory) or
/// under 5 mm (Warning), never Critical.
pub struct IrrigationForecastRule;

/// What the rule judged dryness on.
#[derive(Debug, Clone, Copy)]
enum Judged {
    /// The live soil moisture, as a fraction.
    Moisture(f64),
    /// The last 7 days' rain in mm, where nothing measures soil moisture.
    Rain(f64),
}

/// Fewer forecast days than this with a rain amount is too little to call a dry spell.
const MIN_DAYS_WITH_AMOUNT: usize = 3;

// @spec RULE-WX-002, RULE-WX-003, RULE-WX-009, RULE-WX-010
impl Rule for IrrigationForecastRule {
    fn evaluate(
        &self,
        env: &EnvironmentalSummary,
        profile: &LawnProfile,
        _history: &[Application],
        at: Moment,
    ) -> Option<Recommendation> {
        let forecast = env.forecast.as_ref()?;
        let moisture = env
            .current
            .as_ref()
            .and_then(|current| current.primary_soil_moisture());
        let judged = match moisture {
            Some(moisture) if moisture >= SOIL_MOISTURE_ADEQUATE => return None,
            Some(moisture) => Judged::Moisture(moisture),
            // Nothing measures soil moisture here: judge on the week's rain, only while
            // the grass is growing, and only with both weekly figures in hand.
            None => {
                let week_air = env.ambient_temp_7day_avg_f?;
                let week_rain = env.precipitation_7day_total_mm?;
                if week_air < IRRIGATION_GROWING_AIR_F || week_rain >= IRRIGATION_DRY_WEEK_MM {
                    return None;
                }
                Judged::Rain(week_rain)
            }
        };

        // Only the forecast *amount* counts: a 50% chance of a trace won't relieve dry soil.
        // The forecast amount over the next 5 days, from the days that carry one.
        // @spec RULE-ENG-005, RULE-WX-011
        let amounts: Vec<f64> = forecast
            .next_days(at.today, 5)
            .iter()
            .filter_map(|d| d.total_precipitation_mm)
            .collect();
        if amounts.len() < MIN_DAYS_WITH_AMOUNT {
            return None;
        }

        // If meaningful rain expected (>2.5mm / 0.1"), no recommendation
        if amounts.iter().sum::<f64>() > PRECIP_TRACE_MM {
            return None;
        }

        let severity = match judged {
            Judged::Moisture(m) if m < SOIL_MOISTURE_DROUGHT => Severity::Critical,
            Judged::Moisture(m) if m < SOIL_MOISTURE_IRRIGATION_WARNING => Severity::Warning,
            Judged::Moisture(_) => Severity::Advisory,
            // Rain alone cannot justify Critical.
            Judged::Rain(mm) if mm < IRRIGATION_VERY_DRY_WEEK_MM => Severity::Warning,
            Judged::Rain(_) => Severity::Advisory,
        };

        // Days until the next day that is not dry (if any)
        // @spec RULE-WX-013
        let dry_days = forecast
            .daily_summary
            .iter()
            .take_while(|d| is_dry(d))
            .count();

        Some(self.build_recommendation(env, severity, judged, dry_days, profile))
    }
}

impl IrrigationForecastRule {
    // @spec RULE-WX-004
    fn build_recommendation(
        &self,
        env: &EnvironmentalSummary,
        severity: Severity,
        judged: Judged,
        dry_days: usize,
        _profile: &LawnProfile,
    ) -> Recommendation {
        let title = match severity {
            Severity::Critical => "Irrigation Urgently Needed",
            Severity::Warning => "Irrigation Recommended Soon",
            _ => "Consider Irrigation",
        };

        let description = match judged {
            Judged::Moisture(moisture) => format!(
                "Soil moisture is low ({:.0}%) and no significant rain is forecasted for {} days. \
                 Cool-season grasses need consistent moisture.",
                moisture * 100.0,
                dry_days
            ),
            Judged::Rain(mm) => format!(
                "Only {:.2}\" of rain fell in the last 7 days and no significant rain is \
                 forecasted for {} days. This is judged on rainfall, because nothing measures \
                 soil moisture at your lawn.",
                mm / 25.4,
                dry_days
            ),
        };

        let action = match severity {
            Severity::Critical => {
                "Water immediately. Apply 1-1.5 inches over the next 2-3 days to prevent \
                 drought stress. Water early morning (4-7 AM) to minimize evaporation and disease."
            }
            Severity::Warning => {
                "Plan to irrigate within the next 1-2 days. Apply 1 inch of water per session. \
                 Deep, infrequent watering encourages deeper root growth — better than shallow daily watering."
            }
            _ if matches!(judged, Judged::Rain(_)) => {
                "Check the soil with a trowel: if the top few inches are dry, plan a deep \
                 watering in the early morning."
            }
            _ => {
                "Monitor soil moisture and plan irrigation if conditions don't change. \
                 Consider a deep watering session in early morning."
            }
        };
        let explanation = match judged {
            Judged::Moisture(_) => {
                "Tall Fescue requires 1-1.5 inches of water per week during the growing season. \
                 When soil moisture drops below 10-15% and no rain is expected, supplemental \
                 irrigation prevents drought stress, thinning, and weed invasion. Water deeply \
                 (to 6 inches) to encourage deep root growth."
            }
            Judged::Rain(_) => {
                "A growing cool-season lawn uses about an inch of water a week; under half an \
                 inch of rain with none coming leaves it short. With no station within reach \
                 and no probe, nothing measures soil moisture at your lawn, so this is judged on \
                 rainfall and never goes past a Warning. A soil-moisture probe mapped in Setup \
                 would replace this judgement with a reading."
            }
        };

        let rec = Recommendation::new(
            "irrigation_forecast",
            RecommendationCategory::Irrigation,
            severity,
            title,
            description,
        )
        .with_explanation(explanation);
        let rec = match judged {
            Judged::Moisture(moisture) => rec.with_weather_point(
                "Soil Moisture",
                format!("{:.0}%", moisture * 100.0),
                env.measured_soil_moisture(),
            ),
            Judged::Rain(mm) => rec.with_weather_point(
                "Rain, last 7 days",
                format!("{:.2}\"", mm / 25.4),
                env.measured_week("precipitation_7day_total_mm"),
            ),
        };
        rec.with_weather_point(
            "Dry Days Forecast",
            format!("{} days", dry_days),
            env.measured_forecast(),
        )
        .with_action(action)
        // Nice to have on dry, water-repellent spots; never a reason to shop.
        .with_need(
            ProductNeed::new("wetting agent (optional)", ProductCategory::Surfactant).optional(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DailyForecast, EnvironmentalReading, ForecastProvider, GrassType, WeatherCondition,
        WeatherForecast,
    };
    use chrono::{Duration, Utc};

    fn profile() -> LawnProfile {
        LawnProfile {
            id: Some(1),
            name: "Test".into(),
            grass_type: GrassType::TallFescue,
            usda_zone: "7a".into(),
            soil_type: None,
            lawn_size_sqft: Some(5000.0),
            irrigation_type: None,
            location: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Dry soil (8%) plus a forecast of one day per `(amount in mm, probability)`, from
    /// today.
    fn env(days: &[(Option<f64>, Option<f64>)]) -> EnvironmentalSummary {
        let mut reading = EnvironmentalReading::new(crate::models::DataSource::SoilData);
        reading.soil_moisture_10 = Some(0.08);
        let today = crate::clock::Clock::System.today();
        let daily_summary = days
            .iter()
            .enumerate()
            .map(|(i, (mm, prob))| DailyForecast {
                date: today + Duration::days(i as i64),
                high_temp_f: 85.0,
                low_temp_f: 65.0,
                avg_humidity: 50.0,
                total_precipitation_mm: *mm,
                max_precipitation_prob: *prob,
                dominant_condition: WeatherCondition::default(),
                avg_wind_speed_mph: 5.0,
                max_wind_gust_mph: None,
                hours_covered: 24,
            })
            .collect();
        EnvironmentalSummary {
            current: Some(reading),
            forecast: Some(WeatherForecast {
                fetched_at: Utc::now(),
                provider: ForecastProvider::OpenWeatherMap,
                hourly: Vec::new(),
                daily_summary,
            }),
            ..Default::default()
        }
    }

    fn judge(days: &[(Option<f64>, Option<f64>)]) -> Option<Recommendation> {
        IrrigationForecastRule.evaluate(
            &env(days),
            &profile(),
            &[],
            crate::clock::Clock::System.moment(),
        )
    }

    const DRY: (Option<f64>, Option<f64>) = (Some(0.0), Some(0.0));
    /// A day the provider forecasts no amount for, at `probability`.
    fn no_amount(probability: f64) -> (Option<f64>, Option<f64>) {
        (None, Some(probability))
    }

    // @spec RULE-WX-003
    #[test]
    fn a_trace_or_a_coin_flip_does_not_cancel_a_drought_alert() {
        // 0.3 mm of drizzle and one 50% day with no accumulation: still bone dry.
        let rec = judge(&[
            (Some(0.3), Some(0.2)),
            (Some(0.0), Some(0.5)),
            DRY,
            DRY,
            DRY,
        ])
        .expect("dry soil with no meaningful rain needs irrigation");
        assert_eq!(rec.severity, Severity::Critical);
    }

    // @spec RULE-WX-011
    #[test]
    fn the_amount_is_summed_over_the_days_that_carry_one() {
        let rec = judge(&[
            DRY,
            DRY,
            DRY,
            no_amount(0.2),
            no_amount(0.9),
            no_amount(0.9),
        ]);
        assert!(rec.is_some(), "three dry days with amounts are enough");
        let wet = judge(&[
            DRY,
            (Some(3.0), Some(0.8)),
            DRY,
            no_amount(0.2),
            no_amount(0.2),
        ]);
        assert!(wet.is_none(), "3 mm on a day that carries an amount");
    }

    // @spec RULE-WX-011
    #[test]
    fn fewer_than_three_days_with_an_amount_is_too_little_to_call_a_dry_spell() {
        let rec = judge(&[DRY, DRY, no_amount(0.1), no_amount(0.1), no_amount(0.1)]);
        assert!(rec.is_none());
    }

    // @spec RULE-WX-013
    #[test]
    fn dry_days_ahead_stop_at_the_first_day_that_is_not_dry() {
        let text = |days: &[(Option<f64>, Option<f64>)]| {
            let rec = judge(days).expect("fires");
            format!("{} {:?}", rec.description, rec.data_points)
        };
        let amounts_only = text(&[DRY, DRY, DRY, no_amount(0.2), no_amount(0.6), DRY]);
        assert!(amounts_only.contains("4 days"), "{amounts_only}");
        let none_at_all = text(&[DRY, DRY, DRY, (None, None), DRY]);
        assert!(none_at_all.contains("3 days"), "{none_at_all}");
    }
}
