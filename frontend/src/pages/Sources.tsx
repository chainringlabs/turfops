// The Sources page: what each weather input rests on, how each source is doing, the
// connections and the lawn's location.
import { useEffect, useState } from "react";
import { Link, useLocation } from "react-router";
import { getSources, testHaConnection, unmapSensor } from "../api/client";
import { describeError } from "../api/errors";
import { testedText } from "../components/setup/sensors";
import {
	forecastStatusText,
	groupOf,
	INPUT_GROUP_TITLES,
	INPUT_GROUPS,
	noSourceLine,
} from "../components/sources/display";
import InputCard from "../components/sources/InputCard";
import SourcesTable from "../components/sources/SourcesTable";
import { sharedStyles } from "../styles/shared";
import type { SourcesView } from "../types/sources";
import { FORECAST_PROVIDER_NAMES } from "../types/sources";

// @spec SETUP-PAGE-001, SETUP-PAGE-011
export default function Sources() {
	const [data, setData] = useState<SourcesView | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [problem, setProblem] = useState<string | null>(null);
	const [version, setVersion] = useState(0);
	const saved = (useLocation().state as { saved?: boolean } | null)?.saved;

	// Loaded once, and again after a change made here; the owner reloads to see newer
	// readings.
	// biome-ignore lint/correctness/useExhaustiveDependencies: `version` is the signal to read again.
	useEffect(() => {
		const controller = new AbortController();
		getSources(controller.signal)
			.then(setData)
			.catch((e: unknown) => {
				if (!controller.signal.aborted) setError(describeError(e));
			});
		return () => controller.abort();
	}, [version]);

	// Unmapping keeps the sensor's history, so it asks nothing first.
	// @spec SETUP-FAIL-003
	async function unmap(sensorId: number) {
		setProblem(null);
		try {
			await unmapSensor(sensorId);
			setVersion((v) => v + 1);
		} catch (e) {
			setProblem(describeError(e));
		}
	}

	if (error) return <div style={sharedStyles.error}>{error}</div>;
	if (!data) return <div style={sharedStyles.loading}>Loading sources…</div>;

	const now = new Date();
	const { location } = data;
	const labels = Object.fromEntries(data.inputs.map((i) => [i.input, i.label]));

	return (
		<div style={styles.page}>
			<div style={sharedStyles.headerRow}>
				<h1 style={sharedStyles.pageTitle}>Sources</h1>
			</div>

			{/* @spec SETUP-PLAN-006 */}
			{saved && (
				<div style={sharedStyles.notice}>
					Saved. The new sources start downloading now; history can take several
					minutes.
				</div>
			)}

			{location ? (
				INPUT_GROUPS.map((group) => {
					const shown = data.inputs.filter(
						(i) => groupOf(i.input) === group && i.standing !== "NoSource",
					);
					const nothing = noSourceLine(group, data.inputs);
					const heading = `group-${group}`;
					return (
						<section key={group} aria-labelledby={heading}>
							<h2 id={heading} style={sharedStyles.sectionTitle}>
								{INPUT_GROUP_TITLES[group]}
							</h2>
							<div style={styles.grid}>
								{shown.map((i) => (
									<InputCard
										key={i.input}
										input={i}
										now={now}
										timeZone={location.timezone}
									/>
								))}
							</div>
							{nothing && <p style={styles.nothing}>{nothing}</p>}
						</section>
					);
				})
			) : (
				<div style={sharedStyles.notice}>
					The lawn has no location yet, so no weather source is in use.{" "}
					<Link to="/setup?step=location">Set the location</Link>
				</div>
			)}

			{data.sources.length > 0 && location && (
				<section aria-labelledby="sources-heading">
					<h2 id="sources-heading" style={sharedStyles.sectionTitle}>
						By source
					</h2>
					{problem && <div style={sharedStyles.error}>{problem}</div>}
					<SourcesTable
						sources={data.sources}
						labels={labels}
						now={now}
						timeZone={location.timezone}
						onUnmap={unmap}
					/>
				</section>
			)}

			<Connections data={data} now={now} />

			{location && (
				<section aria-labelledby="location-heading" style={sharedStyles.card}>
					<div style={styles.titleRow}>
						<h2 id="location-heading" style={sharedStyles.sectionTitle}>
							Location
						</h2>
						{/* @spec SETUP-PAGE-006 */}
						<Link to="/setup?step=location" style={styles.action}>
							Change
						</Link>
					</div>
					<dl style={styles.facts}>
						<dt style={styles.term}>Place</dt>
						<dd style={styles.fact}>{location.place_name ?? "—"}</dd>
						<dt style={styles.term}>Coordinates</dt>
						<dd style={styles.fact}>
							{`${location.latitude.toFixed(4)}, ${location.longitude.toFixed(4)}`}
						</dd>
						<dt style={styles.term}>Timezone</dt>
						<dd style={styles.fact}>{location.timezone}</dd>
					</dl>
					{/* @spec SETUP-PAGE-008 */}
					<p style={styles.credit}>
						Timezone boundaries © OpenStreetMap contributors, ODbL.
					</p>
				</section>
			)}
		</div>
	);
}

