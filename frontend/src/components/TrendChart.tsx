import {
	Area,
	AreaChart,
	CartesianGrid,
	ReferenceLine,
	ResponsiveContainer,
	Tooltip,
	XAxis,
	YAxis,
} from "recharts";
import type { SeriesSpan, TimeSeriesPoint } from "../types";
import ChartTable from "./ChartTable";
import {
	basisChanges,
	basisOn,
	localDay,
	spanCaption,
	splitByBasis,
} from "./environmental/trendBasis";
import { basisText } from "./sources/display";

interface TrendChartProps {
	data: TimeSeriesPoint[];
	label: string;
	unit: string;
	color: string;
	thresholdValue?: number;
	thresholdLabel?: string;
	height?: number;
	/** What this series rests on; measured stretches solid, estimated dotted. */
	spans?: SeriesSpan[];
}

export default function TrendChart({
	data,
	label,
	unit,
	color,
	thresholdValue,
	thresholdLabel,
	height = 200,
	spans = [],
}: TrendChartProps) {
	if (data.length === 0) {
		return (
			<div style={styles.container}>
				<div style={styles.label}>{label}</div>
				<div style={styles.empty}>No data available</div>
			</div>
		);
	}

	// @spec WX-VIEW-BASIS-003
	const chartData = splitByBasis(data, spans).map((p, i) => ({
		...p,
		value: data[i].value,
	}));
	const changes = basisChanges(spans).map((day) => {
		const [y, m, d] = day.split("-").map(Number);
		return new Date(y, m - 1, d).getTime();
	});

	const formatTime = (tick: number) => {
		const d = new Date(tick);
		return `${d.getMonth() + 1}/${d.getDate()}`;
	};

	const formatTooltipTime = (tick: number) => {
		return new Date(tick).toLocaleString(undefined, {
			month: "short",
			day: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	};

	return (
		<div style={styles.container}>
			<div style={styles.label}>{label}</div>
			<ResponsiveContainer width="100%" height={height}>
				<AreaChart
					data={chartData}
					margin={{ top: 5, right: 10, left: 0, bottom: 5 }}
				>
					<defs>
						<linearGradient
							id={`grad-${color.replace("#", "")}`}
							x1="0"
							y1="0"
							x2="0"
							y2="1"
						>
							<stop offset="5%" stopColor={color} stopOpacity={0.3} />
							<stop offset="95%" stopColor={color} stopOpacity={0} />
						</linearGradient>
					</defs>
					<CartesianGrid strokeDasharray="3 3" stroke="#edf2f7" />
					<XAxis
						dataKey="time"
						type="number"
						domain={["dataMin", "dataMax"]}
						tickFormatter={formatTime}
						tick={{ fontSize: 11, fill: "#718096" }}
						stroke="#e2e8f0"
					/>
					<YAxis
						tick={{ fontSize: 11, fill: "#718096" }}
						stroke="#e2e8f0"
						width={45}
					/>
					<Tooltip
						labelFormatter={(tick) => formatTooltipTime(Number(tick))}
						formatter={(value) => [
							`${Number(value).toFixed(1)} ${unit}`,
							label,
						]}
						contentStyle={{
							fontSize: "0.8rem",
							borderRadius: 6,
							border: "1px solid #e2e8f0",
						}}
					/>
					{thresholdValue != null && (
						<ReferenceLine
							y={thresholdValue}
							stroke="#ef4444"
							strokeDasharray="5 3"
							label={{
								value: thresholdLabel || "",
								position: "insideTopRight",
								fontSize: 10,
								fill: "#ef4444",
							}}
						/>
					)}
					{changes.map((x) => (
						<ReferenceLine key={x} x={x} stroke="#a0aec0" strokeWidth={1} />
					))}
					<Area
						type="monotone"
						dataKey="measured"
						stroke={color}
						fill={`url(#grad-${color.replace("#", "")})`}
						strokeWidth={2}
						dot={false}
						connectNulls={false}
					/>
					{/* Dotted, not dashed: dashed is reserved for forecasts. */}
					<Area
						type="monotone"
						dataKey="estimated"
						stroke={color}
						strokeDasharray="2 3"
						fill="none"
						strokeWidth={2}
						dot={false}
						connectNulls={false}
					/>
				</AreaChart>
			</ResponsiveContainer>
			{spans.length > 0 && <p style={styles.caption}>{spanCaption(spans)}</p>}
			<ChartTable
				columns={
					spans.length > 0
						? ["Time", `${label} (${unit})`, "Basis"]
						: ["Time", `${label} (${unit})`]
				}
				rows={data.map((p) => {
					const row: (string | number)[] = [
						formatTooltipTime(new Date(p.timestamp).getTime()),
						p.value,
					];
					if (spans.length > 0) {
						const basis = basisOn(spans, localDay(p.timestamp));
						row.push(basis ? basisText(basis) : "—");
					}
					return row;
				})}
			/>
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	container: {
		backgroundColor: "#fff",
		borderRadius: 8,
		padding: "0.75rem 1rem",
		boxShadow: "0 1px 3px rgba(0,0,0,0.08)",
	},
	label: {
		fontSize: "0.8rem",
		fontWeight: 600,
		color: "#4a5568",
		marginBottom: 8,
	},
	caption: { fontSize: "0.75rem", color: "#718096", margin: "0.25rem 0 0" },
	empty: {
		color: "#a0aec0",
		fontSize: "0.8rem",
		padding: "2rem 0",
		textAlign: "center" as const,
	},
};
