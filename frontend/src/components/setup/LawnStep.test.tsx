// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { getProfile, updateProfile } from "../../api/client";
import type { LawnProfile } from "../../types";
import LawnStep from "./LawnStep";

vi.mock("../../api/client", () => ({
	getProfile: vi.fn(),
	updateProfile: vi.fn(),
}));
const load = vi.mocked(getProfile);
const update = vi.mocked(updateProfile);

const PROFILE = {
	id: 1,
	name: "Front yard",
	grass_type: "TallFescue",
	usda_zone: "7a",
	soil_type: "Loam",
	lawn_size_sqft: 5000,
	irrigation_type: "Hose",
	location: {
		latitude: 39.82,
		longitude: -75.78,
		place_name: "Avondale, PA",
		timezone: "America/New_York",
	},
	created_at: "2026-01-01T00:00:00Z",
	updated_at: "2026-11-24T00:00:00Z",
} as unknown as LawnProfile;

beforeEach(() => {
	load.mockReset();
	update.mockReset();
	load.mockResolvedValue(PROFILE);
	update.mockResolvedValue(PROFILE);
});

// @spec SETUP-FLOW-008, SETUP-FLOW-003
describe("your lawn", () => {
	it("shows the profile form filled in from the stored profile", async () => {
		render(<LawnStep onNext={vi.fn()} />);
		expect(await screen.findByLabelText("Lawn Name")).toHaveValue("Front yard");
		expect(screen.getByLabelText("Lawn Size (sqft)")).toHaveValue(5000);
	});

	it("saves the profile without a location, then goes on", async () => {
		const user = userEvent.setup();
		const onNext = vi.fn();
		render(<LawnStep onNext={onNext} />);
		await user.click(
			await screen.findByRole("button", { name: "Save and continue" }),
		);
		expect(update).toHaveBeenCalledTimes(1);
		expect(update.mock.calls[0][0]).not.toHaveProperty("location");
		await vi.waitFor(() => expect(onNext).toHaveBeenCalled());
	});

	it("goes on without saving when skipped", async () => {
		const user = userEvent.setup();
		const onNext = vi.fn();
		render(<LawnStep onNext={onNext} />);
		await user.click(await screen.findByRole("button", { name: "Skip" }));
		expect(update).not.toHaveBeenCalled();
		expect(onNext).toHaveBeenCalled();
	});
});