/** Home Assistant's state, with Test in place and Change in your sensors. */
// @spec SETUP-PAGE-005
function HomeAssistantState({ connected }: { connected: boolean }) {
	const [said, setSaid] = useState<string | null>(null);
	async function test() {
		setSaid("Testing…");
		try {
			setSaid(testedText(await testHaConnection()));
		} catch (e) {
			setSaid(describeError(e));
		}
	}
	return (
		<>
			{connected ? "Connected" : "Not set up"}{" "}
			{connected && (
				<button type="button" style={styles.button} onClick={test}>
					Test
				</button>
			)}{" "}
			<Link
				to="/setup?step=sensors"
				style={styles.action}
				aria-label="Change Home Assistant connection"
			>
				{connected ? "Change" : "Connect"}
			</Link>
			{said && <div style={styles.said}>{said}</div>}
		</>
	);
}

// @spec SETUP-PAGE-005
function Connections({ data, now }: { data: SourcesView; now: Date }) {
	const { connections } = data;
	return (
		<section aria-labelledby="connections-heading" style={sharedStyles.card}>
			<h2 id="connections-heading" style={sharedStyles.sectionTitle}>
				Connections
			</h2>
			<dl style={styles.facts}>
				<dt style={styles.term}>Home Assistant</dt>
				<dd style={styles.fact}>
					<HomeAssistantState connected={connections.home_assistant} />
				</dd>
				<dt style={styles.term}>Forecast</dt>
				<dd style={styles.fact}>
					<span>{FORECAST_PROVIDER_NAMES[data.forecast_provider]}</span>{" "}
					{/* @spec SETUP-PAGE-013 */}
					<span style={styles.status}>
						{forecastStatusText(
							data.forecast,
							now,
							data.location?.timezone ?? "UTC",
						)}
					</span>{" "}
					<Link
						to="/setup?step=found"
						style={styles.action}
						aria-label="Change forecast provider"
					>
						Change
					</Link>
				</dd>
				<dt style={styles.term}>Posted readings</dt>
				<dd style={styles.fact}>
					{connections.posted_readings
						? "On"
						: "Off — set SENSOR_TOKEN on the server to accept readings from hubs and scripts"}
				</dd>
			</dl>
			{/* @spec SETUP-CRED-004 */}
			<p style={styles.credit}>
				A Home Assistant token or OpenWeatherMap key entered in TurfOps is
				stored in its database, so backups of that database contain it.
			</p>
		</section>
	);
}

const styles: Record<string, React.CSSProperties> = {
	page: { display: "flex", flexDirection: "column", gap: "1.25rem" },
	grid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fill, minmax(15rem, 1fr))",
		gap: "0.75rem",
	},
	nothing: { fontSize: "0.85rem", color: "#4a5568", margin: "0.5rem 0 0" },
	facts: {
		display: "grid",
		gridTemplateColumns: "max-content 1fr",
		gap: "0.35rem 1rem",
		margin: 0,
	},
	term: { fontSize: "0.8rem", color: "#718096" },
	fact: { fontSize: "0.85rem", color: "#2d3748", margin: 0 },
	credit: { fontSize: "0.75rem", color: "#718096", margin: "0.75rem 0 0" },
	titleRow: { display: "flex", alignItems: "baseline", gap: "0.75rem" },
	button: {
		background: "none",
		border: "1px solid #2f855a",
		borderRadius: 4,
		padding: "0 0.4rem",
		color: "#2f855a",
		fontWeight: 600,
		fontSize: "0.75rem",
		cursor: "pointer",
	},
	said: { fontSize: "0.8rem", color: "#4a5568", marginTop: "0.25rem" },
	status: { fontSize: "0.8rem", color: "#4a5568" },
	action: {
		fontSize: "0.8rem",
		color: "#2f855a",
		fontWeight: 600,
		textDecoration: "none",
	},
};
