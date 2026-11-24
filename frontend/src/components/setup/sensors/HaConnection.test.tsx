// @vitest-environment jsdom
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
	forgetHaConnection,
	getHaConnection,
	saveHaConnection,
	testHaConnection,
} from "../../../api/client";
import { ApiError } from "../../../api/errors";
import HaConnection from "./HaConnection";

vi.mock("../../../api/client", () => ({
	getHaConnection: vi.fn(),
	saveHaConnection: vi.fn(),
	forgetHaConnection: vi.fn(),
	testHaConnection: vi.fn(),
}));
const load = vi.mocked(getHaConnection);
const save = vi.mocked(saveHaConnection);
const forget = vi.mocked(forgetHaConnection);
const test = vi.mocked(testHaConnection);

const ADDRESS = "http://homeassistant.local:8123";
const TOKEN = "long-lived-token-0123456789";

beforeEach(() => {
	for (const m of [load, save, forget, test]) m.mockReset();
	test.mockResolvedValue({
		answered: { location_name: "Home", version: "2026.11.1" },
		numeric_sensors: 312,
	});
	save.mockResolvedValue({ connected: true, address: ADDRESS });
	forget.mockResolvedValue({ connected: false, address: null });
});

async function fill(user: ReturnType<typeof userEvent.setup>, token = TOKEN) {
	await user.type(
		await screen.findByLabelText("Home Assistant address"),
		ADDRESS,
	);
	await user.type(screen.getByLabelText("Long-lived access token"), token);
}

// @spec SETUP-HA-001, SETUP-HA-004
describe("connecting Home Assistant", () => {
	it("tests the address and token given, says what answered, and only then offers to save them", async () => {
		const user = userEvent.setup();
		load.mockResolvedValue({ connected: false, address: null });
		const onConnected = vi.fn();
		render(<HaConnection onConnected={onConnected} />);
		await fill(user);
		expect(
			screen.queryByRole("button", { name: "Save connection" }),
		).toBeNull();
		await user.click(screen.getByRole("button", { name: "Test" }));
		expect(test).toHaveBeenCalledWith({ address: ADDRESS, token: TOKEN });
		expect(
			await screen.findByText(
				"Home (2026.11.1) answered; 312 numeric sensors.",
			),
		).toBeInTheDocument();
		await user.click(screen.getByRole("button", { name: "Save connection" }));
		expect(save).toHaveBeenCalledWith(ADDRESS, TOKEN);
		expect(onConnected).toHaveBeenLastCalledWith(true);
	});

	it("takes back the offer to save when the address or token changes after the test", async () => {
		const user = userEvent.setup();
		load.mockResolvedValue({ connected: false, address: null });
		render(<HaConnection onConnected={vi.fn()} />);
		await fill(user);
		await user.click(screen.getByRole("button", { name: "Test" }));
		await screen.findByRole("button", { name: "Save connection" });
		await user.type(screen.getByLabelText("Long-lived access token"), "x");
		expect(
			screen.queryByRole("button", { name: "Save connection" }),
		).toBeNull();
	});

	it("says why a test failed and offers no save", async () => {
		const user = userEvent.setup();
		load.mockResolvedValue({ connected: false, address: null });
		test.mockRejectedValue(
			new ApiError(400, "Home Assistant refused the token."),
		);
		render(<HaConnection onConnected={vi.fn()} />);
		await fill(user);
		await user.click(screen.getByRole("button", { name: "Test" }));
		expect(
			await screen.findByText("Home Assistant refused the token."),
		).toBeInTheDocument();
		expect(
			screen.queryByRole("button", { name: "Save connection" }),
		).toBeNull();
	});

	it("keeps the token out of sight", async () => {
		load.mockResolvedValue({ connected: false, address: null });
		render(<HaConnection onConnected={vi.fn()} />);
		expect(
			await screen.findByLabelText("Long-lived access token"),
		).toHaveAttribute("type", "password");
	});
});

// @spec SETUP-HA-002, SETUP-HA-005
describe("a stored connection", () => {
	it("shows as connected with Change, and the form only on Change", async () => {
		const user = userEvent.setup();
		load.mockResolvedValue({ connected: true, address: ADDRESS });
		const onConnected = vi.fn();
		render(<HaConnection onConnected={onConnected} />);
		expect(
			await screen.findByText(`Connected to ${ADDRESS}`),
		).toBeInTheDocument();
		expect(onConnected).toHaveBeenCalledWith(true);
		expect(screen.queryByLabelText("Long-lived access token")).toBeNull();
		await user.click(screen.getByRole("button", { name: "Change" }));
		expect(
			screen.getByLabelText("Long-lived access token"),
		).toBeInTheDocument();
	});

	it("is forgotten only after a confirmation saying what stops and what is kept", async () => {
		const user = userEvent.setup();
		load.mockResolvedValue({ connected: true, address: ADDRESS });
		const onConnected = vi.fn();
		render(<HaConnection onConnected={onConnected} />);
		await user.click(await screen.findByRole("button", { name: "Forget" }));
		expect(forget).not.toHaveBeenCalled();
		expect(
			screen.getByText(
				"Its sensors stop being read. Their mappings and history are kept.",
			),
		).toBeInTheDocument();
		await user.click(
			screen.getByRole("button", { name: "Forget the connection" }),
		);
		expect(forget).toHaveBeenCalledTimes(1);
		expect(onConnected).toHaveBeenLastCalledWith(false);
	});
});
