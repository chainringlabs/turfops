// What the Sources page says about inputs and sources: grouping, values in the app's units,
// times in the lawn's timezone, history, source order and the estimate's accuracy.
import type {
	Basis,
	EstimateAccuracy,
	ForecastStatus,
	InputView,
	SourceKind,
	SourceView,
} from "../../types/sources";
import { BASIS_KIND_LABELS } from "../../types/sources";
import { formatInches } from "../../utils/units";

export type InputGroup = "air" | "soil_temp" | "soil_moisture";

export const INPUT_GROUPS: InputGroup[] = ["air", "soil_temp", "soil_moisture"];

export const INPUT_GROUP_TITLES: Record<InputGroup, string> = {
	air: "Air",
	soil_temp: "Soil temperature",
	soil_moisture: "Soil moisture",
};

/** The group an input belongs to. */
// @spec SETUP-PAGE-009
export function groupOf(input: string): InputGroup {
	if (input.startsWith("soil_temp_")) return "soil_temp";
	if (input.startsWith("soil_moisture_")) return "soil_moisture";
	return "air";
}

/** An input's value in the app's units: °F, percent, or inches of rain. */
// @spec SETUP-PAGE-009
export function formatValue(input: string, value: number): string {
	if (input === "humidity") return `${Math.round(value)}%`;
	if (input.startsWith("soil_moisture_")) return `${Math.round(value * 100)}%`;
	if (input === "rain") return formatInches(value);
	return `${value.toFixed(1)}°F`;
}

const MINUTE_MS = 60_000;

