import { Link } from "react-router";
import type { CurrentBasis } from "../../types";
import Gauge from "../Gauge";
import type { GaugeConfig } from "../gaugeConfigs";
import BasisLabel from "../sources/BasisLabel";

interface GaugeCardProps {
	config: GaugeConfig;
	value: number | null;
	/** What the value rests on, from the summary's `current_bases`. */
	entry: CurrentBasis | undefined;
	style?: React.CSSProperties;
	/** Shown under the gauge, such as its 7-day average. */
	children?: React.ReactNode;
}

// @spec WX-VIEW-BASIS-001, WX-VIEW-BASIS-004, WX-VIEW-PAGE-003, FEED-UI-009
/** A gauge with its basis label; "No source for this location" when the plan has none. */
export default function GaugeCard({
	config,
	value,
	entry,
	style,
	children,
}: GaugeCardProps) {
	if (entry?.standing === "NoSource") {
		return (
			<div data-gauge-card style={style}>
				<span style={styles.label}>{config.label}</span>
				<p style={styles.none}>
					<span>No source for this location</span> ·{" "}
					<Link to="/sources" style={styles.link}>
						Sources
					</Link>
				</p>
			</div>
		);
	}
	return (
		<div data-gauge-card style={style}>
			<Gauge {...config} value={value} />
			{entry?.standing === "InUse" && entry.basis && (
				<div style={styles.basis}>
					<BasisLabel basis={entry.basis} />
				</div>
			)}
			{children}
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	label: { fontSize: "0.85rem", fontWeight: 600, color: "#4a5568" },
	none: { fontSize: "0.85rem", color: "#4a5568", margin: "0.75rem 0 0" },
	link: { color: "#2f855a", fontWeight: 600, textDecoration: "none" },
	basis: { marginTop: "0.4rem" },
};
