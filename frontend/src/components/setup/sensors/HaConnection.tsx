import { useEffect, useRef, useState } from "react";
import {
	forgetHaConnection,
	getHaConnection,
	saveHaConnection,
	testHaConnection,
} from "../../../api/client";
import { describeError } from "../../../api/errors";
import { sharedStyles } from "../../../styles/shared";
import type { ConnectionView } from "../../../types/sensors";
import { testedText } from "../sensors";
import { setupStyles as styles } from "../styles";

interface HaConnectionProps {
	/** Told whether a connection is stored, on load and after any change. */
	onConnected: (connected: boolean) => void;
}

// @spec SETUP-HA-001, SETUP-HA-002, SETUP-HA-004, SETUP-HA-005
/** Connect, test, change or forget Home Assistant. */
export default function HaConnection({ onConnected }: HaConnectionProps) {
	const [stored, setStored] = useState<ConnectionView | null>(null);
	const [editing, setEditing] = useState(false);
	const [address, setAddress] = useState("");
	const [token, setToken] = useState("");
	/** What answered, and for which address and token: Save is offered for those only. */
	const [tested, setTested] = useState<{ said: string; key: string } | null>(
		null,
	);
	const [forgetting, setForgetting] = useState(false);
	const [problem, setProblem] = useState<string | null>(null);
	const [busy, setBusy] = useState(false);

	// The parent's callback may be a new function each render; the load runs once.
	const told = useRef(onConnected);
	told.current = onConnected;
	useEffect(() => {
		getHaConnection()
			.then((view) => {
				setStored(view);
				told.current(view.connected);
			})
			.catch((e: unknown) => setProblem(describeError(e)));
	}, []);

	const key = `${address.trim()}\n${token.trim()}`;

	async function run(work: () => Promise<void>) {
		setProblem(null);
		setBusy(true);
		try {
			await work();
		} catch (e) {
			setProblem(describeError(e));
		} finally {
			setBusy(false);
		}
	}

	const test = () =>
		run(async () => {
			setTested(null);
			const result = await testHaConnection({
				address: address.trim(),
				token: token.trim(),
			});
			setTested({ said: testedText(result), key });
		});

	const save = () =>
		run(async () => {
			const view = await saveHaConnection(address.trim(), token.trim());
			setStored(view);
			setEditing(false);
			setToken("");
			setTested(null);
			onConnected(view.connected);
		});

	const forget = () =>
		run(async () => {
			const view = await forgetHaConnection();
			setStored(view);
			setForgetting(false);
			onConnected(view.connected);
		});

	if (!stored) {
		return problem ? (
			<div style={sharedStyles.error}>{problem}</div>
		) : (
			<p style={styles.note}>Loading the Home Assistant connection…</p>
		);
	}

	if (stored.connected && !editing) {
		return (
			<div style={styles.step}>
				<p style={styles.note}>
					Connected to {stored.address}{" "}
					<button
						type="button"
						style={styles.link}
						onClick={() => setEditing(true)}
					>
						Change
					</button>{" "}
					<button
						type="button"
						style={styles.link}
						onClick={() => setForgetting(true)}
					>
						Forget
					</button>
				</p>
				{forgetting && (
					<div style={sharedStyles.notice}>
						Its sensors stop being read. Their mappings and history are kept.{" "}
						<button
							type="button"
							style={styles.link}
							onClick={forget}
							disabled={busy}
						>
							Forget the connection
						</button>{" "}
						<button
							type="button"
							style={styles.link}
							onClick={() => setForgetting(false)}
						>
							Keep it
						</button>
					</div>
				)}
				{problem && <div style={sharedStyles.error}>{problem}</div>}
			</div>
		);
	}

	return (
		<div style={styles.step}>
			<div style={styles.row}>
				<label style={styles.field}>
					<span style={styles.label}>Home Assistant address</span>
					<input
						style={styles.input}
						placeholder="http://homeassistant.local:8123"
						value={address}
						onChange={(e) => setAddress(e.target.value)}
					/>
				</label>
				<label style={styles.field}>
					<span style={styles.label}>Long-lived access token</span>
					<input
						style={styles.input}
						type="password"
						autoComplete="off"
						value={token}
						onChange={(e) => setToken(e.target.value)}
					/>
				</label>
				<button
					type="button"
					style={styles.button}
					onClick={test}
					disabled={busy || !address.trim() || !token.trim()}
				>
					Test
				</button>
				{/* Only what was just tested may be saved, so a mistyped token is never stored. */}
				{tested && tested.key === key && (
					<button
						type="button"
						style={styles.primary}
						onClick={save}
						disabled={busy}
					>
						Save connection
					</button>
				)}
			</div>
			{tested && tested.key === key && <p style={styles.note}>{tested.said}</p>}
			{problem && <div style={sharedStyles.error}>{problem}</div>}
			{stored.connected && (
				<button
					type="button"
					style={styles.link}
					onClick={() => setEditing(false)}
				>
					Keep the stored connection
				</button>
			)}
		</div>
	);
}
