import { useEffect, useState } from "react";
import { getProfile } from "../api/client";
import { ApiError, describeError } from "../api/errors";
import LawnProfileForm from "../components/profile/LawnProfileForm";
import type { LawnProfile } from "../types";

// @spec APP-SET-001, APP-SET-002, APP-SET-003
export default function Settings() {
	const [profile, setProfile] = useState<LawnProfile | null | undefined>(
		undefined,
	);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		getProfile()
			.then(setProfile)
			.catch((e: unknown) => {
				// @spec APP-SET-003
				if (e instanceof ApiError && e.status === 404) setProfile(null);
				// @spec APP-LIST-005
				else setError(describeError(e));
			});
	}, []);

	return (
		<div>
			<h1 style={styles.title}>Lawn Profile Settings</h1>
			{error ? (
				<div style={styles.error}>{error}</div>
			) : profile === undefined ? (
				<div style={{ color: "#718096", padding: "2rem" }}>Loading...</div>
			) : (
				<LawnProfileForm stored={profile} showDates />
			)}
		</div>
	);
}

const styles: Record<string, React.CSSProperties> = {
	title: { margin: "0 0 1rem", fontSize: "1.5rem", color: "#1a202c" },
	error: {
		padding: "0.5rem 1rem",
		backgroundColor: "#fed7d7",
		color: "#c53030",
		borderRadius: 6,
		marginBottom: "1rem",
		fontSize: "0.85rem",
	},
};
