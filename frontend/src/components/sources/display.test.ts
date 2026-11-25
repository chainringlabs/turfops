import { describe, expect, it } from "vitest";
import type { InputView, SourceView } from "../../types/sources";
import {
	accuracyLines,
	forecastStatusText,
	forecastUp,
	formatValue,
	groupOf,
	historyText,
	noSourceLine,
	observedText,
	orderSources,
	speaksFor,
} from "./display";

function source(over: Partial<SourceView>): SourceView {
	return {
		id: 1,
		name: "Avondale",
		kind: "Station",
		inputs: ["soil_temp_10"],
		last_success: "2026-11-24T18:35:00Z",
		last_failure: null,
		last_error: null,
		held_from: "2011-01-01",
		held_through: "2026-11-23",
		wanted_from: "2011-01-01",
		attribution: "NOAA",
		...over,
	};
}

function input(name: string, standing: InputView["standing"]): InputView {
	return {
		input: name,
		label: name,
		standing,
		sources: [],
		in_use: null,
		estimate_accuracy: null,
		pinned_station_ref: null,
	};
}

// @spec SETUP-PAGE-009
describe("input groups and values", () => {
	it("groups air, soil temperature and soil moisture", () => {
		expect(["air_temp", "humidity", "rain", "daily_air"].map(groupOf)).toEqual([
			"air",
			"air",
			"air",
			"air",
		]);
		expect(groupOf("soil_temp_50")).toBe("soil_temp");
		expect(groupOf("soil_moisture_5")).toBe("soil_moisture");
	});

	it("shows °F, percent and inches as the rest of the app does", () => {
		expect(formatValue("air_temp", 61.24)).toBe("61.2°F");
		expect(formatValue("soil_temp_10", 48)).toBe("48.0°F");
		expect(formatValue("humidity", 70.4)).toBe("70%");
		expect(formatValue("soil_moisture_5", 0.234)).toBe("23%");
		expect(formatValue("rain", 0.508)).toBe("0.02 in");
	});

	it("gives the observed time in the lawn's timezone and how long ago", () => {
		const now = new Date("2026-11-24T19:23:00Z");
		expect(observedText("2026-11-24T19:05:00Z", now, "America/New_York")).toBe(
			"Nov 24, 2:05 PM · 18 min ago",
		);
		expect(observedText("2026-11-24T14:23:00Z", now, "America/New_York")).toBe(
			"Nov 24, 9:23 AM · 5 h ago",
		);
		expect(observedText("2026-11-20T05:00:00Z", now, "America/Chicago")).toBe(
			"Nov 19, 11:00 PM · 4 d ago",
		);
		expect(observedText("2026-11-24T19:22:40Z", now, "America/New_York")).toBe(
			"Nov 24, 2:22 PM · just now",
		);
	});
});

// @spec SETUP-PAGE-010
describe("sources", () => {
	it("reads history as complete, years held against wanted, or nothing for a sensor", () => {
		expect(historyText(source({}), 2026)).toBe("Complete");
		expect(historyText(source({ held_from: "2019-01-01" }), 2026)).toBe(
			"Loading: 2019–2026 of 2011–2026",
		);
		expect(
			historyText(source({ held_from: null, held_through: null }), 2026),
		).toBe("Loading: nothing yet of 2011–2026");
		expect(historyText(source({ kind: "Sensor" }), 2026)).toBeNull();
	});

	it("puts failing sources first, then sensors, stations, grids, the analysis and the estimate", () => {
		const order = orderSources([
			source({ id: 1, name: "Estimate", kind: "Estimate" }),
			source({ id: 2, name: "PRISM", kind: "Grid" }),
			source({ id: 3, name: "B station", kind: "Station" }),
			source({ id: 4, name: "Analysis", kind: "Analysis" }),
			source({ id: 5, name: "A station", kind: "Station" }),
			source({ id: 6, name: "Patio", kind: "Sensor" }),
			source({ id: 7, name: "Failing grid", kind: "Grid", last_error: "x" }),
		]);
		expect(order.map((s) => s.id)).toEqual([7, 6, 5, 3, 2, 4, 1]);
	});
});

