// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { applySetup, getPreview, getStations } from "../../api/client";
import type { ChosenPoint, Preview } from "../../types/setup";
import type { SourcesView } from "../../types/sources";
import FoundStep from "./FoundStep";

vi.mock("../../api/client", () => ({
	applySetup: vi.fn(),
	getPreview: vi.fn(),
	getStations: vi.fn(),
}));
const apply = vi.mocked(applySetup);
const preview = vi.mocked(getPreview);
const stations = vi.mocked(getStations);

const station = (name: string, miles: number) => ({
	name,
	kind: "Station" as const,
	distance_miles: miles,
	pinned: false,
});
const estimate = {
	name: "Soil temperature estimate",
	kind: "Estimate" as const,
	distance_miles: null,
	pinned: false,
};

const plan: Preview = {
	latitude: 39.8234,
	longitude: -75.7846,
	timezone: "America/New_York",
	elevation_ft: 310,
	nearest_soil_station: null,
	inputs: [
		{
			input: "air_temp",
			label: "Air temperature",
			standing: "Measured",
			sources: [
				{
					name: "Patio temperature",
					kind: "Sensor",
					distance_miles: null,
					pinned: false,
				},
			],
		},
		{
			input: "rain",
			label: "Rain",
			standing: "Estimated",
			sources: [
				{
					name: "Hourly analysis",
					kind: "Analysis",
					distance_miles: null,
					pinned: false,
				},
			],
		},
		{
			input: "soil_temp_10",
			label: "Soil temperature, 10 cm",
			standing: "Measured",
			sources: [station("PA Avondale 2 N, PA (USCRN)", 3.2), estimate],
		},
		{
			input: "soil_moisture_50",
			label: "Soil moisture, 50 cm",
			standing: "NoSource",
			sources: [],
		},
	],
};

const chosen: ChosenPoint = {
	latitude: 39.8234,
	longitude: -75.7846,
	place_name: "Avondale, Pennsylvania",
	preview: plan,
};

/** Where the step sent the owner. */
export function Where() {
	const at = useLocation();
	return (
		<div>
			at {at.pathname}
			{(at.state as { saved?: boolean } | null)?.saved ? " saved" : ""}
		</div>
	);
}

function show(over: Partial<Parameters<typeof FoundStep>[0]> = {}) {
	return render(
		<MemoryRouter initialEntries={["/setup?step=found"]}>
			<Routes>
				<Route
					path="/setup"
					element={
						<FoundStep
							chosen={chosen}
							pins={{}}
							provider="OpenMeteo"
							keyStored={false}
							onBack={vi.fn()}
							{...over}
						/>
					}
				/>
				<Route path="/sources" element={<Where />} />
			</Routes>
		</MemoryRouter>,
	);
}

beforeEach(() => {
	apply.mockReset();
	preview.mockReset();
	stations.mockReset();
	apply.mockResolvedValue({} as SourcesView);
});

// @spec SETUP-PLAN-001
it("shows the plan the point would get, a row per input, and says it in words", () => {
	show();
	const table = screen.getByRole("table", { name: "What this location gets" });
	const rows = within(table).getAllByRole("row").slice(1);
	expect(rows).toHaveLength(4);
	expect(rows[2]).toHaveTextContent("Soil temperature, 10 cm");
	expect(rows[2]).toHaveTextContent("PA Avondale 2 N, PA (USCRN)");
	expect(rows[2]).toHaveTextContent("3 mi");
	expect(rows[2]).toHaveTextContent("Then: Soil temperature estimate");
	expect(rows[3]).toHaveTextContent("No source here");
	expect(
		screen.getByText("Measured: air temperature, soil temperature at 10 cm."),
	).toBeInTheDocument();
	expect(
		screen.getByText("No source here: soil moisture at 50 cm."),
	).toBeInTheDocument();
});

