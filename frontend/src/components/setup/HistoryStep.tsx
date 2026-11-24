import { useCallback, useEffect, useState } from "react";
import { Link } from "react-router";
import { getSources } from "../../api/client";
import { describeError } from "../../api/errors";
import { sharedStyles } from "../../styles/shared";
import type { SourceView } from "../../types/sources";
import { historyText } from "../sources/display";
import { setupStyles as styles } from "./styles";

// @spec SETUP-FLOW-005, SETUP-FLOW-006, SETUP-FLOW-007
/** Step 5: how far each source's history has loaded, and what works meanwhile. */
export default function HistoryStep() {
	const [sources, setSources] = useState<SourceView[] | null>(null);
	const [problem, setProblem] = useState<string | null>(null);

	// Read on opening and when the owner asks; a backfill takes minutes, so no polling.
	const check = useCallback(() => {
		setProblem(null);
		getSources()
			.then((view) => setSources(view.sources))
			.catch((e: unknown) => setProblem(describeError(e)));
	}, []);
	useEffect(check, [check]);

	const year = new Date().getFullYear();
	// A sensor keeps no history to download.
	const fetched = (sources ?? [])
		.filter((s) => s.kind !== "Sensor")
		.map((s) => ({ source: s, history: historyText(s, year) ?? "—" }));
	const loading = fetched.some((f) => f.history !== "Complete");

	return (
		<div style={styles.step}>
			<h2 style={styles.heading}>Loading history</h2>
			{problem && <div style={sharedStyles.error}>{problem}</div>}
			{sources && (
				<>
					<ul aria-label="History by source" style={styles.choices}>
						{fetched.map(({ source, history }) => (
							<li key={source.id} style={styles.entity}>
								<div style={styles.entityRow}>
									<span>{source.name}</span>
									<span style={styles.note}>{history}</span>
								</div>
							</li>
						))}
					</ul>
					{loading ? (
						<div>
							<p style={styles.note}>
								Works now: current conditions, the forecast, and disease risk
								for recent days.
							</p>
							<p style={styles.note}>
								Waits for the history: typical dates on the Timing page, and the
								seasonal plan.
							</p>
						</div>
					) : (
						<p style={styles.note}>Everything has its history.</p>
					)}
				</>
			)}
			<div style={styles.row}>
				<button type="button" style={styles.button} onClick={check}>
					Check again
				</button>
				<Link
					to="/"
					style={{
						...styles.primary,
						textDecoration: "none",
						fontSize: "0.85rem",
					}}
				>
					Go to the dashboard
				</Link>
				<Link to="/sources" style={styles.link}>
					See your sources
				</Link>
			</div>
		</div>
	);
}