// @spec SETUP-PAGE-002
describe("the estimate's accuracy in plain words", () => {
	it("gives the daily error, the range of crossings it may date and names those it may not", () => {
		const lines = accuracyLines({
			typical_error_f: 1.83,
			fitted_on: "NearestFive",
			crossings: [
				{
					threshold_f: 45,
					direction: "Rising",
					median_days: 2.4,
					p80_days: 9,
					datable: true,
				},
				{
					threshold_f: 55,
					direction: "Rising",
					median_days: 3.1,
					p80_days: 12.2,
					datable: true,
				},
				{
					threshold_f: 70,
					direction: "Falling",
					median_days: 8,
					p80_days: 20,
					datable: false,
				},
				{
					threshold_f: 75,
					direction: "Falling",
					median_days: 9,
					p80_days: 25,
					datable: false,
				},
			],
		});
		expect(lines).toEqual([
			"Daily values typically within 1.8 °F of a measured station.",
			"Crossing dates typically within 2 to 3 days; one year in five, off by 12 days or more.",
			"Not dated by the estimate: 70 °F falling, 75 °F falling.",
		]);
	});

	it("says one figure when the range is one number, and leaves out what does not apply", () => {
		const lines = accuracyLines({
			typical_error_f: 2,
			fitted_on: "StationAlone",
			crossings: [
				{
					threshold_f: 50,
					direction: "Rising",
					median_days: 2.2,
					p80_days: 7.6,
					datable: true,
				},
			],
		});
		expect(lines).toEqual([
			"Daily values typically within 2.0 °F of a measured station.",
			"Crossing dates typically within 2 days; one year in five, off by 8 days or more.",
		]);
		const none = accuracyLines({
			typical_error_f: 3,
			fitted_on: "NearestFive",
			crossings: [],
		});
		expect(none).toEqual([
			"Daily values typically within 3.0 °F of a measured station.",
		]);
	});
});

// @spec SETUP-PAGE-003
describe("inputs nothing measures", () => {
	it("names them in one line per group with what would measure them", () => {
		const moisture = ["5", "10", "20", "50", "100"].map((cm) =>
			input(
				`soil_moisture_${cm}`,
				cm === "5" || cm === "10" ? "Measured" : "NoSource",
			),
		);
		expect(noSourceLine("soil_moisture", moisture)).toBe(
			"Nothing measures soil moisture at 20, 50 or 100 cm here; a probe you map as a sensor would.",
		);
		const temp = [
			input("soil_temp_50", "NoSource"),
			input("soil_temp_5", "Estimated"),
		];
		expect(noSourceLine("soil_temp", temp)).toBe(
			"Nothing measures soil temperature at 50 cm here; a probe you map as a sensor, or a station within 50 miles, would.",
		);
		expect(noSourceLine("air", [input("rain", "NoSource")])).toBe(
			"Nothing measures rain here; a sensor you map would.",
		);
		expect(
			noSourceLine("soil_temp", [input("soil_temp_5", "Measured")]),
		).toBeNull();
	});
});

// @spec SETUP-PAGE-012
it("lists what a source speaks for in the page's order with soil depths together", () => {
	const labels = {
		air_temp: "Air temperature",
		humidity: "Humidity",
		rain: "Rain",
		daily_air: "Daily high and low",
	};
	expect(
		speaksFor(
			[
				"soil_moisture_10",
				"humidity",
				"soil_temp_100",
				"soil_moisture_5",
				"air_temp",
				"soil_temp_5",
				"soil_temp_20",
			],
			labels,
		),
	).toBe(
		"Air temperature, humidity, soil temperature at 5, 20 and 100 cm, soil moisture at 5 and 10 cm",
	);
	expect(speaksFor(["daily_air", "rain"], labels)).toBe(
		"Rain, daily high and low",
	);
	expect(speaksFor(["soil_temp_10"], labels)).toBe("Soil temperature at 10 cm");
});

describe("the forecast's status", () => {
	const now = new Date("2026-11-24T19:30:00Z");
	const status = (fetched_at: string | null, last_error: string | null) => ({
		provider: "OpenMeteo" as const,
		fetched_at,
		last_error,
	});

	// @spec SETUP-PAGE-013
	it("says when the forecast was fetched, why it is failing, or that it has not been", () => {
		expect(
			forecastStatusText(
				status("2026-11-24T19:07:00Z", null),
				now,
				"America/New_York",
			),
		).toBe("Fetched Nov 24, 2:07 PM · 23 min ago");
		expect(
			forecastStatusText(
				status(
					"2026-11-24T17:07:00Z",
					"The weather forecast is unavailable right now.",
				),
				now,
				"America/New_York",
			),
		).toBe("The weather forecast is unavailable right now.");
		expect(
			forecastStatusText(status(null, null), now, "America/New_York"),
		).toBe("Not fetched yet");
	});

	// @spec SYNC-CONN-008
	it("is up only with a forecast held and no failure", () => {
		expect(forecastUp(status("2026-11-24T19:07:00Z", null))).toBe(true);
		expect(forecastUp(status("2026-11-24T19:07:00Z", "Down."))).toBe(false);
		expect(forecastUp(status(null, null))).toBe(false);
		expect(forecastUp(status(null, "Needs its key."))).toBe(false);
	});
});