/** How long ago, in the largest whole unit that fits. */
function age(ms: number): string {
	const minutes = Math.floor(ms / MINUTE_MS);
	if (minutes < 1) return "just now";
	if (minutes < 60) return `${minutes} min ago`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours} h ago`;
	return `${Math.floor(hours / 24)} d ago`;
}

/** When a value was observed, in the lawn's timezone, and how long ago. */
// @spec SETUP-PAGE-009
export function observedText(
	observedAt: string,
	now: Date,
	timeZone: string,
): string {
	const at = new Date(observedAt);
	const local = new Intl.DateTimeFormat("en-US", {
		timeZone,
		month: "short",
		day: "numeric",
		hour: "numeric",
		minute: "2-digit",
	})
		.format(at)
		// Newer ICU puts a narrow no-break space before AM/PM.
		.replace(/\u202f/g, " ");
	return `${local} · ${age(now.getTime() - at.getTime())}`;
}

/** A source's history: "Complete", the years held against wanted, or nothing for a sensor. */
// @spec SETUP-PAGE-010
export function historyText(
	source: SourceView,
	thisYear: number,
): string | null {
	if (source.kind === "Sensor" || !source.wanted_from) return null;
	const wanted = source.wanted_from;
	const span = `${wanted.slice(0, 4)}–${thisYear}`;
	if (!source.held_from || !source.held_through)
		return `Loading: nothing yet of ${span}`;
	// ISO dates compare as text.
	if (source.held_from <= wanted) return "Complete";
	return `Loading: ${source.held_from.slice(0, 4)}–${source.held_through.slice(0, 4)} of ${span}`;
}

const KIND_ORDER: Record<SourceKind, number> = {
	Sensor: 0,
	Station: 1,
	Grid: 2,
	Analysis: 3,
	Estimate: 4,
};

/** Failing sources first, then by kind, then by name. */
// @spec SETUP-PAGE-004, SETUP-PAGE-010
export function orderSources(sources: SourceView[]): SourceView[] {
	const failing = (s: SourceView) => (s.last_error ? 0 : 1);
	return [...sources].sort(
		(a, b) =>
			failing(a) - failing(b) ||
			KIND_ORDER[a.kind] - KIND_ORDER[b.kind] ||
			a.name.localeCompare(b.name),
	);
}

function days(n: number): string {
	return n === 1 ? "1 day" : `${n} days`;
}

/** The estimate's accuracy in plain words, one sentence per line. */
// @spec SETUP-PAGE-002
export function accuracyLines(accuracy: EstimateAccuracy): string[] {
	const lines = [
		`Daily values typically within ${accuracy.typical_error_f.toFixed(1)} °F of a measured station.`,
	];
	const dated = accuracy.crossings.filter((c) => c.datable);
	if (dated.length > 0) {
		const medians = dated.map((c) => Math.round(c.median_days));
		const low = Math.min(...medians);
		const high = Math.max(...medians);
		const worst = Math.round(Math.max(...dated.map((c) => c.p80_days)));
		const within = low === high ? days(low) : `${low} to ${days(high)}`;
		lines.push(
			`Crossing dates typically within ${within}; one year in five, off by ${days(worst)} or more.`,
		);
	}
	const undated = accuracy.crossings.filter((c) => !c.datable);
	if (undated.length > 0) {
		const names = undated.map(
			(c) => `${c.threshold_f} °F ${c.direction.toLowerCase()}`,
		);
		lines.push(`Not dated by the estimate: ${names.join(", ")}.`);
	}
	return lines;
}

/** "a", "a or b", "a, b or c". */
function orList(items: string[]): string {
	if (items.length < 2) return items.join("");
	return `${items.slice(0, -1).join(", ")} or ${items[items.length - 1]}`;
}

/** One line naming a group's inputs that nothing measures, and what would; null when none. */
// @spec SETUP-PAGE-003
export function noSourceLine(
	group: InputGroup,
	inputs: InputView[],
): string | null {
	const none = inputs.filter(
		(i) => groupOf(i.input) === group && i.standing === "NoSource",
	);
	if (none.length === 0) return null;
	const depths = orList(none.map((i) => i.input.split("_").pop() ?? ""));
	switch (group) {
		case "soil_moisture":
			return `Nothing measures soil moisture at ${depths} cm here; a probe you map as a sensor would.`;
		case "soil_temp":
			return `Nothing measures soil temperature at ${depths} cm here; a probe you map as a sensor, or a station within 50 miles, would.`;
		case "air":
			return `Nothing measures ${orList(none.map((i) => i.label.toLowerCase()))} here; a sensor you map would.`;
	}
}

/** The basis label's words. */
// @spec WX-VIEW-BASIS-001
export function basisText(basis: Basis, mixed = false): string {
	let text = BASIS_KIND_LABELS[basis.kind];
	if (basis.kind === "Station") {
		const miles =
			basis.distance_miles === null
				? ""
				: `, ${Math.round(basis.distance_miles)} mi`;
		text = `${text} · ${basis.source}${miles}`;
	} else if (basis.kind === "Sensor") {
		text = `${text} · ${basis.source}`;
	}
	if (basis.approximate) text += " · approximate";
	if (mixed) text += " · mixed";
	return text;
}

/** The inputs a source speaks for, in the page's order with soil depths together. */
// @spec SETUP-PAGE-012
export function speaksFor(
	inputs: string[],
	labels: Record<string, string>,
): string {
	const AIR = ["air_temp", "humidity", "rain", "daily_air"];
	const parts = AIR.filter((i) => inputs.includes(i)).map((i) =>
		(labels[i] ?? i).toLowerCase(),
	);
	for (const [prefix, name] of [
		["soil_temp_", "soil temperature"],
		["soil_moisture_", "soil moisture"],
	]) {
		const depths = inputs
			.filter((i) => i.startsWith(prefix))
			.map((i) => Number(i.slice(prefix.length)))
			.sort((a, b) => a - b)
			.map(String);
		if (depths.length) parts.push(`${name} at ${andList(depths)} cm`);
	}
	const text = parts.join(", ");
	return text.charAt(0).toUpperCase() + text.slice(1);
}

/** "a", "a and b", "a, b and c". */
function andList(items: string[]): string {
	if (items.length < 2) return items.join("");
	return `${items.slice(0, -1).join(", ")} and ${items[items.length - 1]}`;
}

/**
 * Beside the forecast provider: when its forecast was fetched, why it is failing, or
 * that it has not been fetched yet.
 */
// @spec SETUP-PAGE-013
export function forecastStatusText(
	status: ForecastStatus,
	now: Date,
	timeZone: string,
): string {
	if (status.last_error) return status.last_error;
	if (status.fetched_at)
		return `Fetched ${observedText(status.fetched_at, now, timeZone)}`;
	return "Not fetched yet";
}

/** Whether the forecast is up: one is held and its latest fetch succeeded. */
// @spec SYNC-CONN-008
export function forecastUp(status: ForecastStatus): boolean {
	return status.fetched_at !== null && status.last_error === null;
}
