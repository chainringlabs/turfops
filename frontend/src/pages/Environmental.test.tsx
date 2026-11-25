// @vitest-environment jsdom
import {
	act,
	render as rtlRender,
	screen,
	within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	getEnvironmental,
	getHistorical,
	getProfile,
	getSoilTempForecast,
	getSources,
	refreshEnvironmental,
} from "../api/client";
import {
	AMBIENT_TEMP_GAUGE,
	HUMIDITY_GAUGE,
	SOIL_MOISTURE_GAUGE,
	SOIL_TEMP_GAUGE,
} from "../components/gaugeConfigs";
import {
	ANALYSIS,
	historical,
	inUse,
	SENSOR,
	STATION,
	soilForecast,
	sourcesView,
	summary,
} from "../test/environmentalFixtures";
import type { CurrentBasis, LawnProfile } from "../types";
import Environmental from "./Environmental";

vi.mock("../api/client", () => ({
	getEnvironmental: vi.fn(),
	getHistorical: vi.fn(),
	// A lawn with a location unless a test says otherwise.
	getProfile: vi.fn(() =>
		Promise.resolve({ location: { latitude: 39.86, longitude: -75.79 } }),
	),
	getSoilTempForecast: vi.fn(),
	getSources: vi.fn(),
	refreshEnvironmental: vi.fn(),
}));

// jsdom has no layout; the charts' own behaviour is tested in PredictionChart.test.tsx.
vi.mock("recharts", () => {
	const none = () => null;
	return {
		Area: none,
		AreaChart: none,
		CartesianGrid: none,
		ComposedChart: none,
		Line: none,
		ReferenceLine: none,
		ResponsiveContainer: none,
		Tooltip: none,
		XAxis: none,
		YAxis: none,
	};
});

/** The page inside a router: basis labels link to the Sources page. */
function render(ui: React.ReactElement) {
	return rtlRender(<MemoryRouter>{ui}</MemoryRouter>);
}

let hidden = false;

beforeEach(() => {
	hidden = false;
	Object.defineProperty(document, "hidden", {
		configurable: true,
		get: () => hidden,
	});
	vi.mocked(getEnvironmental).mockResolvedValue(summary());
	vi.mocked(getSources).mockResolvedValue(sourcesView());
	vi.mocked(getHistorical).mockResolvedValue(historical());
	vi.mocked(getSoilTempForecast).mockResolvedValue(soilForecast());
});

afterEach(() => {
	vi.useRealTimers();
	vi.clearAllMocks();
});

/** A gauge's header text (its label is a span; a trend chart's is a div). */
function gaugeText(label: string): string {
	const gauge = screen.getAllByText(label).find((el) => el.tagName === "SPAN");
	return gauge?.parentElement?.textContent ?? "";
}

/** The card holding a gauge: its header, bar and basis label. */
function gaugeCard(label: string): HTMLElement {
	const gauge = screen.getAllByText(label).find((el) => el.tagName === "SPAN");
	const card = gauge?.closest("[data-gauge-card]");
	if (!card) throw new Error(`no card for ${label}`);
	return card as HTMLElement;
}

/** The fixture's bases with one input's standing changed. */
function withStanding(
	input: string,
	standing: CurrentBasis["standing"],
): CurrentBasis[] {
	return (summary().current_bases ?? []).map((entry) =>
		entry.input === input
			? { input, standing, basis: null, observed_at: null }
			: entry,
	);
}

