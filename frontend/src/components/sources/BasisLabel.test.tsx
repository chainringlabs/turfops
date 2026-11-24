// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import type { Basis } from "../../types/sources";
import BasisLabel from "./BasisLabel";

function show(basis: Basis, props: { mixed?: boolean; link?: boolean } = {}) {
	return render(
		<MemoryRouter>
			<BasisLabel basis={basis} {...props} />
		</MemoryRouter>,
	);
}

const station: Basis = {
	kind: "Station",
	source: "PA Avondale 2 N",
	distance_miles: 12.4,
	approximate: false,
};

// @spec WX-VIEW-BASIS-001
describe("BasisLabel words", () => {
	it("names a station with its distance", () => {
		show(station);
		expect(
			screen.getByText("Measured · PA Avondale 2 N, 12 mi"),
		).toBeInTheDocument();
	});

	it("names the owner's sensor", () => {
		show({
			kind: "Sensor",
			source: "Patio temperature",
			distance_miles: null,
			approximate: false,
		});
		expect(
			screen.getByText("Your sensor · Patio temperature"),
		).toBeInTheDocument();
	});

	it("says an estimate is for the location, adding approximate and mixed when they apply", () => {
		show(
			{
				kind: "Estimate",
				source: "Hourly analysis",
				distance_miles: null,
				approximate: true,
			},
			{ mixed: true },
		);
		expect(
			screen.getByText("Estimated for your location · approximate · mixed"),
		).toBeInTheDocument();
	});
});

// @spec WX-VIEW-BASIS-002
describe("BasisLabel mark and link", () => {
	it("marks a station with a solid dot, a sensor with a solid square and an estimate with a hollow dot", () => {
		const marks = (["Station", "Sensor", "Estimate"] as const).map((kind) => {
			const { container, unmount } = show({ ...station, kind });
			const mark = container.querySelector("[data-basis-mark]");
			const text = mark?.textContent;
			unmount();
			return text;
		});
		expect(marks).toEqual(["●", "■", "○"]);
	});

	it("hides the mark from screen readers, whose words already say the basis", () => {
		const { container } = show(station);
		expect(
			container.querySelector("[data-basis-mark]")?.getAttribute("aria-hidden"),
		).toBe("true");
	});

	it("links to the Sources page unless told not to", () => {
		show(station);
		expect(screen.getByRole("link")).toHaveAttribute("href", "/sources");
	});

	it("does not link on the Sources page itself", () => {
		show(station, { link: false });
		expect(screen.queryByRole("link")).toBeNull();
		expect(
			screen.getByText("Measured · PA Avondale 2 N, 12 mi"),
		).toBeInTheDocument();
	});
});
