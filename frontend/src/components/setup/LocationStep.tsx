import { useState } from "react";
import { getPlaces, getPreview } from "../../api/client";
import { describeError } from "../../api/errors";
import { sharedStyles } from "../../styles/shared";
import type { ChosenPoint, Place, Preview } from "../../types/setup";
import type { LocationView } from "../../types/sources";
import { nearestSoilText, parseCoordinate } from "./display";
import { setupStyles as styles } from "./styles";

interface LocationStepProps {
	/** The lawn's stored location, when it has one. */
	stored: LocationView | null;
	onNext: (chosen: ChosenPoint) => void;
}

function placeName(place: Place): string {
	return place.state ? `${place.name}, ${place.state}` : place.name;
}

/** Whatever stopped the search (the service, the network, a timeout), coordinates still work. */
const SEARCH_DOWN = "Place search is unavailable; enter coordinates instead.";

const coordinates = (lat: number, lon: number) =>
	`${lat.toFixed(4)}, ${lon.toFixed(4)}`;

// @spec SETUP-LOC-001, SETUP-LOC-002, SETUP-LOC-003, SETUP-LOC-004, SETUP-LOC-005, SETUP-LOC-006
/** Step 1: find the lawn by search or coordinates, and check it before going on. */
export default function LocationStep({ stored, onNext }: LocationStepProps) {
	const [query, setQuery] = useState("");
	const [results, setResults] = useState<Place[] | null>(null);
	const [lat, setLat] = useState("");
	const [lon, setLon] = useState("");
	const [problem, setProblem] = useState<string | null>(null);
	const [checking, setChecking] = useState<{
		place_name: string | null;
		preview: Preview;
	} | null>(null);
	const [busy, setBusy] = useState(false);

	// The typed text leaves the app only here, when the owner asks for the search.
	async function search(event: React.FormEvent) {
		event.preventDefault();
		if (!query.trim()) return;
		setProblem(null);
		setBusy(true);
		try {
			setResults(await getPlaces(query.trim()));
		} catch {
			// @spec SETUP-FAIL-001
			setProblem(SEARCH_DOWN);
		} finally {
			setBusy(false);
		}
	}

	async function check(
		latitude: number,
		longitude: number,
		name: string | null,
	) {
		setProblem(null);
		setChecking(null);
		setBusy(true);
		try {
			setChecking({
				place_name: name,
				preview: await getPreview(latitude, longitude),
			});
		} catch (e) {
			setProblem(describeError(e));
		} finally {
			setBusy(false);
		}
	}

	function takeCoordinates(event: React.FormEvent) {
		event.preventDefault();
		const latitude = parseCoordinate(lat, "Latitude");
		const longitude = parseCoordinate(lon, "Longitude");
		if ("problem" in latitude) return setProblem(latitude.problem);
		if ("problem" in longitude) return setProblem(longitude.problem);
		void check(latitude.value, longitude.value, null);
	}

	const moved =
		stored &&
		checking &&
		(checking.preview.latitude !== stored.latitude ||
			checking.preview.longitude !== stored.longitude);

	return (
		<div style={styles.step}>
			<h2 style={styles.heading}>Where is your lawn?</h2>
			{stored && (
				<p style={styles.note}>
					Now: {stored.place_name ?? "Typed coordinates"} (
					{coordinates(stored.latitude, stored.longitude)})
				</p>
			)}

			<form onSubmit={search} style={styles.row}>
				<label style={styles.field}>
					<span style={styles.label}>Place name or postal code</span>
					<input
						style={styles.input}
						value={query}
						onChange={(e) => setQuery(e.target.value)}
					/>
				</label>
				<button type="submit" style={styles.button} disabled={busy}>
					Search
				</button>
			</form>
			<p style={styles.note}>
				What you type is sent to the Open-Meteo place search to find matching
				places; nothing else about you is sent.
			</p>
			{results &&
				(results.length === 0 ? (
					<p style={styles.note}>
						No places found. Try a postal code, or type coordinates.
					</p>
				) : (
					<ul style={styles.choices}>
						{results.map((place) => (
							<li key={`${place.latitude},${place.longitude}`}>
								<button
									type="button"
									style={styles.choice}
									onClick={() =>
										check(place.latitude, place.longitude, placeName(place))
									}
								>
									{placeName(place)} ·{" "}
									{coordinates(place.latitude, place.longitude)}
								</button>
							</li>
						))}
					</ul>
				))}

			<form onSubmit={takeCoordinates} style={styles.row}>
				<label style={styles.field}>
					<span style={styles.label}>Latitude</span>
					<input
						style={styles.input}
						inputMode="decimal"
						value={lat}
						onChange={(e) => setLat(e.target.value)}
					/>
				</label>
				<label style={styles.field}>
					<span style={styles.label}>Longitude</span>
					<input
						style={styles.input}
						inputMode="decimal"
						value={lon}
						onChange={(e) => setLon(e.target.value)}
					/>
				</label>
				<button type="submit" style={styles.button} disabled={busy}>
					Use these coordinates
				</button>
			</form>

			{problem && <div style={sharedStyles.error}>{problem}</div>}

			{checking && (
				<div style={sharedStyles.card}>
					<dl style={styles.facts}>
						<dt style={styles.term}>Place</dt>
						<dd style={styles.fact}>
							{checking.place_name ?? "Typed coordinates"}
						</dd>
						<dt style={styles.term}>Coordinates</dt>
						<dd style={styles.fact}>
							{coordinates(
								checking.preview.latitude,
								checking.preview.longitude,
							)}
						</dd>
						<dt style={styles.term}>Timezone</dt>
						<dd style={styles.fact}>{checking.preview.timezone}</dd>
					</dl>
					<p style={styles.note}>
						{nearestSoilText(checking.preview.nearest_soil_station)}
					</p>
					{moved && (
						<div style={sharedStyles.notice}>
							Station and grid history for the old location will be replaced and
							downloaded again. Your sensors' history is kept.
						</div>
					)}
					<button
						type="button"
						style={styles.primary}
						onClick={() =>
							onNext({
								latitude: checking.preview.latitude,
								longitude: checking.preview.longitude,
								place_name: checking.place_name,
								preview: checking.preview,
							})
						}
					>
						Continue
					</button>
				</div>
			)}
		</div>
	);
}
