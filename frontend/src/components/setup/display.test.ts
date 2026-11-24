import { describe, expect, it } from "vitest";
import { pinsParam } from "../../api/client";
import type { Preview, StationChoice } from "../../types/setup";
import {
	nearestSoilText,
	parseCoordinate,
	planSummary,
	stationLine,
} from "./display";

function row(
	input: string,
	label: string,
	standing: Preview["inputs"][number]["standing"],
): Preview["inputs"][number] {
	return { input, label, standing, sources: [] };
}

const station: StationChoice = {
	station_ref: 7,
	network: "USCRN",
	name: "PA Avondale 2 N",
	state: "PA",
	distance_miles: 3.24,
	elevation_difference_ft: 120.4,
	first_year: 2008,
	last_year: null,
};

// @spec SETUP-PLAN-001
describe("the plan in words", () => {
	it("says what is measured, what is estimated and what has no source, soil by depth", () => {
		const lines = planSummary([
			row("air_temp", "Air temperature", "Measured"),
			row("humidity", "Humidity", "Measured"),
			row("rain", "Rain", "Estimated"),
			row("daily_air", "Daily high and low", "Estimated"),
			row("soil_temp_5", "Soil temperature, 5 cm", "Measured"),
			row("soil_temp_10", "Soil temperature, 10 cm", "Measured"),
			row("soil_moisture_20", "Soil moisture, 20 cm", "NoSource"),
			row("soil_moisture_50", "Soil moisture, 50 cm", "NoSource"),
			row("soil_moisture_100", "Soil moisture, 100 cm", "NoSource"),
		]);
		expect(lines).toEqual([
			"Measured: air temperature, humidity, soil temperature at 5 and 10 cm.",
			"Estimated: rain, daily high and low.",
			"No source here: soil moisture at 20, 50 and 100 cm.",
		]);
	});

	it("leaves out a kind that has nothing in it", () => {
		expect(planSummary([row("rain", "Rain", "Estimated")])).toEqual([
			"Estimated: rain.",
		]);
	});
});

// @spec SETUP-LOC-003
it("names the nearest soil station with its distance, or says none was found", () => {
	expect(nearestSoilText(station)).toBe(
		"Nearest soil station: PA Avondale 2 N, PA (USCRN), 3 mi",
	);
	expect(nearestSoilText(null)).toBe("No soil station found yet.");
});

// @spec SETUP-PLAN-002
describe("a station to pin", () => {
	it("gives network, name, distance, elevation difference from the lawn and years", () => {
		expect(stationLine(station)).toBe(
			"USCRN · PA Avondale 2 N, PA · 3 mi · 120 ft higher · 2008–present",
		);
		expect(
			stationLine({
				...station,
				elevation_difference_ft: -45.6,
				last_year: 2019,
			}),
		).toBe("USCRN · PA Avondale 2 N, PA · 3 mi · 46 ft lower · 2008–2019");
		expect(
			stationLine({
				...station,
				elevation_difference_ft: null,
				first_year: null,
				last_year: null,
			}),
		).toBe("USCRN · PA Avondale 2 N, PA · 3 mi");
	});
});

// @spec SETUP-LOC-001
describe("typed coordinates", () => {
	it("takes a number and says what is wrong with anything else", () => {
		expect(parseCoordinate(" 39.86 ", "Latitude")).toEqual({ value: 39.86 });
		expect(parseCoordinate("", "Latitude")).toEqual({
			problem: "Enter a latitude.",
		});
		expect(parseCoordinate("north", "Longitude")).toEqual({
			problem: "Longitude must be a number, such as -75.79.",
		});
	});
});

// @spec SETUP-PLAN-003
it("sends pins as input:station pairs", () => {
	expect(pinsParam({ soil_temp_10: 7, air_temp: 12 })).toBe(
		"soil_temp_10:7,air_temp:12",
	);
	expect(pinsParam({})).toBe("");
});
