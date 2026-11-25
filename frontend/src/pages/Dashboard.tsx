import { useCallback, useEffect, useRef, useState } from "react";
import {
	getDashboard,
	getDiseaseRisk,
	getGdd,
	getNitrogenBudget,
	getSoilTempForecast,
	getTimingWindows,
} from "../api/client";
import { describeError } from "../api/errors";
import AlertCard from "../components/AlertCard";
import {
	loadDashboard,
	type Widget,
	type Widgets,
} from "../components/dashboard/loadDashboard";
import DiseaseRiskWidget from "../components/disease/DiseaseRiskWidget";
import { staleSoilMessage } from "../components/environmental/display";
import GaugeCard from "../components/environmental/GaugeCard";
import GddWidget from "../components/GddWidget";
import {
	AMBIENT_TEMP_GAUGE,
	HUMIDITY_GAUGE,
	SOIL_MOISTURE_GAUGE,
	SOIL_TEMP_GAUGE,
} from "../components/gaugeConfigs";
import ShoppingListPanel from "../components/inventory/ShoppingListPanel";
import NitrogenBudgetWidget from "../components/NitrogenBudgetWidget";
import SetupNotice from "../components/SetupNotice";
import SoilTempForecastWidget from "../components/SoilTempForecastWidget";
import { currentBasisOf } from "../components/sources/display";
import SourceStatusLine from "../components/sources/SourceStatusLine";
import TimingWidget from "../components/timing/TimingWidget";
import { appTypeBadgeStyle, sharedStyles } from "../styles/shared";
import type {
	DashboardResponse,
	GddSummary,
	NitrogenBudget,
	SoilTempForecast,
} from "../types";
import { APPLICATION_TYPE_LABELS, GRASS_TYPE_LABELS } from "../types";
import type { DiseaseRiskResponse } from "../types/disease";
import type { TimingResponse } from "../types/timing";
import { formatInches } from "../utils/units";

const POLL_INTERVAL = 30_000; // 30 seconds

type DashboardWidgets = {
	gdd: GddSummary;
	nBudget: NitrogenBudget;
	soilForecast: SoilTempForecast;
	diseaseRisk: DiseaseRiskResponse;
	timing: TimingResponse;
};

/** A widget's last good data; marked when this poll couldn't refresh it. */
function Stale<T>({
	widget,
	children,
}: {
	widget: Widget<T>;
	children: (data: T) => React.ReactNode;
}) {
	if (widget.value == null) return null;
	return (
		<div>
			{widget.stale && (
				<div style={{ fontSize: "0.72rem", color: "#975a16", marginBottom: 4 }}>
					⚠ Couldn't refresh — showing the last good data
				</div>
			)}
			{children(widget.value)}
		</div>
	);
}

