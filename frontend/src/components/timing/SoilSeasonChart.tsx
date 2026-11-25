import { useState } from "react";
import {
	CartesianGrid,
	Line,
	LineChart,
	ReferenceLine,
	ResponsiveContainer,
	Tooltip,
	XAxis,
	YAxis,
} from "recharts";
import type { SoilSeriesDay } from "../../types/timing";
import { dayNumber, formatMonthDay } from "../../types/timing";
import SoilSeasonTable, { type SoilRow } from "./SoilSeasonTable";

interface SoilSeasonChartProps {
	series: SoilSeriesDay[];
	today: string;
	/** Dates of the lasting changes of 5 cm soil basis this year. */
	changes?: string[];
}

// Categorical pair validated for CVD separation; the typical year is a neutral
// reference, told apart by its dash as well as its color.
const COLORS = { thisYear: "#2a78d6", lastYear: "#eb6834", typical: "#718096" };

const SPRING_THRESHOLDS = [
	{ at: 45, label: "45° pre-em opens" },
	{ at: 50, label: "50° pre-em ideal" },
	{ at: 55, label: "55° crabgrass" },
	{ at: 65, label: "65° seeding ends" },
];
const FALL_THRESHOLDS = [
	{ at: 75, label: "75° seeding opens" },
	{ at: 70, label: "70° pre-em opens" },
	{ at: 65, label: "65° pre-em late" },
	{ at: 55, label: "55° windows close" },
];

const SEASON_DAYS_BEFORE = 75;
const SEASON_DAYS_AFTER = 60;

type Row = SoilRow & {
	this_measured: number | null;
	this_estimated: number | null;
	last_measured: number | null;
	last_estimated: number | null;
};

/**
 * One year's values as a measured line and an estimated line. An estimated stretch takes
 * the measured day on each side too, so the dotted segment joins the solid one.
 */
function splitByEstimate(
	values: (number | null)[],
	estimated: boolean[],
): { measured: (number | null)[]; estimated: (number | null)[] } {
	return {
		measured: values.map((v, i) => (estimated[i] ? null : v)),
		estimated: values.map((v, i) =>
			estimated[i] || estimated[i - 1] || estimated[i + 1] ? v : null,
		),
	};
}

function SoilTooltip({
	active,
	payload,
}: {
	active?: boolean;
	payload?: { payload: Row }[];
}) {
	const row = payload?.[0]?.payload;
	if (!active || !row) return null;
	const line = (name: string, value: number | null) =>
		value !== null && (
			<div>
				{name}: {value.toFixed(1)}°F
			</div>
		);
	return (
		<div style={styles.tooltip}>
			<div style={styles.tooltipTitle}>{row.label}</div>
			{line(
				row.this_year_estimated ? "This year (estimated)" : "This year",
				row.this_year,
			)}
			{row.this_year === null && line("Forecast", row.forecast)}
			{line(
				row.last_year_estimated ? "Last year (estimated)" : "Last year",
				row.last_year,
			)}
			{line("Typical", row.typical)}
		</div>
	);
}

