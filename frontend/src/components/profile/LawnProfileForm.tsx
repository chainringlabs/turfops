import type React from "react";
import { useState } from "react";
import { updateProfile } from "../../api/client";
import { describeError } from "../../api/errors";
import type {
	GrassType,
	IrrigationType,
	LawnProfile,
	SoilType,
} from "../../types";
import { GRASS_TYPE_LABELS } from "../../types";

const GRASS_TYPES: GrassType[] = [
	"KentuckyBluegrass",
	"TallFescue",
	"PerennialRyegrass",
	"FineFescue",
	"Bermuda",
	"Zoysia",
	"StAugustine",
	"Mixed",
];

const SOIL_TYPES: SoilType[] = [
	"Clay",
	"Loam",
	"Sandy",
	"SiltLoam",
	"ClayLoam",
	"SandyLoam",
];

const IRRIGATION_TYPES: IrrigationType[] = ["InGround", "Hose", "None"];

const IRRIGATION_LABELS: Record<IrrigationType, string> = {
	InGround: "In-Ground",
	Hose: "Hose/Sprinkler",
	None: "None",
};

const SOIL_LABELS: Record<SoilType, string> = {
	Clay: "Clay",
	Loam: "Loam",
	Sandy: "Sandy",
	SiltLoam: "Silt Loam",
	ClayLoam: "Clay Loam",
	SandyLoam: "Sandy Loam",
};

interface LawnProfileFormProps {
	/** The stored profile; `null` when there is none yet, and saving creates it. */
	stored: LawnProfile | null;
	/** The save button's words. */
	submitLabel?: string;
	/** Told the saved profile after each save. */
	onSaved?: (profile: LawnProfile) => void;
	/** Shown beside the save button, such as setup's Skip. */
	extra?: React.ReactNode;
	/** Show when the profile was created and last updated. */
	showDates?: boolean;
}

/**
 * The lawn profile form: name, grass type, zone, soil type, size and irrigation. It never
 * sends a location, so saving it leaves the stored location as it is.
 */
