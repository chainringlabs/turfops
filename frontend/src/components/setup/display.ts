// What the setup steps say: the plan in words, the nearest soil station, a station to
// pin, and typed coordinates.
import type { Preview, StationChoice } from "../../types/setup";
import type { Standing } from "../../types/sources";

/** "a", "a and b", "a, b and c". */
function andList(items: string[]): string {
	if (items.length < 2) return items.join("");
	return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

/** The inputs named for a sentence: soil depths together, the rest by label. */
function named(inputs: Preview["inputs"]): string {
	const parts: string[] = [];
	const depths = (prefix: string) =>
		inputs
			.filter((i) => i.input.startsWith(prefix))
			.map((i) => i.input.slice(prefix.length));
	for (const input of inputs) {
		if (!input.input.startsWith("soil_")) parts.push(input.label.toLowerCase());
	}
	const temp = depths("soil_temp_");
	if (temp.length) parts.push(`soil temperature at ${andList(temp)} cm`);
	const moisture = depths("soil_moisture_");
	if (moisture.length) parts.push(`soil moisture at ${andList(moisture)} cm`);
	return parts.join(", ");
}

const SUMMARY: [Standing, string][] = [
	["Measured", "Measured"],
	["Estimated", "Estimated"],
	["NoSource", "No source here"],
];

/** The plan in words: what is measured, what is estimated, what has no source here. */
// @spec SETUP-PLAN-001
export function planSummary(inputs: Preview["inputs"]): string[] {
	return SUMMARY.flatMap(([standing, heading]) => {
		const these = inputs.filter((i) => i.standing === standing);
		return these.length ? [`${heading}: ${named(these)}.`] : [];
	});
}

/** "Nearest soil station: <name>, <state> (<network>), <N> mi", or that none was found. */
// @spec SETUP-LOC-003
export function nearestSoilText(station: StationChoice | null): string {
	if (!station) return "No soil station found yet.";
	return `Nearest soil station: ${station.name}, ${station.state} (${station.network}), ${Math.round(station.distance_miles)} mi`;
}

/** One station the owner may pin: network, name, distance, elevation and years. */
// @spec SETUP-PLAN-002
export function stationLine(station: StationChoice): string {
	const parts = [
		station.network,
		`${station.name}, ${station.state}`,
		`${Math.round(station.distance_miles)} mi`,
	];
	const feet = station.elevation_difference_ft;
	if (feet !== null) {
		const rounded = Math.round(Math.abs(feet));
		parts.push(
			rounded === 0
				? "same height"
				: `${rounded} ft ${feet > 0 ? "higher" : "lower"}`,
		);
	}
	if (station.first_year !== null)
		parts.push(`${station.first_year}–${station.last_year ?? "present"}`);
	return parts.join(" · ");
}

const EXAMPLES = { Latitude: "39.86", Longitude: "-75.79" };

/** A typed latitude or longitude, or why it is not one. */
// @spec SETUP-LOC-001
export function parseCoordinate(
	text: string,
	what: "Latitude" | "Longitude",
): { value: number } | { problem: string } {
	const trimmed = text.trim();
	if (!trimmed) return { problem: `Enter a ${what.toLowerCase()}.` };
	const value = Number(trimmed);
	if (!Number.isFinite(value))
		return { problem: `${what} must be a number, such as ${EXAMPLES[what]}.` };
	return { value };
}
