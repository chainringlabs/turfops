import { useState } from "react";
import { useNavigate } from "react-router";
import { applySetup, getPreview, getStations } from "../../api/client";
import { describeError } from "../../api/errors";
import { sharedStyles } from "../../styles/shared";
import type {
	ChosenPoint,
	Pins,
	Preview,
	StationChoice,
} from "../../types/setup";
import type { ForecastProvider } from "../../types/sources";
import { FORECAST_PROVIDER_NAMES } from "../../types/sources";
import { planSummary, stationLine } from "./display";
import { setupStyles as styles } from "./styles";

interface FoundStepProps {
	chosen: ChosenPoint;
	/** The pins to start from: those stored, when the point is the stored one. */
	pins: Pins;
	provider: ForecastProvider;
	/** Whether an OpenWeatherMap key is stored. */
	keyStored: boolean;
	onBack: () => void;
}

const PROVIDERS = Object.keys(FORECAST_PROVIDER_NAMES) as ForecastProvider[];

// @spec SETUP-PLAN-001, SETUP-PLAN-002, SETUP-PLAN-003, SETUP-PLAN-004, SETUP-PLAN-005, SETUP-PLAN-006
/** Step 2: the plan the point would get, pins, the forecast provider, and the one save. */
export default function FoundStep({
	chosen,
	pins: startPins,
	provider: startProvider,
	keyStored,
	onBack,
}: FoundStepProps) {
	const navigate = useNavigate();
	const [plan, setPlan] = useState<Preview>(chosen.preview);
	const [pins, setPins] = useState<Pins>(startPins);
	const [opened, setOpened] = useState<string | null>(null);
	const [choices, setChoices] = useState<StationChoice[] | null>(null);
	const [provider, setProvider] = useState<ForecastProvider>(startProvider);
	const [key, setKey] = useState("");
	const [replacing, setReplacing] = useState(false);
	const [problem, setProblem] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);

	async function replan(next: Pins) {
		setProblem(null);
		setBusy(true);
		try {
			setPlan(await getPreview(chosen.latitude, chosen.longitude, next));
			setPins(next);
			setOpened(null);
		} catch (e) {
			setProblem(describeError(e));
		} finally {
			setBusy(false);
		}
	}

	async function open(input: string) {
		setOpened(input);
		setChoices(null);
		try {
			setChoices(await getStations(input, chosen.latitude, chosen.longitude));
		} catch (e) {
			setOpened(null);
			setProblem(describeError(e));
		}
	}

	const askKey = provider === "OpenWeatherMap" && (!keyStored || replacing);

	async function save() {
		if (askKey && !key.trim())
			return setProblem("OpenWeatherMap needs its key.");
		setProblem(null);
		setBusy(true);
		try {
			await applySetup({
				location: {
					latitude: chosen.latitude,
					longitude: chosen.longitude,
					place_name: chosen.place_name,
				},
				pins: Object.entries(pins).map(([input, station_ref]) => ({
					input,
					station_ref,
				})),
				forecast_provider: askKey
					? { provider, key: key.trim() }
					: { provider },
			});
			navigate("/sources", { state: { saved: true } });
		} catch (e) {
			setProblem(describeError(e));
			setBusy(false);
		}
	}

	return (
		<div style={styles.step}>
			<h2 style={styles.heading}>What we found</h2>
			<div style={{ overflowX: "auto" }}>
				<table style={sharedStyles.table} aria-label="What this location gets">
					<thead>
						<tr>
							{["Input", "Speaks for it", "Behind it"].map((h) => (
								<th key={h} style={sharedStyles.th}>
									{h}
								</th>
							))}
						</tr>
					</thead>
					<tbody>
						{plan.inputs.map((row) => {
							const [first, ...rest] = row.sources;
							const pinned = pins[row.input] !== undefined;
							return [
								<tr key={row.input}>
									<td style={sharedStyles.td}>
										<div>{row.label}</div>

										{pinned ? (
											<button
												type="button"
												style={styles.link}
												disabled={busy}
												onClick={() => {
													const { [row.input]: _, ...others } = pins;
													void replan(others);
												}}
											>
												Unpin
											</button>
										) : (
											<button
												type="button"
												style={styles.link}
												disabled={busy}
												onClick={() => open(row.input)}
											>
												Pin a station
											</button>
										)}
									</td>
									<td style={sharedStyles.td}>
										{first ? (
											<>
												{first.name}
												{first.distance_miles !== null &&
													` · ${Math.round(first.distance_miles)} mi`}
												{first.pinned && (
													<span style={styles.note}> · Pinned</span>
												)}
											</>
										) : (
											"No source here"
										)}
									</td>
									<td style={sharedStyles.td}>
										{rest.length > 0
											? `Then: ${rest.map((s) => s.name).join(", ")}`
											: "—"}
									</td>
								</tr>,
								opened === row.input && (
									<tr key={`${row.input}-stations`}>
										<td colSpan={3} style={sharedStyles.td}>
											{choices === null ? (
												<span style={styles.note}>Loading stations…</span>
											) : choices.length === 0 ? (
												<span style={styles.note}>
													No station within 150 miles reports this.
												</span>
											) : (
												<ul style={styles.choices}>
													{choices.map((station) => (
														<li key={station.station_ref}>
															<button
																type="button"
																style={styles.choice}
																onClick={() =>
																	replan({
																		...pins,
																		[row.input]: station.station_ref,
																	})
																}
															>
																{stationLine(station)}
															</button>
														</li>
													))}
												</ul>
											)}
										</td>
									</tr>
								),
							];
						})}
					</tbody>
				</table>
			</div>
			<div>
				{planSummary(plan.inputs).map((line) => (
					<p key={line} style={styles.note}>
						{line}
					</p>
				))}
			</div>

			<div style={styles.row}>
				<label style={styles.field}>
					<span style={styles.label}>Forecast provider</span>
					<select
						style={styles.input}
						value={provider}
						onChange={(e) => setProvider(e.target.value as ForecastProvider)}
					>
						{PROVIDERS.map((p) => (
							<option key={p} value={p}>
								{FORECAST_PROVIDER_NAMES[p]}
							</option>
						))}
					</select>
				</label>
				{askKey && (
					<label style={styles.field}>
						<span style={styles.label}>OpenWeatherMap key</span>
						<input
							style={styles.input}
							type="password"
							autoComplete="off"
							value={key}
							onChange={(e) => setKey(e.target.value)}
						/>
					</label>
				)}
			</div>
			{provider === "OpenWeatherMap" && keyStored && !replacing && (
				<p style={styles.note}>
					A key is stored.{" "}
					<button
						type="button"
						style={styles.link}
						onClick={() => setReplacing(true)}
					>
						Replace the key
					</button>
				</p>
			)}

			{problem && <div style={sharedStyles.error}>{problem}</div>}

			<div style={styles.row}>
				<button
					type="button"
					style={styles.button}
					onClick={onBack}
					disabled={busy}
				>
					Back
				</button>
				<button
					type="button"
					style={styles.primary}
					onClick={save}
					disabled={busy}
				>
					Save
				</button>
			</div>
		</div>
	);
}
