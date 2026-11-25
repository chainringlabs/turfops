import { useCallback, useEffect, useRef, useState } from "react";
import {
	getEnvironmental,
	getHistorical,
	getProfile,
	getSoilTempForecast,
	getSources,
	refreshEnvironmental,
} from "../api/client";
import { describeError, needsLocation } from "../api/errors";
import {
	crossingWhen,
	forecastPrecip,
	isPartialDay,
	lastDays,
	soilObservedLine,
	staleSoilMessage,
	TREND_REFERENCES,
} from "../components/environmental/display";
import GaugeCard from "../components/environmental/GaugeCard";
import {
	AMBIENT_TEMP_GAUGE,
	HUMIDITY_GAUGE,
	SOIL_MOISTURE_GAUGE,
	SOIL_TEMP_GAUGE,
} from "../components/gaugeConfigs";
import PredictionChart from "../components/PredictionChart";
import SetupNotice from "../components/SetupNotice";
import BasisLabel from "../components/sources/BasisLabel";
import { currentBasisOf } from "../components/sources/display";
import TrendChart from "../components/TrendChart";
import { sharedStyles } from "../styles/shared";
import type {
	CurrentBasis,
	EnvironmentalSummary,
	HistoricalData,
	SeriesSpan,
	SoilTempForecast,
	WeekBasis,
} from "../types";
import { PREDICTION_CONFIDENCE_COLORS } from "../types";
import type { SourceView } from "../types/sources";
import { FORECAST_PROVIDER_NAMES } from "../types/sources";
import { mmToInches } from "../utils/units";

const POLL_INTERVAL = 30_000;

type HistRange = "7d" | "30d" | "90d";

