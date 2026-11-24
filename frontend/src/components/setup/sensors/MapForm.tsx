import { useEffect, useState } from "react";
import { checkSensor, mapSensor } from "../../../api/client";
import { ApiError, describeError } from "../../../api/errors";
import { sharedStyles } from "../../../styles/shared";
import type { Checked, EntityView } from "../../../types/sensors";
import { checkLine, choiceFor, INPUT_CHOICES, mappingOf } from "../sensors";
import { setupStyles as styles } from "../styles";

interface MapFormProps {
	entity: EntityView;
	onMapped: () => void;
	onCancel: () => void;
}

// @spec SETUP-MAP-002, SETUP-MAP-003, SETUP-MAP-004, SETUP-MAP-005, SETUP-MAP-006, SETUP-MAP-007
/** Map one Home Assistant entity: name, input, the check, and the save. */
export default function MapForm({ entity, onMapped, onCancel }: MapFormProps) {
	const [name, setName] = useState(entity.friendly_name);
	const [choice, setChoice] = useState(choiceFor(entity.suggestion));
	const [checked, setChecked] = useState<Checked | null>(null);
	/** The server's question when the input already has a sensor. */
	const [replacing, setReplacing] = useState<string | null>(null);
	const [problem, setProblem] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);

	// The check runs as soon as there is an input to check against.
	useEffect(() => {
		setChecked(null);
		setReplacing(null);
		if (!choice) return;
		let current = true;
		checkSensor(entity.entity_id, mappingOf(choice).input)
			.then((result) => current && setChecked(result))
			.catch((e: unknown) => current && setProblem(describeError(e)));
		return () => {
			current = false;
		};
	}, [choice, entity.entity_id]);

	async function save(replace: boolean) {
		setProblem(null);
		setBusy(true);
		try {
			await mapSensor({
				name: name.trim(),
				entity_id: entity.entity_id,
				...mappingOf(choice),
				...(replace ? { replace: true } : {}),
			});
			onMapped();
		} catch (e) {
			if (e instanceof ApiError && e.status === 409) setReplacing(e.message);
			else setProblem(describeError(e));
		} finally {
			setBusy(false);
		}
	}

	const blocked = !checked || checked.blocking.length > 0;
	const line = checked ? checkLine(checked, mappingOf(choice).input) : null;

	return (
		<div style={styles.step}>
			<div style={styles.row}>
				<label style={styles.field}>
					<span style={styles.label}>Name</span>
					<input
						style={styles.input}
						value={name}
						onChange={(e) => setName(e.target.value)}
					/>
				</label>
				<label style={styles.field}>
					<span style={styles.label}>Input</span>
					<select
						style={styles.input}
						value={choice}
						onChange={(e) => setChoice(e.target.value)}
					>
						<option value="">Choose what it measures…</option>
						{INPUT_CHOICES.map((c) => (
							<option key={c.value} value={c.value}>
								{c.label}
							</option>
						))}
					</select>
				</label>
			</div>
			{line && <p style={styles.note}>{line}</p>}
			{checked?.blocking.map((b) => (
				<div key={b} style={sharedStyles.error}>
					{b}
				</div>
			))}
			{checked?.warnings.map((w) => (
				<div key={w} style={sharedStyles.notice}>
					{w}
				</div>
			))}
			{problem && <div style={sharedStyles.error}>{problem}</div>}
			{replacing ? (
				<div style={sharedStyles.notice}>
					{replacing}{" "}
					<button
						type="button"
						style={styles.link}
						onClick={() => save(true)}
						disabled={busy}
					>
						Replace
					</button>{" "}
					<button
						type="button"
						style={styles.link}
						onClick={() => setReplacing(null)}
					>
						Cancel
					</button>
				</div>
			) : (
				<div style={styles.row}>
					<button
						type="button"
						style={styles.primary}
						onClick={() => save(false)}
						disabled={busy || blocked || !name.trim()}
					>
						Save
					</button>
					<button type="button" style={styles.button} onClick={onCancel}>
						Cancel
					</button>
				</div>
			)}
		</div>
	);
}