describe("Environmental page loading", () => {
	// @spec WX-VIEW-PAGE-001
	it("loads conditions and the soil outlook together and polls every 30 s only while visible", async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		render(<Environmental />);
		await screen.findByText("Soil Profile by Depth");
		expect(getEnvironmental).toHaveBeenCalledTimes(1);
		expect(getSoilTempForecast).toHaveBeenCalledTimes(1);

		await act(() => vi.advanceTimersByTimeAsync(30_000));
		expect(getEnvironmental).toHaveBeenCalledTimes(2);
		expect(getSoilTempForecast).toHaveBeenCalledTimes(2);

		hidden = true;
		act(() => {
			document.dispatchEvent(new Event("visibilitychange"));
		});
		await act(() => vi.advanceTimersByTimeAsync(120_000));
		expect(getEnvironmental).toHaveBeenCalledTimes(2);

		hidden = false;
		await act(async () => {
			document.dispatchEvent(new Event("visibilitychange"));
		});
		expect(getEnvironmental).toHaveBeenCalledTimes(3);
		await act(() => vi.advanceTimersByTimeAsync(30_000));
		expect(getEnvironmental).toHaveBeenCalledTimes(4);
	});

	// @spec WX-VIEW-PAGE-002
	it("Refresh Now forces a refresh and shows its result", async () => {
		const fresh = summary();
		if (fresh.current) fresh.current.soil_temp_10_f = 70.3;
		vi.mocked(refreshEnvironmental).mockResolvedValue(fresh);
		render(<Environmental />);
		await userEvent.click(
			await screen.findByRole("button", { name: "Refresh Now" }),
		);
		expect(refreshEnvironmental).toHaveBeenCalledTimes(1);
		expect(gaugeText(SOIL_TEMP_GAUGE.label)).toContain("70.3");
	});

	// @spec WX-OUTLOOK-UI-003
	it("hides the soil outlook section when the outlook is unavailable", async () => {
		vi.mocked(getSoilTempForecast).mockRejectedValue(new Error("503"));
		render(<Environmental />);
		await screen.findByText("Soil Profile by Depth");
		expect(
			screen.queryByText("Soil Temperature Forecast"),
		).not.toBeInTheDocument();
		expect(gaugeText(SOIL_TEMP_GAUGE.label)).toContain("62.4");
	});
});

// @spec SETUP-FLOW-001, UI-ERR-012
describe("Environmental page without a location", () => {
	it("shows the setup notice in place of the weather", async () => {
		vi.mocked(getProfile).mockResolvedValueOnce({
			location: null,
		} as unknown as LawnProfile);
		render(<Environmental />);
		expect(
			await screen.findByRole("heading", {
				name: "Tell TurfOps where your lawn is",
			}),
		).toBeInTheDocument();
		expect(screen.queryByText("Refresh Now")).toBeNull();
	});
});

