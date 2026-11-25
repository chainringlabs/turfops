// What a trend chart says about the basis of its series: which points are measured and
// which estimated, where the basis changes, the spans in words, and each row's basis.
import type { SeriesSpan, TimeSeriesPoint } from "../../types";
import type { Basis } from "../../types/sources";
import { basisText } from "../sources/display";

/** A point's local date, `YYYY-MM-DD`. */
export function localDay(timestamp: string): string {
	return new Date(timestamp).toLocaleDateString("en-CA");
}

/** The basis a day rests on, when a span covers it. */
export function basisOn(spans: SeriesSpan[], day: string): Basis | undefined {
	return spans.find((s) => s.from <= day && day <= s.to)?.basis;
}

/** A point drawn as measured (solid) or estimated (dotted). */
export interface SplitPoint {
	time: number;
	measured: number | null;
	estimated: number | null;
}

/**
 * Each point on the measured line or the estimated one, by its day's basis. A point
 * where the basis changes is on both, so the two lines meet.
 */
// @spec WX-VIEW-BASIS-003
export function splitByBasis(
	points: TimeSeriesPoint[],
	spans: SeriesSpan[],
): SplitPoint[] {
	const estimated = points.map(
		(p) => basisOn(spans, localDay(p.timestamp))?.kind === "Estimate",
	);
	return points.map((p, i) => {
		const here = estimated[i];
		const changes =
			(i > 0 && estimated[i - 1] !== here) ||
			(i + 1 < points.length && estimated[i + 1] !== here);
		return {
			time: new Date(p.timestamp).getTime(),
			measured: !here || changes ? p.value : null,
			estimated: here || changes ? p.value : null,
		};
	});
}

/** The start of each span after the first whose basis differs from the one before. */
// @spec WX-VIEW-BASIS-003
export function basisChanges(spans: SeriesSpan[]): string[] {
	const sorted = [...spans].sort((a, b) => a.from.localeCompare(b.from));
	return sorted
		.slice(1)
		.filter((span, i) => basisText(span.basis) !== basisText(sorted[i].basis))
		.map((span) => span.from);
}

function shortDate(day: string): string {
	const [y, m, d] = day.split("-").map(Number);
	return new Date(y, m - 1, d).toLocaleDateString("en-US", {
		month: "short",
		day: "numeric",
	});
}

/** The spans in words, oldest first: "Measured · X, 2 mi: Nov 1 – Nov 20; …". */
// @spec WX-VIEW-BASIS-003
export function spanCaption(spans: SeriesSpan[]): string {
	return [...spans]
		.sort((a, b) => a.from.localeCompare(b.from))
		.map((s) => {
			const when =
				s.from === s.to
					? shortDate(s.from)
					: `${shortDate(s.from)} – ${shortDate(s.to)}`;
			return `${basisText(s.basis)}: ${when}`;
		})
		.join("; ");
}
