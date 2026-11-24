import { useEffect, useState } from "react";
import { getSensors, unmapSensor } from "../../../api/client";
import { describeError } from "../../../api/errors";
import { sharedStyles } from "../../../styles/shared";
import type { SensorView } from "../../../types/sensors";
import { choiceLabel } from "../sensors";
import { setupStyles as styles } from "../styles";

interface MappedSensorsProps {
	version: number;
	onUnmapped: () => void;
}

const WAY_IN = { HomeAssistant: "Home Assistant", Posted: "Posted" };

// @spec SETUP-MAP-013
/** The sensors mapped, with their input and way in, each with Unmap. */
export default function MappedSensors({
	version,
	onUnmapped,
}: MappedSensorsProps) {
	const [sensors, setSensors] = useState<SensorView[] | null>(null);
	const [problem, setProblem] = useState<string | null>(null);

	// biome-ignore lint/correctness/useExhaustiveDependencies: `version` is the signal to read again.
	useEffect(() => {
		let current = true;
		getSensors()
			.then((list) => current && setSensors(list))
			.catch((e: unknown) => current && setProblem(describeError(e)));
		return () => {
			current = false;
		};
	}, [version]);

	// Unmapping keeps the sensor's history, so it asks nothing first.
	async function unmap(id: number) {
		setProblem(null);
		try {
			setSensors(await unmapSensor(id));
			onUnmapped();
		} catch (e) {
			setProblem(describeError(e));
		}
	}

	if (problem) return <div style={sharedStyles.error}>{problem}</div>;
	if (!sensors) return null;
	if (sensors.length === 0)
		return <p style={styles.note}>No sensors mapped yet.</p>;

	return (
		<ul style={styles.choices}>
			{sensors.map((s) => (
				<li key={s.id} style={styles.entity}>
					<div style={styles.entityRow}>
						<div>
							<div>{s.name}</div>
							<div style={styles.note}>
								{choiceLabel(s.input, s.rain_kind)} · {WAY_IN[s.way_in]}
							</div>
						</div>
						<button
							type="button"
							style={styles.link}
							onClick={() => unmap(s.id)}
						>
							Unmap
						</button>
					</div>
				</li>
			))}
		</ul>
	);
}
