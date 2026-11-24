// @vitest-environment jsdom
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getSources } from "../../api/client";
import type { SourcesView, SourceView } from "../../types/sources";
import HistoryStep from "./HistoryStep";

vi.mock("../../api/client", () => ({ getSources: vi.fn() }));
const sources = vi.mocked(getSources);

const source = (over: Partial<SourceView>): SourceView => ({
	id: 1,
	name: "Avondale 2 N",
	kind: "Station",
	inputs: ["soil_temp_10"],
	last_success: null,
	last_failure: null,
	last_error: null,
	held_from: "2011-01-01",
	held_through: "2026-11-23",
	wanted_from: "2011-01-01",
	attribution: "NOAA",
	...over,
});

function view(list: SourceView[]): SourcesView {
	return {
		location: {
			place_name: "Avondale, PA",
			latitude: 39.82,
			longitude: -75.78,
			timezone: "America/New_York",
		},
		inputs: [],
		sources: list,
		forecast_provider: "OpenMeteo",
		connections: {
			home_assistant: true,
			openweathermap_key: false,
			posted_readings: false,
		},
	};
}

function show() {
	return render(
		<MemoryRouter>
			<HistoryStep />
		</MemoryRouter>,
	);
}

beforeEach(() => sources.mockReset());

// @spec SETUP-FLOW-005
describe("loading history", () => {
	it("shows each source's years held against wanted, leaving out sensors", async () => {
		sources.mockResolvedValue(
			view([
				source({ id: 1, name: "Avondale 2 N" }),
				source({
					id: 2,
					name: "PRISM daily grid",
					kind: "Grid",
					held_from: "2019-01-01",
				}),
				source({ id: 3, name: "Patio temperature", kind: "Sensor" }),
			]),
		);
		show();
		const list = await screen.findByRole("list", { name: "History by source" });
		const rows = within(list).getAllByRole("listitem");
		expect(rows).toHaveLength(2);
		expect(rows[0]).toHaveTextContent("Avondale 2 N");
		expect(rows[0]).toHaveTextContent("Complete");
		expect(rows[1]).toHaveTextContent(/Loading: 2019–\d{4} of 2011–\d{4}/);
	});

	it("says what works now and what waits while history is loading", async () => {
		sources.mockResolvedValue(view([source({ held_from: "2019-01-01" })]));
		show();
		expect(
			await screen.findByText(
				"Works now: current conditions, the forecast, and disease risk for recent days.",
			),
		).toBeInTheDocument();
		expect(
			screen.getByText(
				"Waits for the history: typical dates on the Timing page, and the seasonal plan.",
			),
		).toBeInTheDocument();
	});

	it("says when everything has its history", async () => {
		sources.mockResolvedValue(view([source({})]));
		show();
		expect(
			await screen.findByText("Everything has its history."),
		).toBeInTheDocument();
		expect(screen.queryByText(/Waits for the history/)).toBeNull();
	});
});

// @spec SETUP-FLOW-007
it("reads the progress again only when asked", async () => {
	const user = userEvent.setup();
	sources.mockResolvedValue(view([source({ held_from: "2019-01-01" })]));
	show();
	await screen.findByRole("list", { name: "History by source" });
	expect(sources).toHaveBeenCalledTimes(1);
	await user.click(screen.getByRole("button", { name: "Check again" }));
	expect(sources).toHaveBeenCalledTimes(2);
});

// @spec SETUP-FLOW-006
it("finishes at the dashboard, with the Sources page beside it", async () => {
	sources.mockResolvedValue(view([source({})]));
	show();
	expect(
		await screen.findByRole("link", { name: "Go to the dashboard" }),
	).toHaveAttribute("href", "/");
	expect(
		screen.getByRole("link", { name: "See your sources" }),
	).toHaveAttribute("href", "/sources");
});
