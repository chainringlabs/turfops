// What the Environmental page shows beside the numbers: chart reference lines, forecast
// day completeness, the station's observation time, and crossing timing.

import type {
	DailyForecast,
	EnvironmentalSummary,
	ThresholdPrediction,
	TimeSeriesPoint,
} from "../../types";
import { formatMonthDay } from "../../utils/dates";
import { mmToInches } from "../../utils/units";

export interface ReferenceLine {
	value: number;
	label: string;
}

/** Reference lines per trend chart; a chart missing here draws none. Soil has none:
 * pre-emergent and seeding timing are judged on 5 cm soil on the Timing page. */
// @spec WX-VIEW-PAGE-007
export const TREND_REFERENCES: Record<string, ReferenceLine | undefined> = {
	ambient: { value: 85, label: "Heat stress" },
	humidity: { value: 80, label: "Humid (80%)" },
	moisture: { value: 0.1, label: "Drought" },
};

/** Whether a forecast card covers less than the whole day. */
// @spec WX-VIEW-PAGE-009
export function isPartialDay(day: DailyForecast): boolean {
	return day.hours_covered !== undefined && day.hours_covered < 24;
}

/** A forecast card's rain line: the amount, or "not forecast" where the provider gives none. */
// @spec WX-VIEW-PAGE-003
export function forecastPrecip(day: DailyForecast): string {
	const amount =
		day.total_precipitation_mm === null
			? "not forecast"
			: `${mmToInches(day.total_precipitation_mm).toFixed(2)} in`;
	if (day.max_precipitation_prob === null) return `Precip: ${amount}`;
	return `Precip: ${amount} (${(day.max_precipitation_prob * 100).toFixed(0)}%)`;
}

/**
 * When the 10 cm soil in use was observed, worded for its basis: "Station soil observed
 * Oct 10, 2:00 PM", "Sensor soil read Oct 10, 2:05 PM", or "Soil estimated for Oct 10" (an
 * estimate is for a day, whose start is its time). With no soil in use, the newest station
 * reading behind the stale-soil notice. Null with no observation.
 */
// @spec WX-VIEW-PAGE-008
export function soilObservedLine(summary: EnvironmentalSummary): string | null {
	if (!summary.soil_observed_at) return null;
	const at = new Date(summary.soil_observed_at);
	if (Number.isNaN(at.getTime())) return null;
	const kind = summary.current_bases?.find(
		(b) => b.input === "soil_temp_10" && b.standing === "InUse",
	)?.basis?.kind;
	if (kind === "Estimate") {
		const day = at.toLocaleDateString(undefined, {
			month: "short",
			day: "numeric",
		});
		return `Soil estimated for ${day}`;
	}
	const when = at.toLocaleString(undefined, {
		month: "short",
		day: "numeric",
		hour: "numeric",
		minute: "2-digit",
	});
	return kind === "Sensor"
		? `Sensor soil read ${when}`
		: `Station soil observed ${when}`;
}

/**
 * The notice shown while the weather station's soil readings are too old to describe
 * today. The delay is the station network's, so it says there is nothing to fix.
 */
// @spec WX-VIEW-PAGE-008
export function staleSoilMessage(summary: EnvironmentalSummary): string {
	const since = summary.soil_observed_at
		? ` since ${new Date(summary.soil_observed_at).toLocaleDateString()}`
		: " for several days";
	return (
		`The weather station hasn't published new soil readings${since}. ` +
		"The network publishes late from time to time and there is nothing you need to do: " +
		"TurfOps keeps checking and will pick the readings up when they appear. " +
		"Until then soil readings are hidden and soil-based advice is paused."
	);
}

/** The recent line the prediction chart plots: the last `days` days of the series. */
// @spec WX-OUTLOOK-UI-004
export function lastDays(
	points: TimeSeriesPoint[],
	days: number,
): TimeSeriesPoint[] {
	if (points.length === 0) return points;
	const newest = new Date(points[points.length - 1].timestamp).getTime();
	const cutoff = newest - days * 86_400_000;
	return points.filter((p) => new Date(p.timestamp).getTime() >= cutoff);
}

/** "~3 days (Oct 14)", or "beyond the forecast" when the estimate has no date. */
// @spec WX-OUTLOOK-CROSS-005
export function crossingWhen(c: ThresholdPrediction): string {
	if (c.beyond_forecast || !c.estimated_crossing_date) {
		return "beyond the forecast";
	}
	const days = c.days_until_crossing;
	return `~${days} ${days === 1 ? "day" : "days"} (${formatMonthDay(c.estimated_crossing_date)})`;
}
