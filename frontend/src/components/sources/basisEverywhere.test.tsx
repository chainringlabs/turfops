// @vitest-environment jsdom
// Every dashboard widget that shows a weather-dependent value names its basis with the
// shared label, linked to the Sources page (WX-VIEW-BASIS-001).
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import { STATION } from "../../test/environmentalFixtures";
import type { GddSummary } from "../../types";
import DiseaseRiskWidget from "../disease/DiseaseRiskWidget";
import { response as diseaseResponse } from "../disease/testFixtures";
import GddWidget from "../GddWidget";
import SoilTempForecastWidget from "../SoilTempForecastWidget";
import TimingWidget from "../timing/TimingWidget";
import { response as timingResponse } from "../timing/testFixtures";

const PRISM = {
	kind: "Estimate" as const,
	source: "PRISM daily grid",
	distance_miles: null,
	approximate: false,
};

function gdd(basis: GddSummary["basis"]): GddSummary {
	return {
		year: 2026,
		current_gdd_total: 120,
		daily_history: [],
		crabgrass_model: {
			germination_threshold: 200,
			current_gdd: 120,
			status: "PreGermination",
			estimated_germination_date: null,
		},
		last_computed_date: "2026-11-27",
		basis,
	};
}

function labels(): string[] {
	return screen
		.getAllByRole("link")
		.filter((a) => a.getAttribute("href") === "/sources")
		.map((a) => a.textContent ?? "");
}

describe("basis labels on the dashboard widgets", () => {
	// @spec WX-VIEW-BASIS-001
	it("GDD names the daily air it was summed from, mixed when it is", () => {
		render(
			<MemoryRouter>
				<GddWidget data={gdd({ basis: PRISM, mixed: true })} />
			</MemoryRouter>,
		);
		expect(labels()).toEqual([
			expect.stringContaining("Estimated for your location · mixed"),
		]);
	});

	// @spec WX-VIEW-BASIS-001
	it("GDD with no basis shows no label", () => {
		render(
			<MemoryRouter>
				<GddWidget data={gdd(null)} />
			</MemoryRouter>,
		);
		expect(
			screen
				.queryAllByRole("link")
				.filter((a) => a.getAttribute("href") === "/sources"),
		).toEqual([]);
	});

	// @spec WX-VIEW-BASIS-001
	it("the outlook, disease and timing widgets name their soil or hourly basis", () => {
		render(
			<MemoryRouter>
				<SoilTempForecastWidget
					crossings={[]}
					currentSoilTemp={48}
					basis={STATION}
				/>
				<DiseaseRiskWidget data={diseaseResponse()} />
				<TimingWidget data={timingResponse()} />
			</MemoryRouter>,
		);
		const found = labels();
		expect(found).toHaveLength(3);
		expect(found[0]).toContain("Measured · PA Avondale 2 N, 2 mi");
		expect(found[2]).toContain("Measured · Avondale 2 N, 20 mi");
	});
});
