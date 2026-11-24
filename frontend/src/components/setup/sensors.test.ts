import { describe, expect, it } from "vitest";
import type { Checked } from "../../types/sensors";
import {
	checkLine,
	choiceFor,
	exampleRequest,
	INPUT_CHOICES,
	mappingOf,
	testedText,
} from "./sensors";

// @spec SETUP-MAP-003
describe("the inputs a sensor can be mapped to", () => {
	it("are air temperature, humidity, rain as a counter or an amount, and soil at five depths", () => {
		expect(INPUT_CHOICES.map((c) => c.label)).toEqual([
			"Air temperature",
			"Humidity",
			"Rain, as a counter",
			"Rain, as an amount",
			...[5, 10, 20, 50, 100].map((cm) => `Soil temperature, ${cm} cm`),
			...[5, 10, 20, 50, 100].map((cm) => `Soil moisture, ${cm} cm`),
		]);
	});

	it("name rain's kind in the request and nothing more for the rest", () => {
		expect(mappingOf("rain:Counter")).toEqual({
			input: "rain",
			rain_kind: "Counter",
		});
		expect(mappingOf("rain:Amount")).toEqual({
			input: "rain",
			rain_kind: "Amount",
		});
		expect(mappingOf("soil_temp_10")).toEqual({ input: "soil_temp_10" });
	});
});

// @spec SETUP-MAP-002
it("starts the form on the input an entity suggests, and on nothing for a thermometer", () => {
	expect(choiceFor("Humidity")).toBe("humidity");
	expect(choiceFor("SoilMoisture")).toBe("");
	expect(choiceFor("RainCounter")).toBe("rain:Counter");
	expect(choiceFor("RainAmount")).toBe("rain:Amount");
	expect(choiceFor(null)).toBe("");
});

// @spec SETUP-HA-001
it("says what answered a test and how many numeric sensors it has", () => {
	expect(
		testedText({
			answered: { location_name: "Home", version: "2026.11.1" },
			numeric_sensors: 312,
		}),
	).toBe("Home (2026.11.1) answered; 312 numeric sensors.");
	expect(
		testedText({
			answered: { location_name: "", version: "" },
			numeric_sensors: 1,
		}),
	).toBe("Home Assistant answered; 1 numeric sensor.");
});

// @spec SETUP-MAP-004
describe("the check before saving", () => {
	const checked = (over: Partial<Checked>): Checked => ({
		value: 61.24,
		unit: "°F",
		reported_at: "2026-11-24T20:15:00Z",
		otherwise: {
			value: 58,
			observed_at: "2026-11-24T20:00:00Z",
			basis: {
				kind: "Estimate",
				source: "Hourly analysis",
				distance_miles: null,
				approximate: true,
			},
		},
		blocking: [],
		warnings: [],
		...over,
	});

	it("puts the sensor's value beside the value the lawn would otherwise use, with its basis", () => {
		expect(checkLine(checked({}), "air_temp")).toBe(
			"Your sensor 61.2°F · Estimated for your location · approximate 58.0°F",
		);
		expect(
			checkLine(checked({ value: 0.23, otherwise: null }), "soil_moisture_10"),
		).toBe("Your sensor 23% · nothing else measures this here");
	});

	it("says nothing when the sensor has no usable value", () => {
		expect(checkLine(checked({ value: null }), "air_temp")).toBeNull();
	});
});

// @spec SETUP-MAP-010, SETUP-MAP-014
describe("the request a hub sends readings with", () => {
	it("is addressed to this server with $SENSOR_TOKEN for the token and a reading that fits the input", () => {
		expect(
			exampleRequest("https://turf.example", "Garden probe", "soil_temp_10"),
		).toBe(
			[
				"curl -X POST https://turf.example/api/v1/sensor-readings \\",
				'  -H "Authorization: Bearer $SENSOR_TOKEN" \\',
				'  -H "Content-Type: application/json" \\',
				`  -d '{"readings": [{"sensor": "Garden probe", "value": 55.4, "unit": "°F"}]}'`,
			].join("\n"),
		);
		expect(exampleRequest("http://x", "Gauge", "rain:Counter")).toContain(
			'"value": 1.25, "unit": "in"',
		);
		expect(exampleRequest("http://x", "Probe", "soil_moisture_5")).toContain(
			'"value": 23, "unit": "%"',
		);
		expect(exampleRequest("http://x", "Shade", "humidity")).toContain(
			'"value": 64, "unit": "%"',
		);
	});
});
