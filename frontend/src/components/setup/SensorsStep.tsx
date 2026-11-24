import { useState } from "react";
import EntityList from "./sensors/EntityList";
import HaConnection from "./sensors/HaConnection";
import MappedSensors from "./sensors/MappedSensors";
import PostedSensorForm from "./sensors/PostedSensorForm";
import { setupStyles as styles } from "./styles";

interface SensorsStepProps {
	/** Whether the server takes posted readings. */
	postedEnabled: boolean;
	/** "Continue" walked through, "Done" when opened from the Sources page. */
	doneLabel: string;
	onDone: () => void;
}

// @spec SETUP-HA-001, SETUP-MAP-001, SETUP-MAP-010, SETUP-MAP-013
/** Step 3: the owner's sensors, from Home Assistant or posted. Each change saves at once. */
export default function SensorsStep({
	postedEnabled,
	onDone,
}: SensorsStepProps) {
	const [connected, setConnected] = useState(false);
	/** Bumped on every mapping change, so both lists read again. */
	const [version, setVersion] = useState(0);
	const changed = () => setVersion((v) => v + 1);

	return (
		<div style={styles.step}>
			<h2 style={styles.heading}>Your sensors</h2>
			<p style={styles.note}>
				Optional. A sensor of your own speaks for its input ahead of any station
				or estimate.
			</p>

			<section aria-label="Home Assistant" style={styles.step}>
				<h3 style={styles.subheading}>Home Assistant</h3>
				<HaConnection onConnected={setConnected} />
				{connected && <EntityList version={version} onMapped={changed} />}
			</section>

			<section aria-label="Sensors not in Home Assistant" style={styles.step}>
				<h3 style={styles.subheading}>Sensors not in Home Assistant</h3>
				<PostedSensorForm enabled={postedEnabled} onMapped={changed} />
			</section>

			<section aria-label="Mapped sensors" style={styles.step}>
				<h3 style={styles.subheading}>Mapped sensors</h3>
				<MappedSensors version={version} onUnmapped={changed} />
			</section>

			<button type="button" style={styles.primary} onClick={onDone}>
				Done
			</button>
		</div>
	);
}
