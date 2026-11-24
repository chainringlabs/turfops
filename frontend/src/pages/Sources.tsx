// The Sources page: what each weather input rests on, how each source is doing, the
// connections and the lawn's location.
import { useEffect, useState } from "react";
import { getSources } from "../api/client";
import { describeError } from "../api/errors";
import {
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

	// Loaded once; the owner reloads to see newer readings.
	useEffect(() => {
		const controller = new AbortController();
		getSources(controller.signal)
			.then(setData)
			.catch((e: unknown) => {
				if (!controller.signal.aborted) setError(describeError(e));
			});
		return () => controller.abort();
	}, []);

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
					The lawn has no location yet, so no weather source is in use.
				</div>
			)}

			{data.sources.length > 0 && location && (
				<section aria-labelledby="sources-heading">
					<h2 id="sources-heading" style={sharedStyles.sectionTitle}>
						By source
					</h2>
					<SourcesTable
						sources={data.sources}
						labels={labels}
						now={now}
						timeZone={location.timezone}
					/>
				</section>
			)}

			<Connections data={data} />

			{location && (
				<section aria-labelledby="location-heading" style={sharedStyles.card}>
					<h2 id="location-heading" style={sharedStyles.sectionTitle}>
						Location
					</h2>
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

// @spec SETUP-PAGE-005
function Connections({ data }: { data: SourcesView }) {
	const { connections } = data;
	return (
		<section aria-labelledby="connections-heading" style={sharedStyles.card}>
			<h2 id="connections-heading" style={sharedStyles.sectionTitle}>
				Connections
			</h2>
			<dl style={styles.facts}>
				<dt style={styles.term}>Home Assistant</dt>
				<dd style={styles.fact}>
					{connections.home_assistant ? "Connected" : "Not set up"}
				</dd>
				<dt style={styles.term}>Forecast</dt>
				<dd style={styles.fact}>
					{FORECAST_PROVIDER_NAMES[data.forecast_provider]}
				</dd>
				<dt style={styles.term}>Posted readings</dt>
				<dd style={styles.fact}>
					{connections.posted_readings
						? "On"
						: "Off — set SENSOR_TOKEN on the server to accept readings from hubs and scripts"}
				</dd>
			</dl>
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
};