// @spec FEED-UI-008, FEED-UI-009, FEED-UI-011, UI-DASH-001, UI-DASH-002
export default function Dashboard() {
	const [data, setData] = useState<DashboardResponse | null>(null);
	const [widgets, setWidgets] = useState<Widgets<DashboardWidgets> | null>(
		null,
	);
	const widgetsRef = useRef<Widgets<DashboardWidgets> | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [loading, setLoading] = useState(true);
	const abortRef = useRef<AbortController | null>(null);

	const fetchData = useCallback(async () => {
		abortRef.current?.abort();
		const controller = new AbortController();
		abortRef.current = controller;
		try {
			// @spec UI-DASH-003, FEED-UI-013
			const result = await loadDashboard<DashboardResponse, DashboardWidgets>(
				{
					dashboard: getDashboard,
					widgets: {
						gdd: (signal) => getGdd(undefined, signal),
						nBudget: (signal) => getNitrogenBudget(undefined, signal),
						soilForecast: getSoilTempForecast,
						diseaseRisk: getDiseaseRisk,
						timing: getTimingWindows,
					},
				},
				controller.signal,
				widgetsRef.current,
			);
			if (!controller.signal.aborted) {
				setData(result.dashboard);
				widgetsRef.current = result.widgets;
				setWidgets(result.widgets);
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

	// @spec FEED-UI-010
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

		// One named handler, so the cleanup below really removes it. (An anonymous second
		// listener used to survive unmount and restart polling from every other page.)
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

	if (loading)
		return (
			<div role="status" style={sharedStyles.loading}>
				Loading dashboard...
			</div>
		);
	if (error && !data)
		return (
			<div role="alert" style={sharedStyles.error}>
				Error: {error}
			</div>
		);
	if (!data) return null;

	const {
		profile,
		environmental,
		recommendations,
		recent_applications,
		sources,
		forecast_status,
	} = data;
	const current = environmental.current;
	const bases = environmental.current_bases;
	// @spec FEED-UI-015
	const placed = profile.location !== null;

	return (
		<div>
			{error && (
				<div role="alert" style={styles.errorBanner}>
					Data may be stale: {error}
				</div>
			)}
			<div style={styles.headerRow}>
				<h1 style={sharedStyles.pageTitle}>{profile.name}</h1>
				<div style={styles.meta}>
					{GRASS_TYPE_LABELS[profile.grass_type] ?? profile.grass_type} &middot;
					Zone {profile.usda_zone}
					{environmental.last_updated && (
						<span style={styles.updated}>
							{" "}
							&middot; Updated{" "}
							{new Date(environmental.last_updated).toLocaleTimeString()}
						</span>
					)}
				</div>
			</div>

			{!placed && <SetupNotice />}

			{/* @spec SYNC-MERGE-006, FEED-UI-009: data sync owns the 72 h limit */}
			{placed && environmental.soil_stale && (
				<div role="status" style={sharedStyles.notice}>
					{staleSoilMessage(environmental)}
				</div>
			)}

			{/* @spec FEED-UI-009, FEED-UI-014 */}
			{placed && (
				<SourceStatusLine sources={sources} forecast={forecast_status} />
			)}

			{/* Gauges, each with its basis — @spec FEED-UI-009 */}
			{placed && (
				<div style={sharedStyles.gaugeGrid}>
					<GaugeCard
						style={sharedStyles.card}
						config={SOIL_TEMP_GAUGE}
						value={current?.soil_temp_10_f ?? null}
						entry={currentBasisOf(bases, "soil_temp_10")}
					>
						{environmental.soil_temp_7day_avg_f !== null && (
							<div style={styles.subtext}>
								7-day avg: {environmental.soil_temp_7day_avg_f.toFixed(1)}°F{" "}
								{trendArrow(environmental.soil_temp_trend)}
							</div>
						)}
					</GaugeCard>
					<GaugeCard
						style={sharedStyles.card}
						config={AMBIENT_TEMP_GAUGE}
						value={current?.ambient_temp_f ?? null}
						entry={currentBasisOf(bases, "air_temp")}
					>
						{environmental.ambient_temp_7day_avg_f !== null && (
							<div style={styles.subtext}>
								7-day avg: {environmental.ambient_temp_7day_avg_f.toFixed(1)}°F
							</div>
						)}
					</GaugeCard>
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
							current?.soil_moisture_10 !== null &&
							current?.soil_moisture_10 !== undefined
								? current.soil_moisture_10 * 100
								: null
						}
						entry={currentBasisOf(bases, "soil_moisture_10")}
					>
						{environmental.precipitation_7day_total_mm !== null && (
							<div style={styles.subtext}>
								7-day precip:{" "}
								{formatInches(environmental.precipitation_7day_total_mm)}
							</div>
						)}
					</GaugeCard>
				</div>
			)}

			{/* GDD, Disease Risk, Timing, Nitrogen Budget & Soil Temp Forecast widgets */}
			{widgets && (
				<div style={styles.widgetGrid}>
					{placed && (
						<Stale widget={widgets.gdd}>{(d) => <GddWidget data={d} />}</Stale>
					)}
					{placed && (
						<Stale widget={widgets.diseaseRisk}>
							{(d) => <DiseaseRiskWidget data={d} />}
						</Stale>
					)}
					{placed && (
						<Stale widget={widgets.timing}>
							{(d) => <TimingWidget data={d} />}
						</Stale>
					)}
					<ShoppingListPanel compact />
					<Stale widget={widgets.nBudget}>
						{(d) => <NitrogenBudgetWidget data={d} />}
					</Stale>
					{/* @spec WX-OUTLOOK-UI-003: a failed outlook leaves no widget */}
					{placed && (
						<Stale widget={widgets.soilForecast}>
							{(d) => (
								<SoilTempForecastWidget
									crossings={d.threshold_crossings}
									currentSoilTemp={current?.soil_temp_10_f ?? null}
								/>
							)}
						</Stale>
					)}
				</div>
			)}

			{/* Alerts & Recent Apps side by side */}
			<div style={styles.twoCol}>
				<div style={{ flex: 1 }}>
					<h2 style={sharedStyles.sectionTitle}>Active Alerts</h2>
					{recommendations.length === 0 ? (
						<div style={sharedStyles.empty}>No active recommendations</div>
					) : (
						recommendations.map((r) => <AlertCard key={r.id} rec={r} />)
					)}
				</div>
				<div style={{ flex: 1 }}>
					<h2 style={sharedStyles.sectionTitle}>Recent Applications</h2>
					{recent_applications.length === 0 ? (
						<div style={sharedStyles.empty}>No applications recorded</div>
					) : (
						<table style={sharedStyles.table}>
							<thead>
								<tr>
									<th style={sharedStyles.th}>Date</th>
									<th style={sharedStyles.th}>Type</th>
									<th style={sharedStyles.th}>Product</th>
								</tr>
							</thead>
							<tbody>
								{recent_applications.map((app, index) => (
									<tr key={app.id ?? `app-${index}`}>
										<td style={sharedStyles.td}>{app.application_date}</td>
										<td style={sharedStyles.td}>
											<span
												style={appTypeBadgeStyle(
													sharedStyles.badge,
													app.application_type,
												)}
											>
												{APPLICATION_TYPE_LABELS[app.application_type]}
											</span>
										</td>
										<td style={sharedStyles.td}>{app.product_name || "-"}</td>
									</tr>
								))}
							</tbody>
						</table>
					)}
				</div>
			</div>
		</div>
	);
}

function trendArrow(trend: string): string {
	switch (trend) {
		case "Rising":
			return "\u2191";
		case "Falling":
			return "\u2193";
		case "Stable":
			return "\u2192";
		default:
			return "";
	}
}

const styles: Record<string, React.CSSProperties> = {
	errorBanner: {
		padding: "0.5rem 1rem",
		backgroundColor: "#fed7d7",
		color: "#c53030",
		borderRadius: 6,
		marginBottom: "1rem",
		fontSize: "0.85rem",
	},
	headerRow: { marginBottom: "1rem" },
	meta: { color: "#718096", fontSize: "0.85rem", marginTop: 4 },
	updated: { color: "#a0aec0" },
	subtext: { fontSize: "0.75rem", color: "#718096", marginTop: 4 },
	widgetGrid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))",
		gap: "1rem",
		marginBottom: "1.5rem",
	},
	twoCol: { display: "flex", gap: "1.5rem", flexWrap: "wrap" as const },
};
