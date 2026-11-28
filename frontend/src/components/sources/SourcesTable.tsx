import { Link } from "react-router";
import { sharedStyles } from "../../styles/shared";
import type { SourceView } from "../../types/sources";
import { SOURCE_KIND_LABELS } from "../../types/sources";
import { historyText, observedText, orderSources, speaksFor } from "./display";

interface SourcesTableProps {
	sources: SourceView[];
	labels: Record<string, string>;
	now: Date;
	timeZone: string;
	/** Unmap a sensor Home Assistant no longer has. */
	onUnmap?: (sensorId: number) => void;
}

// @spec SETUP-PAGE-004, SETUP-PAGE-010
/** Every source in use, failing first, with its last fetch, failure, history and credit. */
export default function SourcesTable({
	sources,
	labels,
	now,
	timeZone,
	onUnmap,
}: SourcesTableProps) {
	const when = (at: string | null) =>
		at ? observedText(at, now, timeZone) : "—";
	return (
		// Wide tables scroll inside their own box, never the page.
		<div style={styles.scroll}>
			<table style={sharedStyles.table} aria-label="Sources in use">
				<thead>
					<tr>
						{[
							"Source",
							"Kind",
							"Speaks for",
							"Last fetched",
							"Problem",
							"History",
							"Credit",
						].map((h) => (
							<th key={h} style={sharedStyles.th}>
								{h}
							</th>
						))}
					</tr>
				</thead>
				<tbody>
					{orderSources(sources).map((s) => (
						<tr key={s.id} style={s.last_error ? styles.failing : undefined}>
							<td style={sharedStyles.td}>{s.name}</td>
							<td style={sharedStyles.td}>{SOURCE_KIND_LABELS[s.kind]}</td>
							<td style={sharedStyles.td}>{speaksFor(s.inputs, labels)}</td>
							<td style={sharedStyles.td}>{when(s.last_success)}</td>
							<td style={sharedStyles.td}>
								{s.last_error ? (
									<>
										<span aria-hidden="true">⚠ </span>
										{s.last_error}
										<div style={styles.small}>{when(s.last_failure)}</div>
										{/* @spec SETUP-FAIL-003 */}
										{s.missing_sensor != null && (
											<div style={styles.actions}>
												{onUnmap && (
													<button
														type="button"
														style={styles.action}
														aria-label={`Unmap ${s.name}`}
														onClick={() =>
															s.missing_sensor != null &&
															onUnmap(s.missing_sensor)
														}
													>
														Unmap
													</button>
												)}
												<Link
													to="/setup?step=sensors"
													style={styles.action}
													aria-label={`Remap ${s.name}`}
												>
													Remap
												</Link>
											</div>
										)}
									</>
								) : (
									"—"
								)}
							</td>
							<td style={sharedStyles.td}>
								{historyText(s, now.getFullYear()) ?? "—"}
							</td>
							<td style={{ ...sharedStyles.td, ...styles.small }}>
								{s.attribution}
							</td>
						</tr>
					))}
				</tbody>
			</table>
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	scroll: { overflowX: "auto" },
	failing: { backgroundColor: "#fff5f5" },
	small: { fontSize: "0.75rem", color: "#718096" },
	actions: { display: "flex", gap: "0.75rem", marginTop: "0.25rem" },
	action: {
		background: "none",
		border: "none",
		padding: 0,
		fontSize: "0.8rem",
		color: "#2f855a",
		fontWeight: 600,
		textDecoration: "none",
		cursor: "pointer",
	},
};