// @spec TIME-UI-006, UI-CONV-004
/** 5 cm soil temperature (5-day mean): this year vs last year vs the typical year. */
export default function SoilSeasonChart({
	series,
	today,
	changes = [],
}: SoilSeasonChartProps) {
	const [fullYear, setFullYear] = useState(false);
	const todayN = dayNumber(today);
	const isFall = Number(today.split("-")[1]) >= 7;
	const thresholds = isFall ? FALL_THRESHOLDS : SPRING_THRESHOLDS;

	// Bridge the forecast onto the last observed day so the two segments join.
	const lastObserved = series.findLast((d) => d.this_year !== null);
	const shown = series.filter(
		(d) =>
			fullYear ||
			(dayNumber(d.date) >= todayN - SEASON_DAYS_BEFORE &&
				dayNumber(d.date) <= todayN + SEASON_DAYS_AFTER),
	);
	const thisYear = splitByEstimate(
		shown.map((d) => d.this_year),
		shown.map((d) => d.this_year_estimated),
	);
	const lastYear = splitByEstimate(
		shown.map((d) => d.last_year),
		shown.map((d) => d.last_year_estimated),
	);
	const rows: Row[] = shown.map((d, i) => ({
		...d,
		forecast: d === lastObserved ? d.this_year : d.forecast,
		label: formatMonthDay(d.date),
		this_measured: thisYear.measured[i],
		this_estimated: thisYear.estimated[i],
		last_measured: lastYear.measured[i],
		last_estimated: lastYear.estimated[i],
	}));
	const anyEstimated = rows.some(
		(d) => d.this_year_estimated || d.last_year_estimated,
	);
	const changeLabels = rows
		.filter((d) => changes.includes(d.date))
		.map((d) => d.label);
	const monthStarts = rows
		.filter((d) => d.date.endsWith("-01"))
		.map((d) => d.label);
	const todayLabel = rows.find((d) => d.date === today)?.label;

	// Round-number ticks every 10°F spanning the plotted values.
	const values = rows.flatMap((d) =>
		[d.this_year, d.forecast, d.last_year, d.typical].filter(
			(v): v is number => v !== null,
		),
	);
	const yMin = Math.floor((Math.min(...values) - 1) / 10) * 10;
	const yMax = Math.ceil((Math.max(...values) + 1) / 10) * 10;
	const yTicks = Array.from(
		{ length: (yMax - yMin) / 10 + 1 },
		(_, i) => yMin + i * 10,
	);

	if (rows.every((d) => d.this_year === null && d.typical === null)) {
		return (
			<div style={styles.empty}>No soil temperature history to chart.</div>
		);
	}

	return (
		<div>
			<div style={styles.toolbar}>
				<button
					type="button"
					style={styles.toggle}
					onClick={() => setFullYear((v) => !v)}
					aria-pressed={fullYear}
				>
					{fullYear ? "Show this season" : "Show full year"}
				</button>
			</div>
			<ResponsiveContainer width="100%" height={280}>
				<LineChart
					data={rows}
					margin={{ top: 22, right: 112, left: 0, bottom: 0 }}
				>
					<CartesianGrid vertical={false} stroke="#edf2f7" />
					<XAxis
						dataKey="label"
						ticks={monthStarts}
						tick={{ fontSize: 11, fill: "#718096" }}
						tickLine={false}
						axisLine={{ stroke: "#e2e8f0" }}
					/>
					<YAxis
						domain={[yTicks[0], yTicks[yTicks.length - 1]]}
						ticks={yTicks}
						tickFormatter={(v: number) => `${Math.round(v)}°`}
						tick={{ fontSize: 11, fill: "#718096" }}
						tickLine={false}
						axisLine={false}
						width={36}
					/>
					{thresholds.map((t) => (
						<ReferenceLine
							key={t.at}
							y={t.at}
							stroke="#a0aec0"
							strokeDasharray="3 3"
							label={{
								value: t.label,
								position: "right",
								fontSize: 10,
								fill: "#718096",
							}}
						/>
					))}
					{todayLabel && (
						<ReferenceLine
							x={todayLabel}
							stroke="#2d3748"
							label={{
								value: "Today",
								position: "top",
								fontSize: 10,
								fill: "#2d3748",
							}}
						/>
					)}
					{changeLabels.map((label) => (
						<ReferenceLine
							key={`change-${label}`}
							x={label}
							stroke="#a0aec0"
							strokeDasharray="4 3"
							label={{
								value: "Source changed",
								position: "insideTopLeft",
								fontSize: 10,
								fill: "#718096",
							}}
						/>
					))}
					<Tooltip content={<SoilTooltip />} />
					<Line
						dataKey="typical"
						stroke={COLORS.typical}
						strokeWidth={1}
						dot={false}
						isAnimationActive={false}
					/>
					<Line
						dataKey="last_measured"
						stroke={COLORS.lastYear}
						strokeWidth={1.5}
						dot={false}
						isAnimationActive={false}
					/>
					<Line
						dataKey="last_estimated"
						stroke={COLORS.lastYear}
						strokeWidth={1.5}
						strokeDasharray="1 3"
						dot={false}
						isAnimationActive={false}
					/>
					<Line
						dataKey="forecast"
						stroke={COLORS.thisYear}
						strokeWidth={2}
						strokeDasharray="5 4"
						dot={false}
						isAnimationActive={false}
					/>
					<Line
						dataKey="this_measured"
						stroke={COLORS.thisYear}
						strokeWidth={2}
						dot={false}
						isAnimationActive={false}
					/>
					<Line
						dataKey="this_estimated"
						stroke={COLORS.thisYear}
						strokeWidth={2}
						strokeDasharray="1 3"
						dot={false}
						isAnimationActive={false}
					/>
				</LineChart>
			</ResponsiveContainer>

			<div style={styles.legend}>
				<span style={styles.legendItem}>
					<span style={{ ...styles.key, borderTopColor: COLORS.thisYear }} />
					This year
				</span>
				<span style={styles.legendItem}>
					<span
						style={{
							...styles.key,
							borderTopColor: COLORS.thisYear,
							borderTopStyle: "dashed",
						}}
					/>
					Forecast
				</span>
				<span style={styles.legendItem}>
					<span style={{ ...styles.key, borderTopColor: COLORS.lastYear }} />
					Last year
				</span>
				<span style={styles.legendItem}>
					<span
						style={{
							...styles.key,
							borderTopColor: COLORS.typical,
							borderTopWidth: 1,
						}}
					/>
					Typical year
				</span>
				{anyEstimated && (
					<span style={styles.legendItem}>
						<span
							style={{
								...styles.key,
								borderTopColor: "#4a5568",
								borderTopStyle: "dotted",
							}}
						/>
						Dotted = estimated for your location
					</span>
				)}
				{todayLabel && (
					<span style={styles.legendItem}>
						<span style={styles.todayKey} />
						Today
					</span>
				)}
			</div>

			<SoilSeasonTable rows={rows} />
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	empty: { fontSize: "0.85rem", color: "#a0aec0", padding: "1rem 0" },
	toolbar: { display: "flex", justifyContent: "flex-end", marginBottom: 4 },
	todayKey: {
		display: "inline-block",
		height: 10,
		borderLeft: "2px solid #2d3748",
		marginRight: 4,
	},
	toggle: {
		fontSize: "0.75rem",
		color: "#2d3748",
		backgroundColor: "#fff",
		border: "1px solid #cbd5e0",
		borderRadius: 4,
		padding: "3px 10px",
		cursor: "pointer",
	},
	tooltip: {
		backgroundColor: "#fff",
		border: "1px solid #e2e8f0",
		borderRadius: 6,
		padding: "6px 10px",
		fontSize: "0.8rem",
		color: "#2d3748",
		boxShadow: "0 2px 6px rgba(0,0,0,0.1)",
	},
	tooltipTitle: { fontWeight: 600, marginBottom: 2 },
	legend: {
		display: "flex",
		flexWrap: "wrap",
		gap: "0.25rem 1rem",
		fontSize: "0.75rem",
		color: "#4a5568",
		marginTop: 6,
	},
	legendItem: { display: "inline-flex", alignItems: "center" },
	key: {
		display: "inline-block",
		width: 18,
		height: 0,
		borderTopWidth: 2,
		borderTopStyle: "solid",
		marginRight: 6,
	},
};
