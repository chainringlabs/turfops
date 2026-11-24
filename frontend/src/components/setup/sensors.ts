// What the sensors step offers and says: the inputs a sensor can be mapped to, the input
// an entity suggests, a test's result, the check before saving, and the request a hub
// sends readings with.
import type {
	Checked,
	RainKind,
	Suggestion,
	TestResult,
} from "../../types/sensors";
import { basisText, formatValue } from "../sources/display";

/** One input a sensor can be mapped to; rain is offered as a counter and as an amount. */
export interface InputChoice {
	/** `air_temp`, or `rain:Counter` / `rain:Amount` for rain. */
	value: string;
	label: string;
}

const DEPTHS = [5, 10, 20, 50, 100];

// @spec SETUP-MAP-003
export const INPUT_CHOICES: InputChoice[] = [
	{ value: "air_temp", label: "Air temperature" },
	{ value: "humidity", label: "Humidity" },
	{ value: "rain:Counter", label: "Rain, as a counter" },
	{ value: "rain:Amount", label: "Rain, as an amount" },
	...DEPTHS.map((cm) => ({
		value: `soil_temp_${cm}`,
		label: `Soil temperature, ${cm} cm`,
	})),
	...DEPTHS.map((cm) => ({
		value: `soil_moisture_${cm}`,
		label: `Soil moisture, ${cm} cm`,
	})),
];

/** The label of a mapped sensor's input, with rain's kind. */
export function choiceLabel(input: string, rainKind: RainKind | null): string {
	const value = rainKind ? `${input}:${rainKind}` : input;
	return INPUT_CHOICES.find((c) => c.value === value)?.label ?? input;
}

/**
 * The choice an entity's suggestion starts the form on. Soil moisture and thermometers
 * start on nothing: the owner picks the depth, or air or soil.
 */
// @spec SETUP-MAP-002
export function choiceFor(suggestion: Suggestion | null): string {
	switch (suggestion) {
		case "Humidity":
			return "humidity";
		case "RainCounter":
			return "rain:Counter";
		case "RainAmount":
			return "rain:Amount";
		default:
			return "";
	}
}

/** A choice as the mapping request names it. */
export function mappingOf(choice: string): {
	input: string;
	rain_kind?: RainKind;
} {
	const [input, kind] = choice.split(":");
	return kind ? { input, rain_kind: kind as RainKind } : { input };
}

/** What answered a test, and how many numeric sensors it has. */
// @spec SETUP-HA-001
export function testedText(result: TestResult): string {
	const { location_name, version } = result.answered;
	const who = location_name
		? `${location_name}${version ? ` (${version})` : ""}`
		: "Home Assistant";
	const n = result.numeric_sensors;
	return `${who} answered; ${n} numeric sensor${n === 1 ? "" : "s"}.`;
}

/** The sensor's value beside the value the lawn would otherwise use, with its basis. */
// @spec SETUP-MAP-004
export function checkLine(checked: Checked, input: string): string | null {
	if (checked.value === null) return null;
	const mine = `Your sensor ${formatValue(input, checked.value)}`;
	const other = checked.otherwise;
	if (!other) return `${mine} · nothing else measures this here`;
	return `${mine} · ${basisText(other.basis)} ${formatValue(input, other.value)}`;
}

/** A reading that fits an input, for the example request. */
function exampleReading(input: string): { value: number; unit: string } {
	if (input === "rain") return { value: 1.25, unit: "in" };
	if (input === "humidity") return { value: 64, unit: "%" };
	if (input.startsWith("soil_moisture_")) return { value: 23, unit: "%" };
	if (input.startsWith("soil_temp_")) return { value: 55.4, unit: "°F" };
	return { value: 61.2, unit: "°F" };
}

/** The request a hub or script sends a reading to a sensor with. */
// @spec SETUP-MAP-010, SETUP-MAP-014
export function exampleRequest(
	origin: string,
	name: string,
	choice: string,
): string {
	const { value, unit } = exampleReading(mappingOf(choice).input);
	const body = `{"readings": [{"sensor": ${JSON.stringify(name)}, "value": ${value}, "unit": "${unit}"}]}`;
	return [
		`curl -X POST ${origin}/api/v1/sensor-readings \\`,
		'  -H "Authorization: Bearer $SENSOR_TOKEN" \\',
		'  -H "Content-Type: application/json" \\',
		`  -d '${body}'`,
	].join("\n");
}
