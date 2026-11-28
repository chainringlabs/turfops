// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getSources, testHaConnection, unmapSensor } from "../api/client";
import { ApiError } from "../api/errors";
import type { InputView, SourcesView } from "../types/sources";
import Sources from "./Sources";

vi.mock("../api/client", () => ({
	getSources: vi.fn(),
	testHaConnection: vi.fn(),
	unmapSensor: vi.fn(),
}));
const mocked = vi.mocked(getSources);

function input(
	over: Partial<InputView> & { input: string; label: string },
): InputView {
	return {
		standing: "NoSource",
		sources: [],
		in_use: null,
		estimate_accuracy: null,
		pinned_station_ref: null,
		...over,
	};
}

const moistureNone = ["20", "50", "100"].map((cm) =>
	input({ input: `soil_moisture_${cm}`, label: `Soil moisture, ${cm} cm` }),
);

function view(over: Partial<SourcesView> = {}): SourcesView {
	return {
		location: {
			place_name: "Avondale, PA",
			latitude: 39.82341,
			longitude: -75.78456,
			timezone: "America/New_York",
		},
		inputs: [
			input({
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
					{
						name: "Hourly analysis",
						kind: "Analysis",
						distance_miles: null,
						pinned: false,
					},
				],
				in_use: {
					value: 61.24,
					observed_at: "2026-11-24T19:05:00Z",
					basis: {
						kind: "Sensor",
						source: "Patio temperature",
						distance_miles: null,
						approximate: false,
					},
				},
			}),
			input({
				input: "soil_temp_5",
				label: "Soil temperature, 5 cm",
				standing: "Estimated",
				sources: [
					{
						name: "Soil estimate",
						kind: "Estimate",
						distance_miles: null,
						pinned: false,
					},
				],
				in_use: {
					value: 44,
					observed_at: "2026-11-24T05:00:00Z",
					basis: {
						kind: "Estimate",
						source: "Soil estimate",
						distance_miles: null,
						approximate: false,
					},
				},
				estimate_accuracy: {
					typical_error_f: 1.83,
					fitted_on: "NearestFive",
					crossings: [
						{
							threshold_f: 55,
							direction: "Rising",
							median_days: 2.4,
							p80_days: 12,
							datable: true,
						},
						{
							threshold_f: 70,
							direction: "Falling",
							median_days: 8,
							p80_days: 20,
							datable: false,
						},
					],
				},
			}),
			input({
				input: "soil_moisture_5",
				label: "Soil moisture, 5 cm",
				standing: "Measured",
				sources: [
					{
						name: "PA Avondale 2 N",
						kind: "Station",
						distance_miles: 3.2,
						pinned: true,
					},
				],
			}),
			...moistureNone,
		],
		sources: [
			{
				id: 1,
				name: "PA Avondale 2 N",
				kind: "Station",
				inputs: ["soil_moisture_5"],
				last_success: "2026-11-24T18:35:00Z",
				last_failure: null,
				last_error: null,
				held_from: "2011-01-01",
				held_through: "2026-11-23",
				wanted_from: "2011-01-01",
				attribution: "NOAA U.S. Climate Reference Network (USCRN)",
			},
			{
				id: 2,
				name: "Patio temperature",
				kind: "Sensor",
				inputs: ["air_temp"],
				last_success: "2026-11-24T19:20:00Z",
				last_failure: "2026-11-24T19:00:00Z",
				last_error:
					"Home Assistant reports sensor.patio as unavailable or unknown.",
				held_from: null,
				held_through: null,
				wanted_from: "2026-11-24",
				attribution: "Your sensor",
			},
		],
		forecast_provider: "OpenMeteo",
		forecast: {
			provider: "OpenMeteo",
			fetched_at: "2026-11-24T19:07:00Z",
			last_error: null,
		},
		connections: {
			home_assistant: true,
			openweathermap_key: false,
			posted_readings: false,
		},
		...over,
	};
}

function show() {
	return render(
		<MemoryRouter>
			<Sources />
		</MemoryRouter>,
	);
}

beforeEach(() => {
	vi.useFakeTimers({
		shouldAdvanceTime: true,
		now: new Date("2026-11-24T19:23:00Z"),
	});
});
afterEach(() => {
	vi.useRealTimers();
});

// @spec SETUP-PAGE-001, SETUP-PAGE-009
describe("Sources page inputs", () => {
	it("groups the inputs and shows each basis, what stands behind it and the newest value", async () => {
		mocked.mockResolvedValue(view());
		show();
		const air = await screen.findByRole("region", { name: "Air" });
		expect(within(air).getByText("Air temperature")).toBeInTheDocument();
		expect(
			within(air).getByText("Your sensor · Patio temperature"),
		).toBeInTheDocument();
		expect(within(air).getByText("61.2°F")).toBeInTheDocument();
		expect(
			within(air).getByText("Nov 24, 2:05 PM · 18 min ago"),
		).toBeInTheDocument();
		expect(within(air).getByText(/Then: Hourly analysis/)).toBeInTheDocument();
		expect(
			screen.getByRole("region", { name: "Soil temperature" }),
		).toBeInTheDocument();
		expect(
			screen.getByRole("region", { name: "Soil moisture" }),
		).toBeInTheDocument();
	});

	it("does not link a basis label to the page it is on", async () => {
		mocked.mockResolvedValue(view());
		show();
		await screen.findByText("Your sensor · Patio temperature");
		const label = screen.getByText("Your sensor · Patio temperature");
		expect(label.closest("a")).toBeNull();
	});
});