describe("Environmental page content", () => {
	// @spec WX-VIEW-PAGE-003
	it("shows the header, four gauges, 7-day cards, soil profile and forecast cards", async () => {
		render(<Environmental />);
		expect(await screen.findByText(/Last updated:/)).toHaveTextContent(
			"Soil temp trend: Rising",
		);

		expect(gaugeText(SOIL_TEMP_GAUGE.label)).toContain("62.4 °F");
		expect(gaugeText(AMBIENT_TEMP_GAUGE.label)).toContain("71.5 °F");
		expect(gaugeText(HUMIDITY_GAUGE.label)).toContain("55.5 %");
		expect(gaugeText(SOIL_MOISTURE_GAUGE.label)).toContain("25.0 %");
		const ranges = [
			SOIL_TEMP_GAUGE,
			AMBIENT_TEMP_GAUGE,
			HUMIDITY_GAUGE,
			SOIL_MOISTURE_GAUGE,
		].map((g) => [g.min, g.max, g.thresholds.warn, g.thresholds.critical]);
		expect(ranges).toEqual([
			[30, 100, 75, 85],
			[0, 110, 85, 95],
			[0, 100, 80, 90],
			[0, 50, 40, 45],
		]);

		for (const card of [
			"7-Day Soil Temp Avg",
			"7-Day Ambient Avg",
			"7-Day Humidity Avg",
			"7-Day Precipitation",
		]) {
			expect(screen.getByText(card)).toBeInTheDocument();
		}

		const profile = screen.getByText("Soil Profile by Depth")
			.nextElementSibling as HTMLElement;
		const depths = within(profile)
			.getAllByRole("row")
			.slice(1)
			.map((row) => row.firstElementChild?.textContent);
		expect(depths).toEqual(["5 cm", "10 cm", "20 cm", "50 cm", "100 cm"]);
		expect(within(profile).getByText("61.2 °F")).toBeInTheDocument();

		const cards = screen.getByText("Forecast from OpenWeatherMap")
			.nextElementSibling as HTMLElement;
		expect(cards.children).toHaveLength(2);
		const first = cards.children[0] as HTMLElement;
		expect(first).toHaveTextContent("2026-10-12");
		expect(first).toHaveTextContent("68° / 48°");
		expect(first).toHaveTextContent("Humidity: 64%");
		expect(first).toHaveTextContent(/Precip: 1\.00 in \(40%\)/);
	});

	// @spec WX-VIEW-PAGE-003
	it("shows -- for a gauge with no reading", async () => {
		const data = summary();
		if (data.current) data.current.ambient_temp_f = null;
		vi.mocked(getEnvironmental).mockResolvedValue(data);
		render(<Environmental />);
		await screen.findByText("Soil Profile by Depth");
		expect(gaugeText(AMBIENT_TEMP_GAUGE.label)).toContain("--");
	});

	// @spec WX-VIEW-PAGE-003, WX-VIEW-BASIS-001
	it("labels each gauge with the basis of its value", async () => {
		render(<Environmental />);
		await screen.findByText("Soil Profile by Depth");
		const soil = gaugeCard(SOIL_TEMP_GAUGE.label);
		expect(soil).toHaveTextContent("Measured · PA Avondale 2 N, 2 mi");
		expect(within(soil).getByRole("link")).toHaveAttribute("href", "/sources");
		expect(gaugeCard(AMBIENT_TEMP_GAUGE.label)).toHaveTextContent(
			"Your sensor · Patio temperature",
		);
	});

	// @spec WX-VIEW-BASIS-004
	it("says No source for this location in place of a gauge whose input has none", async () => {
		const bases = withStanding("soil_moisture_10", "NoSource");
		const data = summary({ current_bases: bases });
		if (data.current) data.current.soil_moisture_10 = null;
		vi.mocked(getEnvironmental).mockResolvedValue(data);
		render(<Environmental />);
		await screen.findByText("Soil Profile by Depth");
		const moisture = gaugeCard(SOIL_MOISTURE_GAUGE.label);
		expect(moisture).toHaveTextContent("No source for this location");
		expect(moisture).not.toHaveTextContent("--");
		expect(within(moisture).getByRole("link")).toHaveAttribute(
			"href",
			"/sources",
		);
	});

	// @spec WX-VIEW-BASIS-004
	it("shows -- and no label for a gauge whose source has no current value", async () => {
		const data = summary({
			current_bases: withStanding("air_temp", "NoCurrentValue"),
		});
		if (data.current) data.current.ambient_temp_f = null;
		vi.mocked(getEnvironmental).mockResolvedValue(data);
		render(<Environmental />);
		await screen.findByText("Soil Profile by Depth");
		const air = gaugeCard(AMBIENT_TEMP_GAUGE.label);
		expect(air).toHaveTextContent("--");
		expect(air).not.toHaveTextContent("Your sensor");
		expect(air).not.toHaveTextContent("No source");
	});

	// @spec WX-VIEW-PAGE-003, WX-VIEW-BASIS-001
	it("labels each 7-day card with its basis, mixed where the week was", async () => {
		render(<Environmental />);
		const ambient = (await screen.findByText("7-Day Ambient Avg"))
			.parentElement as HTMLElement;
		expect(ambient).toHaveTextContent(
			"Your sensor · Patio temperature · mixed",
		);
		const soil = screen.getByText("7-Day Soil Temp Avg")
			.parentElement as HTMLElement;
		expect(soil).toHaveTextContent("Measured · PA Avondale 2 N, 2 mi");
	});

	// @spec WX-VIEW-PAGE-003, WX-VIEW-BASIS-001
	it("lists only the depths that have a source, each value with its basis", async () => {
		const bases: CurrentBasis[] = [
			inUse("air_temp", SENSOR),
			inUse("soil_temp_5", STATION),
			inUse("soil_temp_10", STATION),
			{
				input: "soil_temp_20",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
			{
				input: "soil_temp_50",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
			{
				input: "soil_temp_100",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
			inUse("soil_moisture_5", STATION),
			{
				input: "soil_moisture_10",
				standing: "NoCurrentValue",
				basis: null,
				observed_at: null,
			},
			{
				input: "soil_moisture_20",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
			{
				input: "soil_moisture_50",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
			{
				input: "soil_moisture_100",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
		];
		const data = summary({ current_bases: bases });
		if (data.current) data.current.soil_moisture_10 = null;
		vi.mocked(getEnvironmental).mockResolvedValue(data);
		render(<Environmental />);
		const profile = (await screen.findByText("Soil Profile by Depth"))
			.nextElementSibling as HTMLElement;
		const rows = within(profile).getAllByRole("row").slice(1);
		expect(rows.map((row) => row.firstElementChild?.textContent)).toEqual([
			"5 cm",
			"10 cm",
		]);
		expect(rows[0]).toHaveTextContent("Measured · PA Avondale 2 N, 2 mi");
		expect(rows[1]).toHaveTextContent("--");
	});

	// @spec WX-VIEW-PAGE-003
	it("heads the forecast with its provider and shows the first 7 days until asked for more", async () => {
		const base = summary();
		const day = base.forecast?.daily_summary[0];
		if (!base.forecast || !day) throw new Error("fixture has a forecast");
		base.forecast.daily_summary = Array.from({ length: 10 }, (_, i) => ({
			...day,
			date: `2026-10-${String(12 + i).padStart(2, "0")}`,
		}));
		vi.mocked(getEnvironmental).mockResolvedValue(base);
		render(<Environmental />);
		const cards = (await screen.findByText("Forecast from OpenWeatherMap"))
			.nextElementSibling as HTMLElement;
		expect(cards.children).toHaveLength(7);
		await userEvent.click(
			screen.getByRole("button", { name: "Show 3 more days" }),
		);
		expect(cards.children).toHaveLength(10);
	});

	// @spec WX-VIEW-PAGE-012
	it("credits every source the page drew on, and no other", async () => {
		render(<Environmental />);
		const credits = await screen.findByRole("region", { name: "Sources" });
		expect(credits).toHaveTextContent(
			"PA Avondale 2 N — NOAA U.S. Climate Reference Network (USCRN)",
		);
		expect(credits).toHaveTextContent(
			"Hourly analysis — Weather data by Open-Meteo.com (CC BY 4.0)",
		);
		expect(credits).toHaveTextContent("Patio temperature — Your sensor");
		expect(credits).not.toHaveTextContent("NRCC");
	});

	// @spec WX-VIEW-PAGE-004
	it("shows precipitation in inches converted from the API's millimetres", async () => {
		render(<Environmental />);
		const card = (await screen.findByText("7-Day Precipitation"))
			.parentElement as HTMLElement;
		expect(card).toHaveTextContent("1.00 in");
		const cards = screen.getByText("Forecast from OpenWeatherMap")
			.nextElementSibling as HTMLElement;
		expect(cards.children[0]).toHaveTextContent("Precip: 1.00 in");
		expect(cards.children[1]).toHaveTextContent("Precip: 0.00 in");
	});

	// @spec WX-OUTLOOK-UI-001
	it("shows crossing cards and the model line under the prediction chart", async () => {
		render(<Environmental />);
		const name = await screen.findByText("Active Growth Peak");
		const card = name.parentElement?.parentElement as HTMLElement;
		expect(within(card).getByText("Medium")).toBeInTheDocument();
		expect(card).toHaveTextContent("↓ 65°F");
		expect(card).toHaveTextContent("~3 days (Oct 14)");
		expect(screen.getByText(/^Model:/)).toHaveTextContent(
			"Model: R²=0.82, lag=2d, quality=Good",
		);
		// The soil the outlook starts from carries its basis label.
		const section = screen.getByText("Soil Temperature Forecast")
			.parentElement as HTMLElement;
		expect(section).toHaveTextContent(
			"Starts from: ● Measured · PA Avondale 2 N, 2 mi",
		);
	});

	// @spec WX-OUTLOOK-UI-001
	it("says an estimated outlook is estimated for the location, with no fit", async () => {
		vi.mocked(getSoilTempForecast).mockResolvedValue(
			soilForecast({
				method: "Estimated",
				basis: ANALYSIS,
				model_info: null,
				estimate_info: {
					stations: ["Station A", "Station B"],
					training_days: 4200,
					daily_error_f: 1.8,
				},
			}),
		);
		render(<Environmental />);
		await screen.findByText("Active Growth Peak");
		expect(screen.queryByText(/^Model:/)).toBeNull();
		expect(
			screen.getByText(
				"Estimated for your location (soil estimate, typically within 1.8 °F a day)",
			),
		).toBeInTheDocument();
	});
});