// @spec WX-VIEW-PAGE-001, WX-VIEW-PAGE-003, WX-VIEW-PAGE-005
export default function Environmental() {
	const [data, setData] = useState<EnvironmentalSummary | null>(null);
	const [histData, setHistData] = useState<HistoricalData | null>(null);
	const [soilForecast, setSoilForecast] = useState<SoilTempForecast | null>(
		null,
	);
	const [histRange, setHistRange] = useState<HistRange>("7d");
	const [showAllDays, setShowAllDays] = useState(false);
	const [sources, setSources] = useState<SourceView[]>([]);
	const [histLoading, setHistLoading] = useState(false);
	const [histError, setHistError] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(true);
	const [refreshing, setRefreshing] = useState(false);
	// @spec SETUP-FLOW-001, UI-ERR-012: the lawn has no location
	const [needsSetup, setNeedsSetup] = useState(false);
	const abortRef = useRef<AbortController | null>(null);

	useEffect(() => {
		getProfile()
			.then((profile) => {
				if (profile.location === null) setNeedsSetup(true);
			})
			.catch(() => {});
	}, []);

	const fetchData = useCallback(async () => {
		abortRef.current?.abort();
		const controller = new AbortController();
		abortRef.current = controller;
		try {
			const [d, sf] = await Promise.all([
				getEnvironmental(),
				getSoilTempForecast().catch(() => null),
			]);
			if (!controller.signal.aborted) {
				setData(d);
				setSoilForecast(sf);
				setError(null);
			}
		} catch (e) {
			if (!controller.signal.aborted) {
				setError(describeError(e));
			}
		} finally {
			if (!controller.signal.aborted) {
				setLoading(false);
			}
		}
	}, []);

	// The sources the page draws on, for their credits; read once.
	useEffect(() => {
		getSources()
			.then((view) => setSources(view.sources))
			.catch(() => setSources([]));
	}, []);

	useEffect(() => {
		fetchData();

		let intervalId: ReturnType<typeof setInterval> | null = null;

		const startPolling = () => {
			stopPolling();
			if (!document.hidden) {
				intervalId = setInterval(fetchData, POLL_INTERVAL);
			}
		};

		const stopPolling = () => {
			if (intervalId !== null) {
				clearInterval(intervalId);
				intervalId = null;
			}
		};

		const handleVisibility = () => {
			if (document.hidden) {
				stopPolling();
			} else {
				fetchData();
				startPolling();
			}
		};

		document.addEventListener("visibilitychange", handleVisibility);
		startPolling();

		return () => {
			stopPolling();
			abortRef.current?.abort();
			document.removeEventListener("visibilitychange", handleVisibility);
		};
	}, [fetchData]);

	// Fetch historical data when range changes
	useEffect(() => {
		let cancelled = false;
		setHistLoading(true);
		setHistError(null);
		getHistorical(histRange)
			.then((d) => {
				if (!cancelled) {
					setHistData(d);
					setHistLoading(false);
				}
			})
			// @spec WX-VIEW-PAGE-006
			// A failed load clears the charts rather than leave the previous range's data
			// under the new selector.
			.catch((e) => {
				if (!cancelled) {
					if (needsLocation(e)) setNeedsSetup(true);
					setHistData(null);
					setHistError(describeError(e));
					setHistLoading(false);
				}
			});
		return () => {
			cancelled = true;
		};
	}, [histRange]);

	// @spec WX-VIEW-PAGE-002
	const handleRefresh = async () => {
		setRefreshing(true);
		try {
			const d = await refreshEnvironmental();
			setData(d);
			setError(null);
		} catch (e) {
			setError(describeError(e));
		} finally {
			setRefreshing(false);
		}
	};

	if (loading) return <div style={sharedStyles.loading}>Loading...</div>;

	const current = data?.current;

	const bases = data?.current_bases;
	const week = data?.week_bases ?? {};
	// Soil depth table: one row per depth whose temperature or moisture has a source.
	const depthRows = (
		[
			[5, current?.soil_temp_5_f, current?.soil_moisture_5],
			[10, current?.soil_temp_10_f, current?.soil_moisture_10],
			[20, current?.soil_temp_20_f, current?.soil_moisture_20],
			[50, current?.soil_temp_50_f, current?.soil_moisture_50],
			[100, current?.soil_temp_100_f, current?.soil_moisture_100],
		] as const
	)
		.map(([cm, temp, moisture]) => ({
			depth: `${cm} cm`,
			temp,
			moisture,
			tempBasis: currentBasisOf(bases, `soil_temp_${cm}`),
			moistureBasis: currentBasisOf(bases, `soil_moisture_${cm}`),
		}))
		// Without bases (an older answer) every depth is shown, as before.
		.filter(
			(row) =>
				!bases ||
				row.tempBasis?.standing !== "NoSource" ||
				row.moistureBasis?.standing !== "NoSource",
		);
	const forecastDays = data?.forecast?.daily_summary ?? [];
	const shownDays = showAllDays ? forecastDays : forecastDays.slice(0, 7);

	if (needsSetup)
		return (
			<div>
				<div style={sharedStyles.headerRow}>
					<h1 style={sharedStyles.pageTitle}>Environmental Data</h1>
				</div>
				<SetupNotice />
			</div>
		);

	return (
		<div>
			<div style={sharedStyles.headerRow}>
				<h1 style={sharedStyles.pageTitle}>Environmental Data</h1>
				<button
					type="button"
					style={styles.refreshBtn}
					onClick={handleRefresh}
					disabled={refreshing}
				>
					{refreshing ? "Refreshing..." : "Refresh Now"}
				</button>
			</div>

			{error && <div style={sharedStyles.error}>{error}</div>}

			{data?.last_updated && (
				<p style={styles.updated}>
					Last updated: {new Date(data.last_updated).toLocaleString()}
					{data.soil_temp_trend &&
						` | Soil temp trend: ${data.soil_temp_trend}`}
					{soilObservedLine(data) && ` | ${soilObservedLine(data)}`}
				</p>
			)}

			{/* @spec WX-VIEW-PAGE-008: data sync owns the stale limit (SYNC-MERGE-006) */}
			{data?.soil_stale && (
				<div role="status" style={sharedStyles.notice}>
					{staleSoilMessage(data)}
				</div>
			)}

			{/* Gauges, each with its basis — @spec WX-VIEW-PAGE-003 */}
			<div style={sharedStyles.gaugeGrid}>
				<GaugeCard
					style={sharedStyles.card}
					config={SOIL_TEMP_GAUGE}
					value={current?.soil_temp_10_f ?? null}
					entry={currentBasisOf(bases, "soil_temp_10")}
				/>
				<GaugeCard
					style={sharedStyles.card}
					config={AMBIENT_TEMP_GAUGE}
					value={current?.ambient_temp_f ?? null}
					entry={currentBasisOf(bases, "air_temp")}
				/>
				<GaugeCard
					style={sharedStyles.card}
					config={HUMIDITY_GAUGE}
					value={current?.humidity_percent ?? null}
					entry={currentBasisOf(bases, "humidity")}
				/>
				<GaugeCard
					style={sharedStyles.card}
					config={SOIL_MOISTURE_GAUGE}
					value={
						current?.soil_moisture_10 != null
							? current.soil_moisture_10 * 100
							: null
					}
					entry={currentBasisOf(bases, "soil_moisture_10")}
				/>
			</div>

			{/* 7-day summary — @spec WX-VIEW-PAGE-004: mm become inches only here */}
			<div style={styles.summaryGrid}>
				<SummaryCard
					label="7-Day Soil Temp Avg"
					basis={week.soil_temp_7day_avg_f}
					value={data?.soil_temp_7day_avg_f}
					unit={"\u00B0F"}
				/>
				<SummaryCard
					label="7-Day Ambient Avg"
					basis={week.ambient_temp_7day_avg_f}
					value={data?.ambient_temp_7day_avg_f}
					unit={"\u00B0F"}
				/>
				<SummaryCard
					label="7-Day Humidity Avg"
					basis={week.humidity_7day_avg}
					value={data?.humidity_7day_avg}
					unit="%"
				/>
				<SummaryCard
					label="7-Day Precipitation"
					basis={week.precipitation_7day_total_mm}
					value={
						data?.precipitation_7day_total_mm != null
							? mmToInches(data.precipitation_7day_total_mm)
							: null
					}
					unit="in"
					decimals={2}
				/>
			</div>

			{/* Soil depth table */}
			<h2 style={sharedStyles.sectionTitle}>Soil Profile by Depth</h2>
			<table style={{ ...sharedStyles.table, marginBottom: "1.5rem" }}>
				<thead>
					<tr>
						<th style={sharedStyles.th}>Depth</th>
						<th style={sharedStyles.th}>Temperature</th>
						<th style={sharedStyles.th}>Moisture</th>
					</tr>
				</thead>
				<tbody>
					{depthRows.map((row) => (
						<tr key={row.depth}>
							<td style={sharedStyles.td}>{row.depth}</td>
							<td style={sharedStyles.td}>
								<DepthValue
									text={
										row.temp != null ? `${row.temp.toFixed(1)} \u00B0F` : "--"
									}
									entry={row.tempBasis}
								/>
							</td>
							<td style={sharedStyles.td}>
								<DepthValue
									text={
										row.moisture != null
											? `${(row.moisture * 100).toFixed(1)}%`
											: "--"
									}
									entry={row.moistureBasis}
								/>
							</td>
						</tr>
					))}
				</tbody>
			</table>

			{/* Forecast */}
			{/* @spec WX-VIEW-PAGE-003: headed by its provider; 7 days until asked for more */}
			{data?.forecast && forecastDays.length > 0 && (
				<>
					<h2 style={sharedStyles.sectionTitle}>
						Forecast from {FORECAST_PROVIDER_NAMES[data.forecast.provider]}
					</h2>
					<div style={styles.forecastGrid}>
						{shownDays.map((day) => (
							<div key={day.date} style={styles.forecastCard}>
								<div style={styles.forecastDate}>
									{day.date}
									{isPartialDay(day) && (
										<span style={styles.partialDay}> · partial day</span>
									)}
								</div>
								<div style={styles.forecastCondition}>
									{day.dominant_condition}
								</div>
								<div style={styles.forecastTemp}>
									{day.high_temp_f.toFixed(0)}
									{"\u00B0"} / {day.low_temp_f.toFixed(0)}
									{"\u00B0"}
								</div>
								<div style={styles.forecastDetail}>
									Humidity: {day.avg_humidity.toFixed(0)}%
								</div>
								<div style={styles.forecastDetail}>{forecastPrecip(day)}</div>
							</div>
						))}
					</div>
					{forecastDays.length > shownDays.length && (
						<button
							type="button"
							style={styles.moreDays}
							onClick={() => setShowAllDays(true)}
						>
							Show {forecastDays.length - shownDays.length} more days
						</button>
					)}
				</>
			)}

			{/* Soil Temperature Forecast — @spec WX-OUTLOOK-UI-001, WX-OUTLOOK-UI-003 */}
			{soilForecast && (
				<>
					<h2 style={sharedStyles.sectionTitle}>Soil Temperature Forecast</h2>
					{soilForecast.basis && (
						<p style={styles.startsFrom}>
							Starts from: <BasisLabel basis={soilForecast.basis} />
						</p>
					)}
					<div style={{ marginBottom: "1rem" }}>
						<PredictionChart
							recentSoilTemps={lastDays(histData?.soil_temp_10_f ?? [], 7)}
							predictions={soilForecast.predictions}
						/>
					</div>
					{soilForecast.threshold_crossings.length > 0 && (
						<div style={styles.crossingGrid}>
							{soilForecast.threshold_crossings.map((c) => (
								<div
									key={`${c.threshold_name}-${c.direction}-${c.threshold_temp_f}`}
									style={styles.crossingCard}
								>
									<div style={styles.crossingHeader}>
										<span style={styles.crossingName}>{c.threshold_name}</span>
										<span
											style={{
												...styles.crossingBadge,
												backgroundColor: `${PREDICTION_CONFIDENCE_COLORS[c.confidence]}22`,
												color: PREDICTION_CONFIDENCE_COLORS[c.confidence],
												borderColor: PREDICTION_CONFIDENCE_COLORS[c.confidence],
											}}
										>
											{c.confidence}
										</span>
									</div>
									<div style={styles.crossingTemp}>
										{c.direction === "Rising" ? "\u2191" : "\u2193"}{" "}
										{c.threshold_temp_f}
										{"\u00B0F"}
									</div>
									<div style={styles.crossingDate}>{crossingWhen(c)}</div>
								</div>
							))}
						</div>
					)}
					{/* @spec WX-OUTLOOK-UI-001: the fit, or the estimate it rests on */}
					{soilForecast.model_info ? (
						<div style={styles.modelInfo}>
							Model: R²={soilForecast.model_info.r_squared.toFixed(2)}, lag=
							{soilForecast.model_info.lag_days}d, quality=
							{soilForecast.model_info.quality}
						</div>
					) : (
						<div style={styles.modelInfo}>
							Estimated for your location
							{soilForecast.estimate_info
								? ` (soil estimate, typically within ${soilForecast.estimate_info.daily_error_f.toFixed(1)} °F a day)`
								: ""}
						</div>
					)}
				</>
			)}

			{/* Historical Trends — @spec WX-VIEW-PAGE-005, WX-VIEW-PAGE-007: reference lines from TREND_REFERENCES only */}
			<div style={styles.trendHeader}>
				<h2 style={sharedStyles.sectionTitle}>Historical Trends</h2>
				<div style={styles.rangeButtons}>
					{(["7d", "30d", "90d"] as HistRange[]).map((r) => (
						<button
							type="button"
							key={r}
							style={{
								...styles.rangeBtn,
								...(histRange === r ? styles.rangeBtnActive : {}),
							}}
							onClick={() => setHistRange(r)}
						>
							{r}
						</button>
					))}
				</div>
			</div>
			{histLoading ? (
				<p style={sharedStyles.loading}>Loading trends...</p>
			) : histError ? (
				<div style={sharedStyles.error}>{histError}</div>
			) : histData ? (
				<div style={styles.chartGrid}>
					<TrendChart
						data={histData.soil_temp_10_f}
						spans={spansOf(histData, "soil_temp_10_f")}
						label="Soil Temperature (10cm)"
						unit={"\u00B0F"}
						color="#e67e22"
					/>
					<TrendChart
						data={histData.ambient_temp_f}
						spans={spansOf(histData, "ambient_temp_f")}
						label="Ambient Temperature"
						unit={"\u00B0F"}
						color="#3498db"
						thresholdValue={TREND_REFERENCES.ambient?.value}
						thresholdLabel={TREND_REFERENCES.ambient?.label}
					/>
					<TrendChart
						data={histData.humidity_percent}
						spans={spansOf(histData, "humidity_percent")}
						label="Humidity"
						unit="%"
						color="#9b59b6"
						thresholdValue={TREND_REFERENCES.humidity?.value}
						thresholdLabel={TREND_REFERENCES.humidity?.label}
					/>
					<TrendChart
						data={histData.soil_moisture_10}
						spans={spansOf(histData, "soil_moisture_10")}
						label="Soil Moisture (10cm)"
						unit=""
						color="#27ae60"
						thresholdValue={TREND_REFERENCES.moisture?.value}
						thresholdLabel={TREND_REFERENCES.moisture?.label}
					/>
					<TrendChart
						data={histData.precipitation_mm.map((p) => ({
							timestamp: p.timestamp,
							value: mmToInches(p.value),
						}))}
						label="Precipitation"
						unit="in"
						spans={spansOf(histData, "precipitation_mm")}
						color="#2c3e50"
					/>
					<TrendChart
						data={histData.gdd_accumulation}
						spans={spansOf(histData, "gdd_accumulation")}
						label="GDD Accumulation (Base 50°F)"
						unit="GDD"
						color="#48bb78"
					/>
				</div>
			) : null}

			{/* @spec WX-VIEW-PAGE-012 */}
			<SourceCredits
				sources={sources}
				used={namesDrawnOn(data, histData, soilForecast)}
			/>
		</div>
	);
}

