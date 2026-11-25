// @vitest-environment jsdom
import { act, render, screen, within } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
	getDashboard,
	getDiseaseRisk,
	getGdd,
	getNitrogenBudget,
	getSoilTempForecast,
	getTimingWindows,
} from "../api/client";
import type { DashboardResponse, Recommendation } from "../types";
import Dashboard from "./Dashboard";

vi.mock("../api/client", () => ({
	getDashboard: vi.fn(),
	getDiseaseRisk: vi.fn(),
	getGdd: vi.fn(),
	getNitrogenBudget: vi.fn(),
	getSoilTempForecast: vi.fn(),
	getTimingWindows: vi.fn(),
}));
// The widgets belong to other features; here they only mark where they render.
vi.mock("../components/Gauge", () => ({
	default: () => <div data-testid="gauge" />,
}));
vi.mock("../components/GddWidget", () => ({
	default: () => <div data-testid="widget">GDD widget</div>,
}));
vi.mock("../components/disease/DiseaseRiskWidget", () => ({
	default: () => <div data-testid="widget">Disease widget</div>,
}));
vi.mock("../components/timing/TimingWidget", () => ({
	default: () => <div data-testid="widget">Timing widget</div>,
}));
vi.mock("../components/inventory/ShoppingListPanel", () => ({
	default: () => <div data-testid="widget">Shopping widget</div>,
}));
vi.mock("../components/NitrogenBudgetWidget", () => ({
	default: () => <div data-testid="widget">Nitrogen widget</div>,
}));
vi.mock("../components/SoilTempForecastWidget", () => ({
	default: () => <div data-testid="widget">Soil forecast widget</div>,
}));

function alert(
	id: string,
	severity: Recommendation["severity"],
): Recommendation {
	return {
		id,
		category: "DiseasePressure",
		severity,
		title: `Alert ${id}`,
		description: `About ${id}`,
		explanation: "",
		data_points: [],
		suggested_action: null,
		needs: [],
		created_at: "2026-10-11T12:00:00Z",
		dismissed: false,
		addressed: false,
	};
}

const dashboard = {
	profile: { name: "Front Lawn", grass_type: "TallFescue", usda_zone: "7a" },
	environmental: {
		current: null,
		soil_temp_7day_avg_f: null,
		ambient_temp_7day_avg_f: null,
		humidity_7day_avg: null,
		precipitation_7day_total_mm: null,
		soil_temp_trend: "Unknown",
		last_updated: "2026-10-11T12:00:00Z",
		soil_observed_at: "2026-10-05T12:00:00Z",
		soil_stale: true,
		forecast: null,
		gdd_base50_ytd: null,
		current_bases: [
			{
				input: "soil_temp_10",
				standing: "InUse",
				basis: {
					kind: "Station",
					source: "PA Avondale 2 N",
					distance_miles: 2.1,
					approximate: false,
				},
				observed_at: "2026-10-05T12:00:00Z",
			},
			{
				input: "soil_moisture_10",
				standing: "NoSource",
				basis: null,
				observed_at: null,
			},
		],
		week_bases: {},
	},
	recommendations: [
		{
			...alert("brown_patch", "Critical"),
			inventory: { state: "NotOnHand", matches: [], missing: ["fungicide"] },
		},
		alert("dollar_spot", "Warning"),
		alert("aeration", "Advisory"),
	],
	recent_applications: [1, 2, 3, 4, 5].map((id) => ({
		id,
		application_type: "Mowing",
		product_name: `Entry ${id}`,
		application_date: `2026-10-0${id}`,
	})),
	sources: [
		{
			id: 1,
			name: "PA Avondale 2 N",
			kind: "Station",
			ok: true,
			last_success: "2026-10-11T11:35:00Z",
			last_failure: null,
			last_error: null,
			inputs: ["soil_temp_10"],
			held_from: "2011-01-01",
			held_through: "2026-10-11",
			wanted_from: "2011-01-01",
		},
		{
			id: 2,
			name: "Hourly analysis",
			kind: "Analysis",
			ok: false,
			last_success: "2026-10-11T09:35:00Z",
			last_failure: "2026-10-11T11:35:00Z",
			last_error: "Open-Meteo did not answer.",
			inputs: ["air_temp"],
			held_from: "2026-07-13",
			held_through: "2026-10-11",
			wanted_from: "2026-07-13",
		},
	],
	forecast_status: {
		provider: "NationalWeatherService",
		fetched_at: "2026-10-11T11:07:00Z",
		last_error: null,
	},
} as unknown as DashboardResponse;

const widgetFetchers = [
	getGdd,
	getNitrogenBudget,
	getSoilTempForecast,
	getDiseaseRisk,
	getTimingWindows,
].map((f) => vi.mocked(f as (...a: unknown[]) => Promise<unknown>));

let hidden = false;

beforeEach(() => {
	vi.mocked(getDashboard).mockReset().mockResolvedValue(dashboard);
	for (const f of widgetFetchers) f.mockReset().mockResolvedValue({});
	hidden = false;
	Object.defineProperty(document, "hidden", {
		configurable: true,
		get: () => hidden,
	});
});