// @spec SETUP-PLAN-002, SETUP-PLAN-003
describe("pinning a station", () => {
	it("lists the stations for that input in place and shows the plan again with the pin", async () => {
		const user = userEvent.setup();
		stations.mockResolvedValue([
			{
				station_ref: 9,
				network: "SCAN",
				name: "Rock Springs",
				state: "PA",
				distance_miles: 41.2,
				elevation_difference_ft: -60,
				first_year: 2001,
				last_year: null,
			},
		]);
		const pinnedPlan: Preview = {
			...plan,
			inputs: plan.inputs.map((i) =>
				i.input === "soil_temp_10"
					? {
							...i,
							sources: [
								{ ...station("Rock Springs, PA (SCAN)", 41.2), pinned: true },
								estimate,
							],
						}
					: i,
			),
		};
		preview.mockResolvedValue(pinnedPlan);
		show();
		const row = screen.getByRole("row", { name: /Soil temperature, 10 cm/ });
		await user.click(
			within(row).getByRole("button", { name: "Pin a station" }),
		);
		expect(stations).toHaveBeenCalledWith("soil_temp_10", 39.8234, -75.7846);
		await user.click(
			await screen.findByRole("button", {
				name: "SCAN · Rock Springs, PA · 41 mi · 60 ft lower · 2001–present",
			}),
		);
		expect(preview).toHaveBeenCalledWith(39.8234, -75.7846, {
			soil_temp_10: 9,
		});
		const again = await screen.findByRole("row", {
			name: /Rock Springs, PA \(SCAN\)/,
		});
		expect(within(again).getByText(/Pinned/)).toBeInTheDocument();
		expect(
			within(again).getByRole("button", { name: "Unpin" }),
		).toBeInTheDocument();
	});

	it("unpins, showing the plan again without the pin", async () => {
		const user = userEvent.setup();
		preview.mockResolvedValue(plan);
		const pinned = {
			...chosen,
			preview: {
				...plan,
				inputs: plan.inputs.map((i) =>
					i.input === "soil_temp_10"
						? {
								...i,
								sources: [
									{ ...station("Rock Springs, PA (SCAN)", 41.2), pinned: true },
								],
							}
						: i,
				),
			},
		};
		show({ chosen: pinned, pins: { soil_temp_10: 9 } });
		await user.click(screen.getByRole("button", { name: "Unpin" }));
		expect(preview).toHaveBeenCalledWith(39.8234, -75.7846, {});
	});
});

// @spec SETUP-PLAN-004
describe("the forecast provider", () => {
	it("offers three providers with Open-Meteo chosen, and asks for a key for OpenWeatherMap when none is stored", async () => {
		const user = userEvent.setup();
		show();
		const choice = screen.getByLabelText("Forecast provider");
		expect(choice).toHaveValue("OpenMeteo");
		expect(
			within(choice)
				.getAllByRole("option")
				.map((o) => o.textContent),
		).toEqual(["Open-Meteo", "National Weather Service", "OpenWeatherMap"]);
		expect(screen.queryByLabelText("OpenWeatherMap key")).toBeNull();
		await user.selectOptions(choice, "OpenWeatherMap");
		expect(screen.getByLabelText("OpenWeatherMap key")).toHaveAttribute(
			"type",
			"password",
		);
		await user.click(screen.getByRole("button", { name: "Save" }));
		expect(apply).not.toHaveBeenCalled();
		expect(
			screen.getByText("OpenWeatherMap needs its key."),
		).toBeInTheDocument();
	});

	it("says a key is stored and offers to replace it", async () => {
		const user = userEvent.setup();
		show({ provider: "OpenWeatherMap", keyStored: true });
		expect(screen.getByText(/A key is stored/)).toBeInTheDocument();
		expect(screen.queryByLabelText("OpenWeatherMap key")).toBeNull();
		await user.click(screen.getByRole("button", { name: "Replace the key" }));
		expect(screen.getByLabelText("OpenWeatherMap key")).toBeInTheDocument();
	});
});

// @spec SETUP-PLAN-005, SETUP-PLAN-006, SETUP-FLOW-002
it("saves the location, the pins and the provider in one request, then returns to the Sources page", async () => {
	const user = userEvent.setup();
	show({
		pins: { soil_temp_10: 9 },
		provider: "OpenWeatherMap",
		keyStored: true,
	});
	await user.click(screen.getByRole("button", { name: "Save" }));
	expect(apply).toHaveBeenCalledTimes(1);
	expect(apply).toHaveBeenCalledWith({
		location: {
			latitude: 39.8234,
			longitude: -75.7846,
			place_name: "Avondale, Pennsylvania",
		},
		pins: [{ input: "soil_temp_10", station_ref: 9 }],
		forecast_provider: { provider: "OpenWeatherMap" },
	});
	expect(await screen.findByText("at /sources saved")).toBeInTheDocument();
});

it("shows a failed save as its owner sentence and stays on the step", async () => {
	const user = userEvent.setup();
	const { ApiError } = await import("../../api/errors");
	apply.mockRejectedValue(
		new ApiError(400, "That station does not report soil temperature, 10 cm."),
	);
	show();
	await user.click(screen.getByRole("button", { name: "Save" }));
	expect(
		await screen.findByText(
			"That station does not report soil temperature, 10 cm.",
		),
	).toBeInTheDocument();
	expect(screen.queryByText(/at \/sources/)).toBeNull();
});