// @spec APP-SET-001, APP-SET-002, SETUP-FLOW-008
export default function LawnProfileForm({
	stored,
	submitLabel = "Save Profile",
	onSaved,
	extra,
	showDates = false,
}: LawnProfileFormProps) {
	const [profile, setProfile] = useState<LawnProfile | null>(stored);
	const [error, setError] = useState<string | null>(null);
	const [success, setSuccess] = useState(false);
	const [saving, setSaving] = useState(false);
	/** No lawn profile exists yet: saving the form creates it. */
	const [creating, setCreating] = useState(stored === null);

	// The form starts from the stored profile, or from defaults when there is none.
	const [name, setName] = useState(stored?.name ?? "Main Lawn");
	const [grassType, setGrassType] = useState<string>(
		stored?.grass_type ?? "TallFescue",
	);
	const [zone, setZone] = useState(stored?.usda_zone ?? "7a");
	const [soilType, setSoilType] = useState<string>(stored?.soil_type || "");
	const [size, setSize] = useState(stored?.lawn_size_sqft?.toString() || "");
	const [irrigationType, setIrrigationType] = useState<string>(
		stored?.irrigation_type || "",
	);

	const handleSave = async (e: React.FormEvent) => {
		e.preventDefault();
		setSaving(true);
		setError(null);
		setSuccess(false);

		try {
			const validGrassType = GRASS_TYPES.includes(grassType as GrassType)
				? (grassType as GrassType)
				: undefined;
			const validSoilType =
				soilType && SOIL_TYPES.includes(soilType as SoilType)
					? (soilType as SoilType)
					: ""; // "Not specified" clears it
			const validIrrigationType =
				irrigationType &&
				IRRIGATION_TYPES.includes(irrigationType as IrrigationType)
					? (irrigationType as IrrigationType)
					: "";

			if (!validGrassType) {
				setError("Please select a valid grass type");
				setSaving(false);
				return;
			}

			const updated = await updateProfile({
				name,
				grass_type: validGrassType,
				usda_zone: zone,
				soil_type: validSoilType,
				lawn_size_sqft: size ? parseFloat(size) : 0, // 0 clears it
				irrigation_type: validIrrigationType,
			});
			setProfile(updated);
			setCreating(false);
			setSuccess(true);
			onSaved?.(updated);
			setTimeout(() => setSuccess(false), 3000);
		} catch (e) {
			setError(describeError(e));
		} finally {
			setSaving(false);
		}
	};

	return (
		<div>
			{creating && (
				<div style={styles.notice}>
					There is no lawn profile yet. Fill in your lawn below; saving creates
					it.
				</div>
			)}
			{error && <div style={styles.error}>{error}</div>}
			{success && <div style={styles.success}>Profile saved successfully!</div>}

			<form onSubmit={handleSave} style={styles.form}>
				<div style={styles.grid}>
					<div>
						<label htmlFor="lawn-name" style={styles.label}>
							Lawn Name
						</label>
						<input
							id="lawn-name"
							style={styles.input}
							value={name}
							onChange={(e) => setName(e.target.value)}
							required
						/>
					</div>
					<div>
						<label htmlFor="grass-type" style={styles.label}>
							Grass Type
						</label>
						<select
							id="grass-type"
							style={styles.input}
							value={grassType}
							onChange={(e) => setGrassType(e.target.value)}
						>
							{GRASS_TYPES.map((gt) => (
								<option key={gt} value={gt}>
									{GRASS_TYPE_LABELS[gt]}
								</option>
							))}
						</select>
					</div>
					<div>
						<label htmlFor="usda-zone" style={styles.label}>
							USDA Zone
						</label>
						<input
							id="usda-zone"
							style={styles.input}
							value={zone}
							onChange={(e) => setZone(e.target.value)}
							placeholder="e.g. 7a"
							required
						/>
					</div>
					<div>
						<label htmlFor="soil-type" style={styles.label}>
							Soil Type
						</label>
						<select
							id="soil-type"
							style={styles.input}
							value={soilType}
							onChange={(e) => setSoilType(e.target.value)}
						>
							<option value="">Not specified</option>
							{SOIL_TYPES.map((st) => (
								<option key={st} value={st}>
									{SOIL_LABELS[st]}
								</option>
							))}
						</select>
					</div>
					<div>
						<label htmlFor="lawn-size-sqft" style={styles.label}>
							Lawn Size (sqft)
						</label>
						<input
							id="lawn-size-sqft"
							type="number"
							style={styles.input}
							value={size}
							onChange={(e) => setSize(e.target.value)}
							placeholder="e.g. 5000"
						/>
					</div>
					<div>
						<label htmlFor="irrigation-type" style={styles.label}>
							Irrigation Type
						</label>
						<select
							id="irrigation-type"
							style={styles.input}
							value={irrigationType}
							onChange={(e) => setIrrigationType(e.target.value)}
						>
							<option value="">Not specified</option>
							{IRRIGATION_TYPES.map((it) => (
								<option key={it} value={it}>
									{IRRIGATION_LABELS[it]}
								</option>
							))}
						</select>
					</div>
				</div>
				<div style={styles.actions}>
					<button type="submit" style={styles.saveBtn} disabled={saving}>
						{saving ? "Saving..." : submitLabel}
					</button>
					{extra}
				</div>
			</form>

			{showDates && profile && (
				<div style={styles.meta}>
					Created: {new Date(profile.created_at).toLocaleDateString()} | Last
					updated: {new Date(profile.updated_at).toLocaleString()}
				</div>
			)}
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	actions: { display: "flex", gap: "0.75rem", alignItems: "center" },
	error: {
		padding: "0.5rem 1rem",
		backgroundColor: "#fed7d7",
		color: "#c53030",
		borderRadius: 6,
		marginBottom: "1rem",
		fontSize: "0.85rem",
	},
	notice: {
		padding: "0.5rem 1rem",
		backgroundColor: "#ebf8ff",
		color: "#2a4365",
		borderRadius: 6,
		marginBottom: "1rem",
		fontSize: "0.85rem",
	},
	success: {
		padding: "0.5rem 1rem",
		backgroundColor: "#c6f6d5",
		color: "#276749",
		borderRadius: 6,
		marginBottom: "1rem",
		fontSize: "0.85rem",
	},
	form: {
		backgroundColor: "#fff",
		borderRadius: 8,
		padding: "1.5rem",
		boxShadow: "0 1px 3px rgba(0,0,0,0.08)",
		marginBottom: "1rem",
	},
	grid: {
		display: "grid",
		gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))",
		gap: "1rem",
		marginBottom: "1rem",
	},
	label: {
		display: "block",
		fontSize: "0.8rem",
		color: "#718096",
		marginBottom: 4,
		fontWeight: 600,
	},
	input: {
		width: "100%",
		padding: "0.5rem 0.75rem",
		borderRadius: 6,
		border: "1px solid #e2e8f0",
		fontSize: "0.9rem",
	},
	saveBtn: {
		padding: "0.6rem 2rem",
		backgroundColor: "#3182ce",
		color: "#fff",
		border: "none",
		borderRadius: 6,
		cursor: "pointer",
		fontWeight: 600,
		fontSize: "0.9rem",
	},
	meta: {
		fontSize: "0.75rem",
		color: "#a0aec0",
	},
};