// @spec SETUP-PAGE-002
it("states the estimate's accuracy in plain words", async () => {
	mocked.mockResolvedValue(view());
	show();
	expect(
		await screen.findByText(
			"Daily values typically within 1.8 °F of a measured station.",
		),
	).toBeInTheDocument();
	expect(
		screen.getByText("Not dated by the estimate: 70 °F falling."),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-003
it("names the inputs nothing measures in one line for their group", async () => {
	mocked.mockResolvedValue(view());
	show();
	const moisture = await screen.findByRole("region", { name: "Soil moisture" });
	expect(
		within(moisture).getByText(
			"Nothing measures soil moisture at 20, 50 or 100 cm here; a probe you map as a sensor would.",
		),
	).toBeInTheDocument();
	expect(within(moisture).queryByText("Soil moisture, 20 cm")).toBeNull();
});

// @spec SETUP-PAGE-004, SETUP-PAGE-010
it("lists the sources failing first with their failure, history and attribution", async () => {
	mocked.mockResolvedValue(view());
	show();
	const table = await screen.findByRole("table", { name: "Sources in use" });
	const rows = within(table).getAllByRole("row").slice(1);
	expect(rows[0]).toHaveTextContent("Patio temperature");
	expect(rows[0]).toHaveTextContent(
		"Home Assistant reports sensor.patio as unavailable or unknown.",
	);
	expect(rows[1]).toHaveTextContent("PA Avondale 2 N");
	expect(rows[1]).toHaveTextContent("Complete");
	expect(rows[1]).toHaveTextContent(
		"NOAA U.S. Climate Reference Network (USCRN)",
	);
});

// @spec SETUP-FAIL-003
describe("a sensor Home Assistant no longer has", () => {
	const gone = (v: SourcesView): SourcesView => ({
		...v,
		sources: v.sources.map((s) =>
			s.name === "Patio temperature"
				? {
						...s,
						last_error:
							"sensor.patio was not found in Home Assistant. Remap this sensor or unmap it.",
						missing_sensor: 7,
					}
				: s,
		),
	});

	it("is shown as not found, with Unmap and a Remap link to the sensors step", async () => {
		const user = (await import("@testing-library/user-event")).default.setup();
		mocked.mockResolvedValueOnce(gone(view()));
		mocked.mockResolvedValueOnce(view());
		vi.mocked(unmapSensor).mockResolvedValue([]);
		show();
		const table = await screen.findByRole("table", { name: "Sources in use" });
		const row = within(table).getByRole("row", { name: /Patio temperature/ });
		expect(row).toHaveTextContent("not found in Home Assistant");
		expect(
			within(row).getByRole("link", { name: "Remap Patio temperature" }),
		).toHaveAttribute("href", "/setup?step=sensors");
		await user.click(
			within(row).getByRole("button", { name: "Unmap Patio temperature" }),
		);
		expect(unmapSensor).toHaveBeenCalledWith(7);
		await vi.waitFor(() => expect(mocked).toHaveBeenCalledTimes(2));
	});

	it("offers neither for any other problem", async () => {
		mocked.mockResolvedValue(view());
		show();
		const table = await screen.findByRole("table", { name: "Sources in use" });
		expect(within(table).queryByRole("button", { name: /Unmap/ })).toBeNull();
		expect(within(table).queryByRole("link", { name: /Remap/ })).toBeNull();
	});
});

// @spec SETUP-CRED-004
it("says a key or token entered in the app is in the database and its backups", async () => {
	mocked.mockResolvedValue(view());
	show();
	const connections = await screen.findByRole("region", {
		name: "Connections",
	});
	expect(
		within(connections).getByText(
			"A Home Assistant token or OpenWeatherMap key entered in TurfOps is stored in its database, so backups of that database contain it.",
		),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-005
it("shows the connections: Home Assistant, the forecast provider and posted readings", async () => {
	mocked.mockResolvedValue(view());
	show();
	const connections = await screen.findByRole("region", {
		name: "Connections",
	});
	expect(within(connections).getByText("Home Assistant")).toBeInTheDocument();
	expect(within(connections).getByText("Connected")).toBeInTheDocument();
	expect(within(connections).getByText("Open-Meteo")).toBeInTheDocument();
	expect(
		within(connections).getByText(/Off — set SENSOR_TOKEN/),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-013
it("shows beside the forecast provider when it was fetched or why it is failing", async () => {
	mocked.mockResolvedValue(view());
	const { unmount } = show();
	let connections = await screen.findByRole("region", { name: "Connections" });
	expect(
		within(connections).getByText(/^Fetched Nov 24, 2:07 PM/),
	).toBeInTheDocument();
	unmount();

	mocked.mockResolvedValue(
		view({
			forecast_provider: "OpenWeatherMap",
			forecast: {
				provider: "OpenWeatherMap",
				fetched_at: null,
				last_error: "OpenWeatherMap needs its API key.",
			},
		}),
	);
	show();
	connections = await screen.findByRole("region", { name: "Connections" });
	expect(within(connections).getByText("OpenWeatherMap")).toBeInTheDocument();
	expect(
		within(connections).getByText("OpenWeatherMap needs its API key."),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-006, SETUP-PAGE-008
it("shows the lawn's place, coordinates, timezone and the timezone data's credit", async () => {
	mocked.mockResolvedValue(view());
	show();
	const location = await screen.findByRole("region", { name: "Location" });
	expect(within(location).getByText("Avondale, PA")).toBeInTheDocument();
	expect(within(location).getByText("39.8234, -75.7846")).toBeInTheDocument();
	expect(within(location).getByText("America/New_York")).toBeInTheDocument();
	expect(
		within(location).getByText(/© OpenStreetMap contributors, ODbL/),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-011
it("says the lawn has no location in place of the inputs", async () => {
	mocked.mockResolvedValue(
		view({ location: null, inputs: moistureNone, sources: [] }),
	);
	show();
	expect(
		await screen.findByText(/The lawn has no location yet/),
	).toBeInTheDocument();
	expect(screen.queryByRole("region", { name: "Soil moisture" })).toBeNull();
});

it("shows a failed load as its owner sentence", async () => {
	mocked.mockRejectedValue(
		new ApiError(503, "The weather record could not be read."),
	);
	show();
	expect(
		await screen.findByText("The weather record could not be read."),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-005, SETUP-PAGE-006, SETUP-PAGE-007
describe("changing what the lawn reads", () => {
	it("opens the location step from Location and what we found from the forecast and each input", async () => {
		mocked.mockResolvedValue(view());
		show();
		const location = await screen.findByRole("region", { name: "Location" });
		expect(
			within(location).getByRole("link", { name: "Change" }),
		).toHaveAttribute("href", "/setup?step=location");
		const connections = screen.getByRole("region", { name: "Connections" });
		expect(
			within(connections).getByRole("link", {
				name: "Change forecast provider",
			}),
		).toHaveAttribute("href", "/setup?step=found");
		const pins = screen.getAllByRole("link", {
			name: /Pin a station|Change pin/,
		});
		expect(pins.length).toBeGreaterThan(0);
		for (const pin of pins)
			expect(pin).toHaveAttribute("href", "/setup?step=found");
		const moisture = screen.getByRole("region", { name: "Soil moisture" });
		expect(
			within(moisture).getByRole("link", { name: "Change pin" }),
		).toBeInTheDocument();
	});

	it("offers setup in place of the inputs while the lawn has no location", async () => {
		mocked.mockResolvedValue(
			view({ location: null, inputs: moistureNone, sources: [] }),
		);
		show();
		expect(
			await screen.findByRole("link", { name: "Set the location" }),
		).toHaveAttribute("href", "/setup?step=location");
	});
});

// @spec SETUP-PLAN-006
it("says the new sources are downloading after setup saved", async () => {
	mocked.mockResolvedValue(view());
	render(
		<MemoryRouter
			initialEntries={[{ pathname: "/sources", state: { saved: true } }]}
		>
			<Sources />
		</MemoryRouter>,
	);
	expect(
		await screen.findByText(
			"Saved. The new sources start downloading now; history can take several minutes.",
		),
	).toBeInTheDocument();
});

// @spec SETUP-PAGE-005
describe("the Home Assistant connection", () => {
	it("is tested in place and changed in your sensors", async () => {
		const user = (await import("@testing-library/user-event")).default.setup();
		vi.mocked(testHaConnection).mockResolvedValue({
			answered: { location_name: "Home", version: "2026.11.1" },
			numeric_sensors: 312,
		});
		mocked.mockResolvedValue(view());
		show();
		const connections = await screen.findByRole("region", {
			name: "Connections",
		});
		expect(
			within(connections).getByRole("link", {
				name: "Change Home Assistant connection",
			}),
		).toHaveAttribute("href", "/setup?step=sensors");
		await user.click(within(connections).getByRole("button", { name: "Test" }));
		expect(testHaConnection).toHaveBeenCalledWith();
		expect(
			await within(connections).findByText(
				"Home (2026.11.1) answered; 312 numeric sensors.",
			),
		).toBeInTheDocument();
	});
});

// @spec SETUP-PAGE-007
it("maps a sensor for an input from its card, or changes the one it has", async () => {
	mocked.mockResolvedValue(view());
	show();
	const air = await screen.findByRole("region", { name: "Air" });
	expect(
		within(air).getByRole("link", { name: "Change sensor" }),
	).toHaveAttribute("href", "/setup?step=sensors");
	const moisture = screen.getByRole("region", { name: "Soil moisture" });
	expect(
		within(moisture).getByRole("link", { name: "Map a sensor" }),
	).toHaveAttribute("href", "/setup?step=sensors");
});