/** One chart series' basis spans. */
function spansOf(data: HistoricalData, series: string): SeriesSpan[] {
	return (data.basis ?? []).filter((s) => s.series === series);
}

/** The names of the sources every value on the page rests on. */
function namesDrawnOn(
	data: EnvironmentalSummary | null,
	hist: HistoricalData | null,
	outlook: SoilTempForecast | null,
): Set<string> {
	const names = new Set<string>();
	for (const entry of data?.current_bases ?? []) {
		if (entry.basis) names.add(entry.basis.source);
	}
	for (const week of Object.values(data?.week_bases ?? {})) {
		names.add(week.basis.source);
	}
	for (const span of hist?.basis ?? []) names.add(span.basis.source);
	if (outlook?.basis) names.add(outlook.basis.source);
	return names;
}

/** Every source the page drew on, with the credit its terms require. */
// @spec WX-VIEW-PAGE-012
function SourceCredits({
	sources,
	used,
}: {
	sources: SourceView[];
	used: Set<string>;
}) {
	const drawn = sources.filter((s) => used.has(s.name));
	if (drawn.length === 0) return null;
	return (
		<section aria-label="Sources" style={styles.credits}>
			<h2 style={styles.creditsTitle}>Sources</h2>
			<ul style={styles.creditList}>
				{drawn.map((s) => (
					<li key={s.id}>
						{s.name} — {s.attribution}
					</li>
				))}
			</ul>
		</section>
	);
}

