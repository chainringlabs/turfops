import { useEffect, useState } from "react";
import { getHaEntities } from "../../../api/client";
import { describeError } from "../../../api/errors";
import { sharedStyles } from "../../../styles/shared";
import type { EntityView } from "../../../types/sensors";
import { observedText } from "../../sources/display";
import { choiceLabel } from "../sensors";
import { setupStyles as styles } from "../styles";
import MapForm from "./MapForm";

interface EntityListProps {
	/** Changes whenever the mapped sensors change, so the list is read again. */
	version: number;
	onMapped: () => void;
}

const HERE = Intl.DateTimeFormat().resolvedOptions().timeZone;

// @spec SETUP-MAP-001
/** Home Assistant's numeric sensors, filtered by text, each mappable in place. */
export default function EntityList({ version, onMapped }: EntityListProps) {
	const [entities, setEntities] = useState<EntityView[] | null>(null);
	const [filter, setFilter] = useState("");
	const [mapping, setMapping] = useState<string | null>(null);
	const [problem, setProblem] = useState<string | null>(null);

	// biome-ignore lint/correctness/useExhaustiveDependencies: `version` is the signal to read again.
	useEffect(() => {
		let current = true;
		setProblem(null);
		getHaEntities()
			.then((list) => current && setEntities(list))
			.catch((e: unknown) => current && setProblem(describeError(e)));
		return () => {
			current = false;
		};
	}, [version]);

	if (problem) return <div style={sharedStyles.error}>{problem}</div>;
	if (!entities)
		return <p style={styles.note}>Reading Home Assistant's sensors…</p>;

	const wanted = filter.trim().toLowerCase();
	const shown = entities.filter(
		(e) =>
			!wanted ||
			e.friendly_name.toLowerCase().includes(wanted) ||
			e.entity_id.toLowerCase().includes(wanted),
	);
	const now = new Date();

	return (
		<div style={styles.step}>
			{/* In a column, a row field's growth would stretch it tall. */}
			<label style={{ ...styles.field, flex: "none" }}>
				<span style={styles.label}>Filter sensors</span>
				<input
					style={styles.input}
					value={filter}
					onChange={(e) => setFilter(e.target.value)}
				/>
			</label>
			<ul style={styles.choices}>
				{shown.map((entity) => (
					<li key={entity.entity_id} style={styles.entity}>
						<div style={styles.entityRow}>
							<div>
								<div>{entity.friendly_name}</div>
								<div style={styles.note}>
									{entity.value} {entity.unit ?? ""} ·{" "}
									{observedText(entity.reported_at, now, HERE)} ·{" "}
									{entity.entity_id}
								</div>
							</div>
							{entity.mapped_to ? (
								<span style={styles.note}>
									Mapped to{" "}
									{entity.mapped_to === "rain"
										? "Rain"
										: choiceLabel(entity.mapped_to, null)}
								</span>
							) : (
								mapping !== entity.entity_id && (
									<button
										type="button"
										style={styles.link}
										onClick={() => setMapping(entity.entity_id)}
									>
										Map
									</button>
								)
							)}
						</div>
						{mapping === entity.entity_id && (
							<MapForm
								entity={entity}
								onCancel={() => setMapping(null)}
								onMapped={() => {
									setMapping(null);
									onMapped();
								}}
							/>
						)}
					</li>
				))}
			</ul>
		</div>
	);
}
