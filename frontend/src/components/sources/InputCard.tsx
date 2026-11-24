import type { InputView } from "../../types/sources";
import BasisLabel from "./BasisLabel";
import { accuracyLines, formatValue, observedText } from "./display";

interface InputCardProps {
	input: InputView;
	now: Date;
	timeZone: string;
}

// @spec SETUP-PAGE-001, SETUP-PAGE-002, SETUP-PAGE-009
/** One input: its newest value and basis, what stands behind it, and an estimate's accuracy. */
export default function InputCard({ input, now, timeZone }: InputCardProps) {
	const held = input.in_use;
	const behind = input.sources.slice(held ? 1 : 0).map((s) => s.name);
	const pinned = input.sources.find((s) => s.pinned);
	return (
		<div style={styles.card}>
			<div style={styles.label}>{input.label}</div>
			{held ? (
				<>
					<div style={styles.value}>{formatValue(input.input, held.value)}</div>
					{input.input === "rain" && (
						<div style={styles.meta}>Rain in the newest hour</div>
					)}
					<div style={styles.meta}>
						<BasisLabel basis={held.basis} link={false} />
					</div>
					<div style={styles.meta}>
						{observedText(held.observed_at, now, timeZone)}
					</div>
				</>
			) : (
				<>
					<div style={styles.value}>--</div>
					<div style={styles.meta}>No current value</div>
				</>
			)}
			{behind.length > 0 && (
				<div style={styles.meta}>Then: {behind.join(", ")}</div>
			)}
			{pinned && <div style={styles.meta}>Pinned: {pinned.name}</div>}
			{input.estimate_accuracy && (
				<div style={styles.accuracy}>
					{accuracyLines(input.estimate_accuracy).map((line) => (
						<div key={line}>{line}</div>
					))}
				</div>
			)}
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	card: {
		backgroundColor: "#fff",
		borderRadius: 8,
		padding: "0.9rem 1rem",
		boxShadow: "0 1px 3px rgba(0,0,0,0.08)",
		display: "flex",
		flexDirection: "column",
		gap: "0.25rem",
	},
	label: { fontSize: "0.8rem", fontWeight: 600, color: "#4a5568" },
	value: { fontSize: "1.4rem", fontWeight: 700, color: "#1a202c" },
	meta: { fontSize: "0.8rem", color: "#4a5568" },
	accuracy: {
		marginTop: "0.4rem",
		paddingTop: "0.4rem",
		borderTop: "1px solid #edf2f7",
		fontSize: "0.78rem",
		color: "#4a5568",
		display: "flex",
		flexDirection: "column",
		gap: "0.2rem",
	},
};
