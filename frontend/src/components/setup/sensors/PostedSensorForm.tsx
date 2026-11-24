import { useState } from "react";
import { mapSensor } from "../../../api/client";
import { describeError } from "../../../api/errors";
import { sharedStyles } from "../../../styles/shared";
import { exampleRequest, INPUT_CHOICES, mappingOf } from "../sensors";
import { setupStyles as styles } from "../styles";

interface PostedSensorFormProps {
	/** Whether the server takes posted readings (SENSOR_TOKEN is set). */
	enabled: boolean;
	onMapped: () => void;
}

// @spec SETUP-MAP-010, SETUP-MAP-014
/** A sensor that is not in Home Assistant: name and input, then the request to send. */
export default function PostedSensorForm({
	enabled,
	onMapped,
}: PostedSensorFormProps) {
	const [name, setName] = useState("");
	const [choice, setChoice] = useState("");
	const [added, setAdded] = useState<{ name: string; choice: string } | null>(
		null,
	);
	const [problem, setProblem] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);

	async function add(event: React.FormEvent) {
		event.preventDefault();
		setProblem(null);
		setBusy(true);
		try {
			await mapSensor({ name: name.trim(), ...mappingOf(choice) });
			setAdded({ name: name.trim(), choice });
			setName("");
			setChoice("");
			onMapped();
		} catch (e) {
			setProblem(describeError(e));
		} finally {
			setBusy(false);
		}
	}

	return (
		<div style={styles.step}>
			{!enabled && (
				<p style={styles.note}>
					Taking readings from hubs and scripts needs SENSOR_TOKEN set on the
					server; until it is, a sensor added here waits for them.
				</p>
			)}
			<form onSubmit={add} style={styles.row}>
				<label style={styles.field}>
					<span style={styles.label}>Sensor name</span>
					<input
						style={styles.input}
						value={name}
						onChange={(e) => setName(e.target.value)}
					/>
				</label>
				<label style={styles.field}>
					<span style={styles.label}>Measures</span>
					<select
						style={styles.input}
						value={choice}
						onChange={(e) => setChoice(e.target.value)}
					>
						<option value="">Choose…</option>
						{INPUT_CHOICES.map((c) => (
							<option key={c.value} value={c.value}>
								{c.label}
							</option>
						))}
					</select>
				</label>
				<button
					type="submit"
					style={styles.button}
					disabled={busy || !name.trim() || !choice}
				>
					Add sensor
				</button>
			</form>
			{problem && <div style={sharedStyles.error}>{problem}</div>}
			{added && (
				<div>
					<p style={styles.note}>
						Send {added.name}'s readings like this, with your token in
						SENSOR_TOKEN:
					</p>
					<pre style={styles.code}>
						{exampleRequest(window.location.origin, added.name, added.choice)}
					</pre>
				</div>
			)}
		</div>
	);
}
