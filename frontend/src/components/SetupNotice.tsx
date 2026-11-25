import { Link } from "react-router";

// @spec SETUP-FLOW-001
/** The one notice shown in place of weather content while the lawn has no location. */
export default function SetupNotice() {
	return (
		<section style={styles.notice} aria-labelledby="setup-notice-title">
			<h2 id="setup-notice-title" style={styles.title}>
				Tell TurfOps where your lawn is
			</h2>
			<p style={styles.text}>
				Weather, soil temperature, timing windows and disease risk all come from
				your lawn's location. Until it is set, dates use UTC.
			</p>
			<Link to="/setup?step=location" style={styles.action}>
				Set your location
			</Link>
		</section>
	);
}

const styles: Record<string, React.CSSProperties> = {
	notice: {
		padding: "1rem 1.25rem",
		backgroundColor: "#ebf8ff",
		border: "1px solid #90cdf4",
		borderRadius: 8,
		marginBottom: "1rem",
	},
	title: { fontSize: "1.05rem", margin: "0 0 0.35rem", color: "#1a202c" },
	text: {
		fontSize: "0.9rem",
		color: "#2d3748",
		margin: "0 0 0.75rem",
		lineHeight: 1.5,
	},
	action: {
		display: "inline-block",
		padding: "0.45rem 0.9rem",
		backgroundColor: "#2b6cb0",
		color: "#fff",
		borderRadius: 6,
		textDecoration: "none",
		fontSize: "0.9rem",
		fontWeight: 600,
	},
};