// @spec WX-VIEW-PAGE-003, WX-VIEW-BASIS-001
function SummaryCard({
	label,
	value,
	unit,
	decimals = 1,
	basis,
}: {
	label: string;
	value: number | null | undefined;
	unit: string;
	decimals?: number;
	basis?: WeekBasis;
}) {
	return (
		<div style={styles.summaryCard}>
			<div style={styles.summaryLabel}>{label}</div>
			<div style={styles.summaryValue}>
				{value != null ? `${value.toFixed(decimals)} ${unit}` : "--"}
			</div>
			{value != null && basis && (
				<BasisLabel basis={basis.basis} mixed={basis.mixed} />
			)}
		</div>
	);
}

/** A soil value with its basis, or "No source for this location". */
// @spec WX-VIEW-BASIS-001, WX-VIEW-BASIS-004
function DepthValue({
	text,
	entry,
}: {
	text: string;
	entry: CurrentBasis | undefined;
}) {
	if (entry?.standing === "NoSource") {
		return <span style={styles.noSource}>No source for this location</span>;
	}
	return (
		<>
			<div>{text}</div>
			{entry?.standing === "InUse" && entry.basis && (
				<BasisLabel basis={entry.basis} />
			)}
		</>
	);
}

const styles: Record<string, React.CSSProperties> = {
	moreDays: {
		background: "none",
		border: "1px solid #3182ce",
		borderRadius: 4,
		color: "#3182ce",
		padding: "0.25rem 0.75rem",
		fontSize: "0.8rem",
		cursor: "pointer",
		marginBottom: "1.5rem",
	},
	noSource: { fontSize: "0.8rem", color: "#718096" },
	startsFrom: { fontSize: "0.8rem", color: "#4a5568", margin: "0 0 0.5rem" },
	credits: { marginTop: "2rem", fontSize: "0.75rem", color: "#718096" },
	creditsTitle: { fontSize: "0.8rem", fontWeight: 600, margin: "0 0 0.25rem" },
	creditList: { margin: 0, paddingLeft: "1.1rem" },
	refreshBtn: {
		padding: "0.5rem 1rem",
		backgroundColor: "#3182ce",
		color: "#fff",
		border: "none",
		borderRadius: 6,
		cursor: "pointer",
		fontWeight: 600,
		fontSize: "0.85rem",
	},
	updated: { color: "#718096", fontSize: "0.8rem", marginBottom: "1rem" },
	summaryGrid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
		gap: "1rem",
		marginBottom: "1.5rem",
	},
	summaryCard: {
		backgroundColor: "#fff",
		borderRadius: 8,
		padding: "0.8rem 1rem",
		boxShadow: "0 1px 3px rgba(0,0,0,0.08)",
		textAlign: "center" as const,
	},
	summaryLabel: { fontSize: "0.75rem", color: "#718096", marginBottom: 4 },
	summaryValue: { fontSize: "1.2rem", fontWeight: 600, color: "#2d3748" },
	forecastGrid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fit, minmax(150px, 1fr))",
		gap: "0.75rem",
		marginBottom: "1.5rem",
	},
	forecastCard: {
		backgroundColor: "#fff",
		borderRadius: 8,
		padding: "0.8rem",
		boxShadow: "0 1px 3px rgba(0,0,0,0.08)",
		textAlign: "center" as const,
	},
	forecastDate: { fontSize: "0.8rem", fontWeight: 600, color: "#2d3748" },
	partialDay: { fontWeight: 400, color: "#975a16" },
	forecastCondition: { fontSize: "0.85rem", color: "#4a5568", margin: "4px 0" },
	forecastTemp: { fontSize: "1.1rem", fontWeight: 600, color: "#1a202c" },
	forecastDetail: { fontSize: "0.7rem", color: "#718096", marginTop: 2 },
	trendHeader: {
		display: "flex",
		justifyContent: "space-between",
		alignItems: "center",
		marginBottom: "0.75rem",
	},
	rangeButtons: {
		display: "flex",
		gap: 4,
	},
	rangeBtn: {
		padding: "4px 12px",
		borderRadius: 6,
		border: "1px solid #e2e8f0",
		backgroundColor: "#fff",
		color: "#4a5568",
		fontSize: "0.75rem",
		fontWeight: 600,
		cursor: "pointer",
	},
	rangeBtnActive: {
		backgroundColor: "#3182ce",
		color: "#fff",
		borderColor: "#3182ce",
	},
	chartGrid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fit, minmax(380px, 1fr))",
		gap: "1rem",
		marginBottom: "1.5rem",
	},
	crossingGrid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))",
		gap: "0.75rem",
		marginBottom: "0.75rem",
	},
	crossingCard: {
		backgroundColor: "#fff",
		borderRadius: 8,
		padding: "0.8rem",
		boxShadow: "0 1px 3px rgba(0,0,0,0.08)",
	},
	crossingHeader: {
		display: "flex",
		justifyContent: "space-between",
		alignItems: "center",
		marginBottom: 4,
	},
	crossingName: {
		fontSize: "0.75rem",
		fontWeight: 600,
		color: "#4a5568",
	},
	crossingBadge: {
		display: "inline-block",
		padding: "1px 6px",
		borderRadius: 8,
		fontSize: "0.6rem",
		fontWeight: 600,
		border: "1px solid",
	},
	crossingTemp: {
		fontSize: "1.1rem",
		fontWeight: 700,
		color: "#1a202c",
	},
	crossingDate: {
		fontSize: "0.7rem",
		color: "#718096",
		marginTop: 2,
	},
	modelInfo: {
		fontSize: "0.65rem",
		color: "#a0aec0",
		textAlign: "center" as const,
		marginBottom: "1.5rem",
	},
};
