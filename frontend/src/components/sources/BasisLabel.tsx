import { Link } from "react-router";
import type { Basis, BasisKind } from "../../types/sources";
import { basisText } from "./display";

interface BasisLabelProps {
	basis: Basis;
	/** The value's period drew on more than one basis. */
	mixed?: boolean;
	/** Link to the Sources page; off on the Sources page itself. */
	link?: boolean;
}

/** A solid dot for a station, a solid square for a sensor, a hollow dot for an estimate. */
const MARKS: Record<BasisKind, string> = {
	Station: "●",
	Sensor: "■",
	Estimate: "○",
};

// @spec WX-VIEW-BASIS-001, WX-VIEW-BASIS-002
/** What a weather value rests on, in words and a mark, never by color alone. */
export default function BasisLabel({
	basis,
	mixed = false,
	link = true,
}: BasisLabelProps) {
	const words = basisText(basis, mixed);
	const content = (
		<>
			<span data-basis-mark aria-hidden="true" style={styles.mark}>
				{MARKS[basis.kind]}
			</span>{" "}
			<span>{words}</span>
		</>
	);
	return link ? (
		<Link to="/sources" style={styles.label}>
			{content}
		</Link>
	) : (
		<span style={styles.label}>{content}</span>
	);
}

const styles: Record<string, React.CSSProperties> = {
	label: {
		fontSize: "0.8rem",
		color: "#555",
		textDecoration: "none",
	},
	mark: { fontSize: "0.7rem" },
};
