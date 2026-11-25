import { Link } from "react-router";
import type { SourceStatus } from "../../types";
import type { ForecastStatus } from "../../types/sources";
import { statusLine } from "./display";

interface SourceStatusLineProps {
	sources: SourceStatus[];
	forecast: ForecastStatus;
}

// @spec FEED-UI-014
/** Whether every source is current, which are not, and history progress, linked to Sources. */
export default function SourceStatusLine({
	sources,
	forecast,
}: SourceStatusLineProps) {
	const line = statusLine(sources, forecast);
	if (!line) return null;
	const failing = line.text !== "All sources current";
	return (
		<p style={styles.line}>
			<span style={failing ? styles.failing : undefined}>{line.text}</span>
			{line.history && <span> · {line.history}</span>} ·{" "}
			<Link to="/sources" style={styles.link}>
				Sources
			</Link>
		</p>
	);
}

const styles: Record<string, React.CSSProperties> = {
	line: { fontSize: "0.85rem", color: "#4a5568", margin: "0 0 1rem" },
	failing: { color: "#975a16", fontWeight: 600 },
	link: { color: "#2f855a", fontWeight: 600, textDecoration: "none" },
};
