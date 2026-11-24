import { useEffect, useState } from "react";
import { getProfile } from "../../api/client";
import { ApiError, describeError } from "../../api/errors";
import { sharedStyles } from "../../styles/shared";
import type { LawnProfile } from "../../types";
import LawnProfileForm from "../profile/LawnProfileForm";
import { setupStyles as styles } from "./styles";

interface LawnStepProps {
	/** Saved or skipped: go on to the next step. */
	onNext: () => void;
}

// @spec SETUP-FLOW-008, SETUP-FLOW-003
/** Step 4: the lawn profile, as Settings shows it. Saving never changes the location. */
export default function LawnStep({ onNext }: LawnStepProps) {
	/** `undefined` while loading; `null` when there is no profile yet. */
	const [profile, setProfile] = useState<LawnProfile | null | undefined>(
		undefined,
	);
	const [problem, setProblem] = useState<string | null>(null);

	useEffect(() => {
		getProfile()
			.then(setProfile)
			.catch((e: unknown) => {
				if (e instanceof ApiError && e.status === 404) setProfile(null);
				else setProblem(describeError(e));
			});
	}, []);

	const skip = (
		<button type="button" style={styles.button} onClick={onNext}>
			Skip
		</button>
	);

	return (
		<div style={styles.step}>
			<h2 style={styles.heading}>Your lawn</h2>
			<p style={styles.note}>
				Optional. The grass, size, soil and watering shape the advice; you can
				change them in Settings at any time.
			</p>
			{problem ? (
				<>
					<div style={sharedStyles.error}>{problem}</div>
					{skip}
				</>
			) : profile === undefined ? (
				<p style={styles.note}>Loading the lawn profile…</p>
			) : (
				<LawnProfileForm
					stored={profile}
					submitLabel="Save and continue"
					onSaved={onNext}
					extra={skip}
				/>
			)}
		</div>
	);
}
