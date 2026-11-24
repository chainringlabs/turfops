// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getPreview, getSources } from "../api/client";
import type { ChosenPoint, Preview } from "../types/setup";
import type { SourcesView } from "../types/sources";
import Setup from "./Setup";

vi.mock("../api/client", () => ({ getSources: vi.fn(), getPreview: vi.fn() }));
// The steps have their own tests; here they show what they were given.
vi.mock("../components/setup/LocationStep", () => ({
	default: ({
		stored,
		onNext,
	}: {
		stored: { place_name: string } | null;
		onNext: (c: ChosenPoint) => void;
	}) => (
		<div>
			<div>location step, stored: {stored?.place_name ?? "none"}</div>
			<button
				type="button"
				onClick={() =>
					onNext({
						latitude: 40,
						longitude: -75,
						place_name: "Picked",
						preview: {} as Preview,
					})
				}
			>
				pick
			</button>
		</div>
	),
}));
vi.mock("../components/setup/FoundStep", () => ({
	default: (props: {
		chosen: ChosenPoint;
		pins: Record<string, number>;
		provider: string;
		keyStored: boolean;
		onSaved: () => void;
	}) => (
		<div>
			found step for {props.chosen.place_name} pins {JSON.stringify(props.pins)}{" "}
			provider {props.provider} key {String(props.keyStored)}
			<button type="button" onClick={props.onSaved}>
				save found
			</button>
		</div>
	),
}));
vi.mock("../components/setup/SensorsStep", () => ({
	default: (props: { postedEnabled: boolean; onDone: () => void }) => (
		<div>
			sensors step, posted {String(props.postedEnabled)}
			<button type="button" onClick={props.onDone}>
				done
			</button>
		</div>
	),
}));

const sources = vi.mocked(getSources);
const preview = vi.mocked(getPreview);

function stored(over: Partial<SourcesView> = {}): SourcesView {
	return {
		location: {
			place_name: "Avondale, PA",
			latitude: 39.82,
			longitude: -75.78,
			timezone: "America/New_York",
		},
		inputs: [
			{
				input: "soil_temp_10",
				label: "Soil temperature, 10 cm",
				standing: "Measured",
				sources: [],
				in_use: null,
				estimate_accuracy: null,
				pinned_station_ref: 9,
			},
			{
				input: "air_temp",
				label: "Air temperature",
				standing: "Measured",
				sources: [],
				in_use: null,
				estimate_accuracy: null,
				pinned_station_ref: null,
			},
		],
		sources: [],
		forecast_provider: "OpenWeatherMap",
		connections: {
			home_assistant: true,
			openweathermap_key: true,
			posted_readings: false,
		},
		...over,
	};
}

/** Where setup sent the owner. */
export function Landed() {
	const at = useLocation();
	const saved = (at.state as { saved?: boolean } | null)?.saved;
	return (
		<div>
			landed on {at.pathname}
			{saved ? " with the saved notice" : ""}
		</div>
	);
}

function show(at: string) {
	return render(
		<MemoryRouter initialEntries={[at]}>
			<Routes>
				<Route path="/setup" element={<Setup />} />
				<Route path="/sources" element={<Landed />} />
			</Routes>
		</MemoryRouter>,
	);
}

beforeEach(() => {
	sources.mockReset();
	preview.mockReset();
	preview.mockResolvedValue({} as Preview);
});

// @spec SETUP-FLOW-002
describe("the steps", () => {
	it("opens on the location step and names the steps in order", async () => {
		sources.mockResolvedValue(stored({ location: null }));
		show("/setup");
		expect(
			await screen.findByText("location step, stored: none"),
		).toBeInTheDocument();
		const steps = screen.getByRole("list", { name: "Setup steps" });
		expect(steps).toHaveTextContent(/Location.*What we found.*Your sensors/);
	});

	it("goes on to what we found with the point chosen, without saving anything", async () => {
		const user = userEvent.setup();
		sources.mockResolvedValue(stored({ location: null }));
		show("/setup");
		await user.click(await screen.findByRole("button", { name: "pick" }));
		expect(
			await screen.findByText(
				/found step for Picked pins \{\} provider OpenWeatherMap key true/,
			),
		).toBeInTheDocument();
	});
});

// @spec SETUP-FLOW-004
describe("opened on a step", () => {
	it("fills in the stored location, pins and provider on what we found", async () => {
		sources.mockResolvedValue(stored());
		show("/setup?step=found");
		expect(
			await screen.findByText(
				/found step for Avondale, PA pins \{"soil_temp_10":9\} provider OpenWeatherMap key true/,
			),
		).toBeInTheDocument();
		expect(preview).toHaveBeenCalledWith(39.82, -75.78, { soil_temp_10: 9 });
	});

	it("opens on the location step when the lawn has no location", async () => {
		sources.mockResolvedValue(stored({ location: null }));
		show("/setup?step=found");
		expect(
			await screen.findByText("location step, stored: none"),
		).toBeInTheDocument();
	});

	it("gives the location step the stored location to change", async () => {
		sources.mockResolvedValue(stored());
		show("/setup?step=location");
		expect(
			await screen.findByText("location step, stored: Avondale, PA"),
		).toBeInTheDocument();
	});
});

// @spec SETUP-FLOW-006, SETUP-PLAN-006
describe("where a step leads", () => {
	it("goes on from what we found to your sensors when walked through, then back to Sources", async () => {
		const user = userEvent.setup();
		sources.mockResolvedValue(stored({ location: null }));
		show("/setup");
		await user.click(await screen.findByRole("button", { name: "pick" }));
		await user.click(await screen.findByRole("button", { name: "save found" }));
		expect(
			await screen.findByText("sensors step, posted false"),
		).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "done" }));
		expect(
			await screen.findByText("landed on /sources with the saved notice"),
		).toBeInTheDocument();
	});

	it("returns to Sources with the saved notice when opened on what we found", async () => {
		const user = userEvent.setup();
		sources.mockResolvedValue(stored());
		show("/setup?step=found");
		await user.click(await screen.findByRole("button", { name: "save found" }));
		expect(
			await screen.findByText("landed on /sources with the saved notice"),
		).toBeInTheDocument();
	});

	it("opens your sensors without a location, and returns to Sources when done", async () => {
		const user = userEvent.setup();
		sources.mockResolvedValue(
			stored({
				location: null,
				connections: {
					home_assistant: false,
					openweathermap_key: false,
					posted_readings: true,
				},
			}),
		);
		show("/setup?step=sensors");
		expect(
			await screen.findByText("sensors step, posted true"),
		).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "done" }));
		expect(await screen.findByText("landed on /sources")).toBeInTheDocument();
	});
});