afterEach(() => {
	vi.useRealTimers();
});

async function renderDashboard() {
	render(
		<MemoryRouter>
			<Dashboard />
		</MemoryRouter>,
	);
	await screen.findByRole("heading", { name: "Front Lawn" });
}

function before(a: Element, b: Element) {
	return Boolean(
		a.compareDocumentPosition(b) & Node.DOCUMENT_POSITION_FOLLOWING,
	);
}

describe("Dashboard", () => {
	// @spec FEED-UI-008
	it("shows the endpoint's alerts as read-only cards", async () => {
		await renderDashboard();
		const column = screen.getByRole("heading", { name: "Active Alerts" })
			.parentElement as HTMLElement;
		const inColumn = within(column);
		expect(
			inColumn.getAllByText(/^Alert /).map((el) => el.textContent),
		).toEqual(["Alert brown_patch", "Alert dollar_spot", "Alert aeration"]);
		expect(inColumn.getByText("! Critical")).toBeInTheDocument();
		expect(inColumn.getAllByText("Disease Pressure")).toHaveLength(3);
		expect(inColumn.getByText("Need to buy")).toBeInTheDocument();
		expect(inColumn.getByText("About brown_patch")).toBeInTheDocument();
		expect(inColumn.queryAllByRole("button")).toHaveLength(0);
	});

	// @spec FEED-UI-009
	it("lays out header, stale banner, connections, gauges, widgets, then alerts beside recent applications", async () => {
		await renderDashboard();
		const header = screen.getByRole("heading", { name: "Front Lawn" });
		expect(header.parentElement).toHaveTextContent("Tall Fescue");
		expect(header.parentElement).toHaveTextContent("Zone 7a");
		expect(header.parentElement).toHaveTextContent("Updated");
		// A notice, not an error: the delay is the station network's.
		expect(screen.queryByRole("alert")).toBeNull();
		const banner = screen.getByRole("status");
		expect(banner).toHaveTextContent("hasn't published new soil readings");
		expect(banner).toHaveTextContent("nothing you need to do");
		// @spec FEED-UI-014: the failing source is named, the line links to Sources.
		const connections = screen.getByText("Not current: Hourly analysis");
		expect(
			within(connections.parentElement as HTMLElement).getByRole("link", {
				name: "Sources",
			}),
		).toHaveAttribute("href", "/sources");
		expect(screen.queryByText("Weather lake")).toBeNull();
		const gauges = screen.getAllByTestId("gauge");
		expect(gauges).toHaveLength(3);
		// The 10 cm soil gauge says what it rests on; moisture has no source here.
		expect(gauges[0].closest("[data-gauge-card]")).toHaveTextContent(
			"Measured · PA Avondale 2 N, 2 mi",
		);
		expect(screen.getByText("No source for this location")).toBeInTheDocument();
		const widgets = screen.getAllByTestId("widget");
		const alerts = screen.getByRole("heading", { name: "Active Alerts" });
		const recent = screen.getByRole("heading", { name: "Recent Applications" });
		const sequence = [
			header,
			banner,
			connections,
			gauges[0],
			widgets[0],
			alerts,
			recent,
		];
		for (let i = 1; i < sequence.length; i++) {
			expect(before(sequence[i - 1], sequence[i])).toBe(true);
		}
		const table = within(recent.parentElement as HTMLElement).getByRole(
			"table",
		);
		expect(within(table).getAllByRole("row")).toHaveLength(1 + 5);
	});

	// @spec FEED-UI-010
	it("polls every 30 s only while the tab is visible and re-fetches on return", async () => {
		vi.useFakeTimers();
		render(
			<MemoryRouter>
				<Dashboard />
			</MemoryRouter>,
		);
		const flush = () => act(() => vi.advanceTimersByTimeAsync(0));
		const wait = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));
		const calls = () => vi.mocked(getDashboard).mock.calls.length;
		await flush();
		expect(calls()).toBe(1);
		await wait(30_000);
		expect(calls()).toBe(2);

		hidden = true;
		document.dispatchEvent(new Event("visibilitychange"));
		await wait(90_000);
		expect(calls()).toBe(2);

		hidden = false;
		document.dispatchEvent(new Event("visibilitychange"));
		await flush();
		expect(calls()).toBe(3);
		await wait(30_000);
		expect(calls()).toBe(4);
	});

	// @spec FEED-UI-011
	it("hides a widget whose first fetch fails and renders the rest", async () => {
		vi.mocked(getGdd).mockRejectedValue(new Error("lake down"));
		await renderDashboard();
		expect(screen.queryByText("GDD widget")).not.toBeInTheDocument();
		for (const name of ["Disease widget", "Timing widget", "Nitrogen widget"]) {
			expect(screen.getByText(name)).toBeInTheDocument();
		}
		expect(screen.getByText("Alert brown_patch")).toBeInTheDocument();
		// Three gauges and the soil moisture card that has no source here.
		expect(screen.getAllByTestId("gauge")).toHaveLength(3);
	});
});
