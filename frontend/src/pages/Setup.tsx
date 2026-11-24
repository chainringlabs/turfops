// Setup: the steps that tell TurfOps where the lawn is and what to read for it. The step
// is in the address, so the Sources page can open the one it means.
import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { getPreview, getSources } from "../api/client";
import { describeError } from "../api/errors";
import FoundStep from "../components/setup/FoundStep";
import LocationStep from "../components/setup/LocationStep";
import SensorsStep from "../components/setup/SensorsStep";
import { sharedStyles } from "../styles/shared";
import type { ChosenPoint, Pins } from "../types/setup";
import type { SourcesView } from "../types/sources";

const STEPS = [
	{ id: "location", title: "Location" },
	{ id: "found", title: "What we found" },
	{ id: "sensors", title: "Your sensors" },
] as const;

/** The pins stored for each input. */
function storedPins(view: SourcesView): Pins {
	return Object.fromEntries(
		view.inputs
			.filter((i) => i.pinned_station_ref !== null)
			.map((i) => [i.input, i.pinned_station_ref as number]),
	);
}

// @spec SETUP-FLOW-002, SETUP-FLOW-004, SETUP-FLOW-006
export default function Setup() {
	const [params, setParams] = useSearchParams();
	const navigate = useNavigate();
	/** Whether the location and what we found were saved on this visit. */
	const [saved, setSaved] = useState(false);
	const [stored, setStored] = useState<SourcesView | null>(null);
	const [chosen, setChosen] = useState<ChosenPoint | null>(null);
	const [pins, setPins] = useState<Pins>({});
	const [error, setError] = useState<string | null>(null);
	// The step setup was opened on; moving between steps afterwards keeps their state.
	const [openedOn] = useState(() => params.get("step"));

	// What is stored fills in the steps.
	useEffect(() => {
		const controller = new AbortController();
		getSources(controller.signal)
			.then(async (view) => {
				const at = view.location;
				const wantsFound = openedOn === "found";
				if (at && wantsFound) {
					const held = storedPins(view);
					const preview = await getPreview(at.latitude, at.longitude, held);
					setPins(held);
					setChosen({
						latitude: at.latitude,
						longitude: at.longitude,
						place_name: at.place_name,
						preview,
					});
				}
				setStored(view);
			})
			.catch((e: unknown) => {
				if (!controller.signal.aborted) setError(describeError(e));
			});
		return () => controller.abort();
	}, [openedOn]);

	if (error) return <div style={sharedStyles.error}>{error}</div>;
	if (!stored) return <div style={sharedStyles.loading}>Loading setup…</div>;

	// What we found needs a point; without one, setup starts at the location. Your
	// sensors needs none.
	const asked = params.get("step");
	const step =
		asked === "sensors"
			? "sensors"
			: asked === "found" && chosen
				? "found"
				: "location";

	// Opened on one step from the Sources page, setup goes back there after it;
	// walked through, it goes on.
	function foundSaved() {
		if (openedOn === "found") {
			navigate("/sources", { state: { saved: true } });
			return;
		}
		setSaved(true);
		setParams({ step: "sensors" });
	}
	const done = () => navigate("/sources", { state: { saved } });

	function next(point: ChosenPoint) {
		const at = stored?.location;
		const same =
			at && at.latitude === point.latitude && at.longitude === point.longitude;
		// Pins were chosen for the stored point; a new point starts with none.
		setPins(same && stored ? storedPins(stored) : {});
		setChosen(point);
		setParams({ step: "found" });
	}

	return (
		<div style={styles.page}>
			<h1 style={sharedStyles.pageTitle}>Set up your lawn's weather</h1>
			<ol aria-label="Setup steps" style={styles.steps}>
				{STEPS.map((s, n) => (
					<li
						key={s.id}
						aria-current={s.id === step ? "step" : undefined}
						style={s.id === step ? styles.current : styles.stepName}
					>
						{n + 1}. {s.title}
					</li>
				))}
			</ol>
			<div style={sharedStyles.card}>
				{step === "found" && chosen ? (
					<FoundStep
						key={`${chosen.latitude},${chosen.longitude}`}
						chosen={chosen}
						pins={pins}
						provider={stored.forecast_provider}
						keyStored={stored.connections.openweathermap_key}
						onBack={() => setParams({ step: "location" })}
						onSaved={foundSaved}
					/>
				) : step === "sensors" ? (
					<SensorsStep
						postedEnabled={stored.connections.posted_readings}
						onDone={done}
					/>
				) : (
					<LocationStep stored={stored.location} onNext={next} />
				)}
			</div>
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	page: {
		display: "flex",
		flexDirection: "column",
		gap: "1rem",
		maxWidth: "56rem",
	},
	steps: {
		display: "flex",
		flexWrap: "wrap",
		gap: "1rem",
		listStyle: "none",
		margin: 0,
		padding: 0,
		fontSize: "0.85rem",
	},
	stepName: { color: "#718096" },
	current: { color: "#1a202c", fontWeight: 700 },
};
