// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getPlaces, getPreview } from "../../api/client";
import { ApiError } from "../../api/errors";
import type { Preview } from "../../types/setup";
import LocationStep from "./LocationStep";

vi.mock("../../api/client", () => ({
	getPlaces: vi.fn(),
	getPreview: vi.fn(),
}));
const places = vi.mocked(getPlaces);
const preview = vi.mocked(getPreview);

const avondale: Preview = {
	latitude: 39.8234,
	longitude: -75.7846,
	timezone: "America/New_York",
	elevation_ft: 310,
	inputs: [],
	nearest_soil_station: {
		station_ref: 7,
		network: "USCRN",
		name: "PA Avondale 2 N",
		state: "PA",
		distance_miles: 3.2,
		elevation_difference_ft: 20,
		first_year: 2008,
		last_year: null,
	},
};

const stored = {
	place_name: "Kennett Square, PA",
	latitude: 39.8468,
	longitude: -75.7116,
	timezone: "America/New_York",
};

beforeEach(() => {
	places.mockReset();
	preview.mockReset();
	places.mockResolvedValue([
		{
			name: "Avondale",
			state: "Pennsylvania",
			latitude: 39.82341,
			longitude: -75.78456,
		},
	]);
	preview.mockResolvedValue(avondale);
});

// @spec SETUP-LOC-001, SETUP-LOC-002, SETUP-LOC-005
describe("searching for the lawn", () => {
	it("says where the typed text goes and sends it only when asked", async () => {
		const user = userEvent.setup();
		render(<LocationStep stored={null} onNext={vi.fn()} />);
		expect(
			screen.getByText(/sent to the Open-Meteo place search/),
		).toBeInTheDocument();
		await user.type(
			screen.getByLabelText("Place name or postal code"),
			"Avondale",
		);
		expect(places).not.toHaveBeenCalled();
		await user.click(screen.getByRole("button", { name: "Search" }));
		expect(places).toHaveBeenCalledWith("Avondale");
		expect(
			await screen.findByRole("button", { name: /Avondale, Pennsylvania/ }),
		).toBeInTheDocument();
	});
});

// @spec SETUP-LOC-003
describe("checking the point before going on", () => {
	it("shows the place, the coordinates as stored, the timezone and the nearest soil station", async () => {
		const user = userEvent.setup();
		const onNext = vi.fn();
		render(<LocationStep stored={null} onNext={onNext} />);
		await user.type(
			screen.getByLabelText("Place name or postal code"),
			"Avondale",
		);
		await user.click(screen.getByRole("button", { name: "Search" }));
		await user.click(
			await screen.findByRole("button", { name: /Avondale, Pennsylvania/ }),
		);
		expect(preview).toHaveBeenCalledWith(39.82341, -75.78456);
		expect(
			await screen.findByText("Avondale, Pennsylvania"),
		).toBeInTheDocument();
		expect(screen.getByText("39.8234, -75.7846")).toBeInTheDocument();
		expect(screen.getByText("America/New_York")).toBeInTheDocument();
		expect(
			screen.getByText(
				"Nearest soil station: PA Avondale 2 N, PA (USCRN), 3 mi",
			),
		).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "Continue" }));
		expect(onNext).toHaveBeenCalledWith({
			latitude: 39.8234,
			longitude: -75.7846,
			place_name: "Avondale, Pennsylvania",
			preview: avondale,
		});
	});

	it("takes typed coordinates, saying what is wrong with one that is not a number", async () => {
		const user = userEvent.setup();
		render(<LocationStep stored={null} onNext={vi.fn()} />);
		await user.type(screen.getByLabelText("Latitude"), "north");
		await user.type(screen.getByLabelText("Longitude"), "-75.78");
		await user.click(
			screen.getByRole("button", { name: "Use these coordinates" }),
		);
		expect(
			screen.getByText("Latitude must be a number, such as 39.86."),
		).toBeInTheDocument();
		expect(preview).not.toHaveBeenCalled();
		await user.clear(screen.getByLabelText("Latitude"));
		await user.type(screen.getByLabelText("Latitude"), "39.82");
		await user.click(
			screen.getByRole("button", { name: "Use these coordinates" }),
		);
		expect(preview).toHaveBeenCalledWith(39.82, -75.78);
		expect(await screen.findByText("39.8234, -75.7846")).toBeInTheDocument();
	});
});

// @spec SETUP-LOC-004
it("refuses a point outside the contiguous United States with the server's reason", async () => {
	const user = userEvent.setup();
	preview.mockRejectedValue(
		new ApiError(
			400,
			"No weather data is available there yet: TurfOps covers the contiguous United States.",
		),
	);
	render(<LocationStep stored={null} onNext={vi.fn()} />);
	await user.type(screen.getByLabelText("Latitude"), "51.5");
	await user.type(screen.getByLabelText("Longitude"), "-0.12");
	await user.click(
		screen.getByRole("button", { name: "Use these coordinates" }),
	);
	expect(
		await screen.findByText(/No weather data is available there yet/),
	).toBeInTheDocument();
	expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
});

// @spec SETUP-LOC-006, SETUP-PAGE-006
describe("changing a stored location", () => {
	it("warns that station and grid history will be reloaded and sensor history kept", async () => {
		const user = userEvent.setup();
		render(<LocationStep stored={stored} onNext={vi.fn()} />);
		expect(screen.getByText(/Kennett Square, PA/)).toBeInTheDocument();
		await user.type(screen.getByLabelText("Latitude"), "39.82");
		await user.type(screen.getByLabelText("Longitude"), "-75.78");
		await user.click(
			screen.getByRole("button", { name: "Use these coordinates" }),
		);
		expect(
			await screen.findByText(
				"Station and grid history for the old location will be replaced and downloaded again. Your sensors' history is kept.",
			),
		).toBeInTheDocument();
	});

	it("does not warn when the point is the one stored", async () => {
		const user = userEvent.setup();
		preview.mockResolvedValue({
			...avondale,
			latitude: stored.latitude,
			longitude: stored.longitude,
		});
		render(<LocationStep stored={stored} onNext={vi.fn()} />);
		await user.type(screen.getByLabelText("Latitude"), "39.8468");
		await user.type(screen.getByLabelText("Longitude"), "-75.7116");
		await user.click(
			screen.getByRole("button", { name: "Use these coordinates" }),
		);
		await screen.findByRole("button", { name: "Continue" });
		expect(
			screen.queryByText(/will be replaced and downloaded again/),
		).toBeNull();
	});
});
