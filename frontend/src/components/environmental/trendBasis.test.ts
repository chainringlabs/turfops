import { describe, expect, it } from "vitest";
import type { SeriesSpan, TimeSeriesPoint } from "../../types";
import type { Basis } from "../../types/sources";
import { basisChanges, basisOn, spanCaption, splitByBasis } from "./trendBasis";

const STATION: Basis = {
	kind: "Station",
	source: "PA Avondale 2 N",
	distance_miles: 2.1,
	approximate: false,
};
const ESTIMATE: Basis = {
	kind: "Estimate",
	source: "Soil temperature estimate",
	distance_miles: null,
	approximate: false,
};

const spans: SeriesSpan[] = [
	{
		series: "soil_temp_10_f",
		from: "2026-11-01",
		to: "2026-11-03",
		basis: STATION,
	},
	{
		series: "soil_temp_10_f",
		from: "2026-11-04",
		to: "2026-11-05",
		basis: ESTIMATE,
	},
];

/** Noon UTC each day, so the local date is the same in any US zone. */
function points(days: string[]): TimeSeriesPoint[] {
	return days.map((d, i) => ({ timestamp: `${d}T12:00:00Z`, value: 50 + i }));
}

describe("a trend chart's basis", () => {
	// @spec WX-VIEW-BASIS-003
	it("puts measured days on the solid line and estimated days on the dotted one, meeting at the change", () => {
		const split = splitByBasis(
			points(["2026-11-02", "2026-11-03", "2026-11-04", "2026-11-05"]),
			spans,
		);
		expect(split.map((p) => [p.measured, p.estimated])).toEqual([
			[50, null],
			[51, 51],
			[52, 52],
			[null, 53],
		]);
	});

	// @spec WX-VIEW-BASIS-003
	it("marks where the basis changes and lists the spans in words", () => {
		expect(basisChanges(spans)).toEqual(["2026-11-04"]);
		expect(spanCaption(spans)).toBe(
			"Measured · PA Avondale 2 N, 2 mi: Nov 1 – Nov 3; Estimated for your location: Nov 4 – Nov 5",
		);
		expect(basisOn(spans, "2026-11-05")).toEqual(ESTIMATE);
		expect(basisOn(spans, "2026-11-09")).toBeUndefined();
	});

	// @spec WX-VIEW-BASIS-003
	it("does not mark a change between spans on the same basis", () => {
		const same: SeriesSpan[] = [
			{ ...spans[0] },
			{ ...spans[0], from: "2026-11-04", to: "2026-11-05" },
		];
		expect(basisChanges(same)).toEqual([]);
	});
});
